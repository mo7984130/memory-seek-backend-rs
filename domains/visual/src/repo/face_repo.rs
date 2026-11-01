use audit::{AuditEvent, AuditRecorder};
use common_core::{
    DbConn as ConnectionTrait,
    error::{AppError, ContextualError, contextual::Result},
    ext::ToOk,
    types::CursorPage,
};
use common_db::{db_transaction, utils::DbUtils};
use common_metrics::{MetricsTimerExt, metrics_name};
use constants::RedisKeys;
use sea_orm::{DbBackend, EntityName, Statement};
use types_identity::auth::user::UserId;
use types_visual::{
    PersonVisualCursorParam, UnassignedFaceVisualCursorParam, face, face::FaceId, face::FaceRecord,
    face::NewFaceRecord, person, person::PersonId, visual::VisualId,
};

use crate::{
    VisualState,
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper},
    repo::count_cache,
};

pub struct FaceRepo;

// 创建
impl FaceRepo {
    /// 批量插入人脸
    pub async fn insert_faces(state: &VisualState, faces: Vec<NewFaceRecord>) -> Result<()> {
        FaceMapper::inserts(&state.db, faces)
            .timed(metrics_name!("db_insert"))
            .await?;

        count_cache::invalidate(state, RedisKeys::visual::count::UNASSIGNED_FACE_VISUALS).await;

        Ok(())
    }
}

// 修改
impl FaceRepo {
    /// 在事务内修改人脸归属，并维护新旧人物的质心、权重与封面
    pub async fn change_face_belonging(
        state: &VisualState,
        face_id: FaceId,
        person_id: Option<PersonId>,
        user_id: UserId,
    ) -> Result<()> {
        let old_person_id = db_transaction!(&state.db, |txn| {
            // 锁定人脸
            let face = FaceMapper::lock_by_id(txn, face_id).await?;
            let old_person_id = face.person_id;
            // 归属未变化, 直接返回
            if person_id == face.person_id {
                return Ok(old_person_id);
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

            Ok(old_person_id)
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        count_cache::invalidate(state, RedisKeys::visual::count::UNASSIGNED_FACE_VISUALS).await;
        if let Some(old_person_id) = old_person_id {
            count_cache::invalidate(
                state,
                &RedisKeys::visual::count::person_visuals(old_person_id),
            )
            .await;
        }
        if let Some(person_id) = person_id {
            count_cache::invalidate(state, &RedisKeys::visual::count::person_visuals(person_id))
                .await;
        }

        Ok(())
    }
}

// 查询
impl FaceRepo {
    /// 分批查询人脸计算所需的照片 ID 与 file_id
    pub async fn query_face_compute_visuals(
        state: &VisualState,
        full: bool,
        size: u64,
        previous_id: VisualId,
    ) -> Result<Vec<(VisualId, String)>> {
        FaceMapper::query_face_compute_visuals(&state.db, full, size, previous_id)
            .timed(metrics_name!("query"))
            .await
    }

    /// 查询人脸及其人物名称
    pub async fn query_faces_with_person_names(
        state: &VisualState,
        visual_id: VisualId,
    ) -> Result<(Vec<FaceRecord>, std::collections::HashMap<PersonId, String>)> {
        let faces = FaceMapper::query_by_visual_id(&state.db, visual_id)
            .timed(metrics_name!("query_by_visual_id"))
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

    /// 查询未分配人脸的人脸记录
    pub async fn lock_unassigned_faces(state: &VisualState) -> Result<Vec<FaceRecord>> {
        FaceMapper::lock_unassigned_faces(&state.db)
            .timed(metrics_name!("query_unassigned_faces"))
            .await
    }

    /// 游标查询包含未分配人脸的照片 ID
    pub async fn query_unassigned_face_visual_ids(
        state: &VisualState,
        param: &UnassignedFaceVisualCursorParam,
    ) -> Result<CursorPage<VisualId, ()>> {
        let total_count = count_cache::get_or_load(
            state,
            RedisKeys::visual::count::UNASSIGNED_FACE_VISUALS,
            || async { FaceMapper::count_unassigned_face_visuals(&state.db).await },
        )
        .await?;
        FaceMapper::query_unassigned_face_visual_ids_cursor_page(
            &state.db,
            param.cursor.clone(),
            param.size,
            total_count,
        )
        .timed(metrics_name!("query_unassigned_face_visual_ids"))
        .await
    }

    /// 查询人物关联的照片 ID
    pub async fn query_person_visual_ids(
        state: &VisualState,
        person_id: PersonId,
        req: &PersonVisualCursorParam,
    ) -> Result<CursorPage<VisualId, ()>> {
        let key = RedisKeys::visual::count::person_visuals(person_id);
        let total_count = count_cache::get_or_load(state, &key, || async {
            FaceMapper::count_person_visuals(&state.db, person_id).await
        })
        .await?;
        FaceMapper::query_person_visual_ids(
            &state.db,
            person_id,
            req.cursor.clone(),
            req.size,
            total_count,
        )
        .timed(metrics_name!("query_visual_ids"))
        .await
    }
}

// 删除
impl FaceRepo {
    // 删除人脸
    /// 在事务内删除无归属的人脸；有归属的人脸不可删除
    pub async fn delete_faces(
        state: &VisualState,
        face_ids: Vec<FaceId>,
        user_id: UserId,
    ) -> Result<u64> {
        let affected = db_transaction!(&state.db, |txn| {
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
        .await?;
        if affected > 0 {
            count_cache::invalidate(state, RedisKeys::visual::count::UNASSIGNED_FACE_VISUALS).await;
        }
        affected.to_ok()
    }
}

impl FaceRepo {
    /// 备份人脸与人物表后清空两表（重置自增）
    pub async fn backup_and_truncate(state: &VisualState) -> Result<()> {
        Self::backup_face_tables(state).await?;
        let person_ids = PersonMapper::query_all(&state.db)
            .await?
            .into_iter()
            .map(|person| person.id)
            .collect::<Vec<_>>();

        common_db::db_transaction!(&state.db, |txn| {
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

        count_cache::invalidate(state, RedisKeys::visual::count::PERSONS).await;
        count_cache::invalidate(state, RedisKeys::visual::count::UNASSIGNED_FACE_VISUALS).await;
        for person_id in person_ids {
            count_cache::invalidate(state, &RedisKeys::visual::count::person_visuals(person_id))
                .await;
        }

        Ok(())
    }

    /// 备份人脸表与人物表
    pub async fn backup_face_tables(state: &VisualState) -> Result<()> {
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
