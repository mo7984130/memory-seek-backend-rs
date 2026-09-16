use audit::{AuditEvent, AuditRecorder};
use common::{
    DbConn as ConnectionTrait, db_transaction,
    error::{AppError, ContextualError, contextual::Result},
    ext::ToOk,
    metrics_name,
    types::CursorPage,
    utils::{DbUtils, MetricsTimerExt},
};
use sea_orm::{DbBackend, EntityName, Statement};
use types::{
    auth::user::UserId,
    media::{
        PersonMediaCursorParam, UnassignedFaceMediaCursorParam,
        face::{self, FaceId, FaceRecord, NewFaceRecord},
        person::{self, PersonId},
        media::MediaId,
    },
};

use crate::{
    MediaState,
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper},
};

pub struct FaceRepo;

// 创建
impl FaceRepo {
    /// 批量插入人脸.
    pub async fn insert_faces(state: &MediaState, faces: Vec<NewFaceRecord>) -> Result<()> {
        FaceMapper::inserts(&state.db, faces)
            .timed(metrics_name!("db_insert"))
            .await?;

        Ok(())
    }
}

// 修改
impl FaceRepo {
    pub async fn change_face_belonging(
        state: &MediaState,
        face_id: FaceId,
        person_id: Option<PersonId>,
        user_id: UserId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 锁定人脸
            let face = FaceMapper::lock_by_id(txn, face_id).await?;
            // 归属未变化, 直接返回
            if person_id == face.person_id {
                return Ok(());
            }

            // 移动人脸归属
            FaceMapper::update_person_id(txn, person_id, face_id).await?;

            // 修改归属
            match person_id {
                // 新人物
                Some(new_person_id) => {
                    // 加锁涉及的两个人物行
                    let (new_person, old_person) = DbUtils::ensure_lock_two_optional_ordered(
                        txn,
                        new_person_id,
                        face.person_id,
                        |txn, id| async move { Ok(PersonMapper::lock_by_id(txn, id).await?) },
                    )
                    .await
                    .map_err(|error| {
                        ContextualError::error_without_source(
                            "person_lock_err",
                            "锁定人物失败",
                            error,
                        )
                    })?;

                    // 添加进的人物
                    PersonMapper::add_faces(txn, new_person, std::slice::from_ref(&face)).await?;

                    // 移出的人物
                    if let Some(old_person) = old_person {
                        PersonMapper::remove_faces(txn, old_person, &[face]).await?;
                    }
                }
                // 取消归属: 仅需处理旧人物减量维护
                None => {
                    if let Some(old_person_id) = face.person_id {
                        let old_person = PersonMapper::lock_by_id(txn, old_person_id).await?;

                        PersonMapper::remove_faces(txn, old_person, &[face]).await?;
                    }
                }
            };

            AuditRecorder::append(
                txn,
                AuditEvent::new("face_change_belonging")
                    .with_actor(user_id)
                    .with_target("face", face_id)
                    .with_detail(serde_json::json!({ "toPersonId": person_id })),
            )
            .await?;

            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        Ok(())
    }
}

// 查询
impl FaceRepo {
    pub async fn query_face_compute_medias(
        state: &MediaState,
        full: bool,
        size: u64,
        previous_id: MediaId,
    ) -> Result<Vec<(MediaId, String)>> {
        FaceMapper::query_face_compute_medias(&state.db, full, size, previous_id)
            .timed(metrics_name!("query"))
            .await
    }

    /// 查询人脸及其人物名称
    pub async fn query_faces_with_person_names(
        state: &MediaState,
        media_id: MediaId,
    ) -> Result<(Vec<FaceRecord>, std::collections::HashMap<PersonId, String>)> {
        let faces = FaceMapper::query_by_media_id(&state.db, media_id)
            .timed(metrics_name!("query_by_media_id"))
            .await?;
        let ids = faces
            .iter()
            .filter_map(|face| face.person_id)
            // 去重
            .collect::<std::collections::HashSet<_>>();
        let names = PersonMapper::query_id_and_name_by_ids(&state.db, ids)
            .timed(metrics_name!("query_id_and_name_by_ids"))
            .await?
            .into_iter()
            .collect();
        Ok((faces, names))
    }

