use std::collections::{HashMap, HashSet};

use audit::{AuditEvent, AuditRecorder};
use common_core::error::{ContextualError, contextual::Result};
use common_core::{types::CursorPageWithTotal, types::HasChanged::Changed};
use common_db::{db_transaction, utils::DbUtils};
use common_metrics::{MetricsTimerExt, metrics_name};
use constants::RedisKeys;
use serde_json::json;
use types_core::cursor::CountIdCursor;
use types_core::{AdminId, UserId};
use types_visual::MergePersonParam;
use types_visual::face::FaceRecord;
use types_visual::person::{PersonId, PersonRecord, UpdatePersonRecord};
use types_visual::visual::VisualId;

use crate::VisualState;
use crate::mappers::face_mapper::FaceMapper;
use crate::mappers::person_mapper::PersonMapper;
use crate::mappers::visual_mapper::VisualMapper;
use crate::repo::count_cache;

pub struct PersonRepo;

// 创建
impl PersonRepo {}

// 修改
impl PersonRepo {
    /// 更新人物名称
    pub async fn rename_person(
        state: &VisualState,
        id: PersonId,
        name: String,
        initials: Option<String>,
        user_id: UserId,
    ) -> Result<()> {
        db_transaction!(&state.db, |txn| {
            let mut update_person = UpdatePersonRecord::new(id);
            update_person.name = Changed(name.clone());
            update_person.name_initials = Changed(initials.clone());
            PersonMapper::update(txn, update_person).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("person_rename")
                    .with_actor(user_id.0)
                    .with_target("person", id.0)
                    .with_detail(json!({ "name": name, "initials": initials })),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;
        Ok(())
    }

    // 合并人物
    // 返回合并后的人物记录
    /// 在事务内将源人物的人脸转移到目标人物并删除源人物，仅管理员
    pub async fn merge_person(
        state: &VisualState,
        admin: AdminId,
        req: MergePersonParam,
    ) -> Result<PersonRecord> {
        let MergePersonParam {
            source_person_id,
            target_person_id,
        } = req;

        let merged = db_transaction!(&state.db, |txn| {
            // 锁定人物
            let (source, target) = DbUtils::ensure_lock_two_ordered(
                txn,
                source_person_id,
                target_person_id,
                |db, id| async move { Ok(PersonMapper::lock_by_id(db, id).await?) },
            )
            .await
            .map_err(|error| {
                ContextualError::error_without_source("person_lock_err", "锁定人物失败", error)
            })?;

            // 获取源人物人脸
            let source_faces = FaceMapper::lock_by_person_id(txn, source_person_id).await?;

            // 转移人脸归属
            PersonMapper::add_faces(txn, target, &source_faces).await?;

            // 删除源人物
            PersonMapper::delete(txn, source.id).await?;

            // 返回合并后的目标人物视图
            let person = PersonMapper::query_by_id(txn, target_person_id).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("merge_person")
                    .with_actor(admin.into_inner())
                    .with_target("person_id", target_person_id)
                    .with_detail(json!({"source_person_id": source_person_id})),
            )
            .await?;

            Ok(person)
        })
        .timed(metrics_name!("db_transaction"))
        .await?;
        count_cache::invalidate(state, RedisKeys::visual::count::PERSONS).await;
        count_cache::invalidate(
            state,
            &RedisKeys::visual::count::person_visuals(source_person_id),
        )
        .await;
        count_cache::invalidate(
            state,
            &RedisKeys::visual::count::person_visuals(target_person_id),
        )
        .await;
        Ok(merged)
    }

    /// 为人物添加人脸
    pub async fn add_faces(
        state: &VisualState,
        person: PersonRecord,
        faces: Vec<FaceRecord>,
    ) -> Result<()> {
        let person_id = person.id;
        PersonMapper::add_faces(&state.db, person, &faces)
            .timed(metrics_name!("db_update"))
            .await?;
        count_cache::invalidate(state, &RedisKeys::visual::count::person_visuals(person_id)).await;
        Ok(())
    }
}

// 查询
impl PersonRepo {
    /// 查询所有人物
    pub async fn query_all(state: &VisualState) -> Result<Vec<PersonRecord>> {
        PersonMapper::query_all(&state.db)
            .timed(metrics_name!("query_all"))
            .await
    }

    /// 游标分页查询人物
    pub async fn query_page(
        state: &VisualState,
        cursor: Option<CountIdCursor<PersonId>>,
        size: u64,
    ) -> Result<CursorPageWithTotal<PersonRecord, ()>> {
        let total_count = if cursor.is_none() {
            Some(
                count_cache::get_or_load(state, RedisKeys::visual::count::PERSONS, || async {
                    PersonMapper::count_persons(&state.db).await
                })
                .await?,
            )
        } else {
            None
        };
        Ok(PersonMapper::query_page(&state.db, cursor, size)
            .timed(metrics_name!("query_page"))
            .await?
            .with_optional_total(total_count))
    }

    /// 按关键词查询人物分页
    pub async fn search_person_page(
        state: &VisualState,
        keyword: &str,
        cursor: Option<PersonId>,
        size: u64,
    ) -> Result<CursorPageWithTotal<types_visual::person::PersonRecord, ()>> {
        let key = RedisKeys::visual::count::person_search(keyword);
        let total_count = if cursor.is_none() {
            Some(
                count_cache::get_or_load(state, &key, || async {
                    PersonMapper::count_person_search(&state.db, keyword).await
                })
                .await?,
            )
        } else {
            None
        };
        Ok(PersonMapper::query_search(&state.db, keyword, cursor, size)
            .timed(metrics_name!("query_search"))
            .await?
            .with_optional_total(total_count))
    }

    /// 加载全部人脸及其对应影像文件的 file_id
    pub async fn load_faces_with_visual_files(
        state: &VisualState,
    ) -> Result<(Vec<FaceRecord>, HashMap<VisualId, String>)> {
        let faces = FaceMapper::query_all(&state.db)
            .timed(metrics_name!("query_faces"))
            .await?;
        let ids = faces
            .iter()
            .map(|face| face.visual_id)
            .collect::<HashSet<_>>();

        let files = VisualMapper::query_id_and_file_id_by_ids(&state.db, &ids)
            .timed(metrics_name!("query_visual_files"))
            .await?;
        Ok((faces, files))
    }
}

// 删除
impl PersonRepo {
    // 删除人物
    // 同时重置对应人脸的人物id
    // 仅可以管理员执行
    /// 在事务内清空人脸归属后删除人物，仅管理员
    pub async fn delete_person(
        state: &VisualState,
        person_id: PersonId,
        admin: AdminId,
    ) -> Result<()> {
        db_transaction!(&state.db, |txn| {
            // 清空该人物所有人脸归属
            FaceMapper::clean_person_id_by_person_id(txn, person_id).await?;

            // 删除人物
            PersonMapper::delete(txn, person_id).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("person_delete")
                    .with_actor(admin.into_inner())
                    .with_target("person", person_id),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        count_cache::invalidate(state, RedisKeys::visual::count::PERSONS).await;
        count_cache::invalidate(state, &RedisKeys::visual::count::person_visuals(person_id)).await;
        count_cache::invalidate(state, RedisKeys::visual::count::UNASSIGNED_FACE_VISUALS).await;

        Ok(())
    }
}