    /// 查询未分配人脸的人脸记录.
    pub async fn lock_unassigned_faces(state: &MediaState) -> Result<Vec<FaceRecord>> {
        FaceMapper::lock_unassigned_faces(&state.db)
            .timed(metrics_name!("query_unassigned_faces"))
            .await
    }

    /// 游标查询包含未分配人脸的照片 ID.
    pub async fn query_unassigned_face_media_ids(
        state: &MediaState,
        param: &UnassignedFaceMediaCursorParam,
    ) -> Result<CursorPage<MediaId, ()>> {
        FaceMapper::query_unassigned_face_media_ids_cursor_page(
            &state.db,
            param.cursor.clone(),
            param.size,
        )
        .timed(metrics_name!("query_unassigned_face_media_ids"))
        .await
    }

    /// 查询人物关联的照片 ID.
    pub async fn query_person_media_ids(
        state: &MediaState,
        person_id: PersonId,
        req: &PersonMediaCursorParam,
    ) -> Result<CursorPage<MediaId, ()>> {
        FaceMapper::query_person_media_ids(&state.db, person_id, req.cursor.clone(), req.size)
            .timed(metrics_name!("query_media_ids"))
            .await
    }
}

// 删除
impl FaceRepo {
    // 删除人脸
    pub async fn delete_faces(
        state: &MediaState,
        face_ids: Vec<FaceId>,
        user_id: UserId,
    ) -> Result<u64> {
        db_transaction!(scoped & state.db, |txn| {
            // 行锁读取人脸
            let faces = FaceMapper::lock_by_ids(txn, &face_ids).await?;

            // 有归属的人脸不可删除
            if faces.iter().all(|face| face.person_id.is_some()) {
                return Err(ContextualError::warn_without_source(
                    "face_delete_conflict",
                    "用户尝试删除有归属的人脸",
                    AppError::bad_request("人脸已归属人物, 请先取消归属后再删除"),
                ));
            }

            // 删除人脸
            let affected = FaceMapper::delete_by_ids(txn, &face_ids).await?;

            AuditRecorder::append_many(
                txn,
                face_ids.iter().map(|id| {
                    AuditEvent::new("face_delete")
                        .with_actor(user_id.0)
                        .with_target("face", id.0)
                }),
            )
            .await?;
            Ok(affected)
        })
        .timed(metrics_name!("db_transaction"))
        .await?
        .to_ok()
    }
}

impl FaceRepo {
    pub async fn backup_and_truncate(state: &MediaState) -> Result<()> {
        Self::backup_face_tables(state).await?;

        common::db_transaction!(scoped & state.db, |txn| {
            txn.execute_raw(Statement::from_string(
                DbBackend::Postgres,
                format!(
                    "TRUNCATE TABLE {} RESTART IDENTITY CASCADE",
                    face::Entity.table_name()
                ),
            ))
            .await?;
            txn.execute_raw(Statement::from_string(
                DbBackend::Postgres,
                format!(
                    "TRUNCATE TABLE {} RESTART IDENTITY CASCADE",
                    person::Entity.table_name()
                ),
            ))
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        Ok(())
    }

    pub async fn backup_face_tables(state: &MediaState) -> Result<()> {
        use sea_orm::EntityName;
        backup::BackupService::backup_tables(
            state.backup_state.clone(),
            &[face::Entity.table_name(), person::Entity.table_name()],
        )
        .timed(metrics_name!("backup"))
        .await
        .map_err(|error| {
            ContextualError::error(
                "backup_face_tables",
                "备份人脸与人物表失败",
                error.to_string(),
                AppError::InternalServerError,
            )
        })?;
        Ok(())
    }
}
