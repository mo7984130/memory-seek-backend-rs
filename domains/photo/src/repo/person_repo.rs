use std::collections::{HashMap, HashSet};

use audit::{AuditEvent, AuditService};
use common::db_transaction;
use common::error::contextual::Result;
use common::ext::{ContextualResultExt, IntoContextualExt, ToOk};
use common::models::CursorPage;
use common::types::HasChanged::Changed;
use common::utils::DbUtils;
use constants::RedisKeys;
use serde_json::json;
use types::auth::user::{AdminId, UserId};
use types::cursor::CountIdCursor;
use types::photo::MergePersonParam;
use types::photo::face::FaceRecord;
use types::photo::person::{PersonId, PersonRecord, UpdatePersonRecord};
use types::photo::photo::PhotoId;

use crate::PhotoState;
use crate::mappers::face_mapper::FaceMapper;
use crate::mappers::person_mapper::PersonMapper;
use crate::mappers::photo_mapper::PhotoMapper;

pub struct PersonRepo;

// 创建
impl PersonRepo {}

// 修改
impl PersonRepo {
    /// 更新人物名称
    pub async fn rename_person(
        state: &PhotoState,
        id: PersonId,
        name: String,
        initials: Option<String>,
        user_id: UserId,
    ) -> Result<()> {
        db_transaction!(contextual & state.db, |txn| {
            let mut update_person = UpdatePersonRecord::new(id);
            update_person.name = Changed(name.clone());
            update_person.name_initials = Changed(initials.clone());
            PersonMapper::update(txn, update_person).await?;

            AuditService::append(
                txn,
                AuditEvent::new("person_rename")
                    .with_actor(user_id.0)
                    .with_target("person", id.0)
                    .with_detail(json!({ "name": name, "initials": initials })),
            )
            .await?;
            Ok(())
        })
        .await?;
        Self::invalidate_persons(state, &[id]).await;
        Ok(())
    }

    // 合并人物
    // 返回合并后的人物记录
    pub async fn merge_person(
        state: &PhotoState,
        admin: AdminId,
        req: MergePersonParam,
    ) -> Result<PersonRecord> {
        let MergePersonParam {
            source_person_id,
            target_person_id,
        } = req;

        db_transaction!(scoped & state.db, |txn| {
            // 锁定人物
            let (source, target) = DbUtils::ensure_lock_two_ordered(
                txn,
                source_person_id,
                target_person_id,
                |db, id| async move { Ok(PersonMapper::lock_by_id(db, id).await?) },
            )
            .await?;

            // 获取源人物人脸
            let source_faces = FaceMapper::lock_by_person_id(txn, source_person_id).await?;

            // 转移人脸归属
            PersonMapper::add_faces(txn, target, &source_faces).await?;

            // 删除源人物
            PersonMapper::delete(txn, source.id).await?;

            // 返回合并后的目标人物视图
            let person = PersonMapper::query_by_id(txn, target_person_id).await?;

            // 失效源/目标人物缓存
            Self::invalidate_persons(state, &[source_person_id, target_person_id]).await;

            AuditService::append(
                txn,
                AuditEvent::new("merge_person")
                    .with_actor(admin.into_inner())
                    .with_target("person_id", target_person_id)
                    .with_detail(json!({"source_person_id": source_person_id})),
            )
            .await?;

            Ok(person)
        })
        .await?
        .to_ok()
    }

    /// 为人物添加人脸.
    pub async fn add_faces(
        state: &PhotoState,
        person: PersonRecord,
        faces: Vec<FaceRecord>,
    ) -> Result<()> {
        PersonMapper::add_faces(&state.db, person, &faces).await?;
        Ok(())
    }
}

// 查询
impl PersonRepo {
    pub async fn query_all(state: &PhotoState) -> Result<Vec<PersonRecord>> {
        PersonMapper::query_all(&state.db).await
    }

    pub async fn query_page(
        state: &PhotoState,
        cursor: Option<CountIdCursor<PersonId>>,
        size: u64,
    ) -> Result<CursorPage<PersonRecord, ()>> {
        PersonMapper::query_page(&state.db, cursor, size).await
    }

    /// 按关键词查询人物分页.
    pub async fn search_person_page(
        state: &PhotoState,
        keyword: &str,
        cursor: Option<PersonId>,
        size: u64,
    ) -> Result<CursorPage<types::photo::person::PersonRecord, ()>> {
        PersonMapper::query_search(&state.db, keyword, cursor, size).await
    }

    pub async fn load_faces_with_photo_files(
        state: &PhotoState,
    ) -> Result<(Vec<FaceRecord>, HashMap<PhotoId, String>)> {
        let faces = FaceMapper::query_all(&state.db).await?;
        let ids = faces
            .iter()
            .map(|face| face.photo_id)
            .collect::<HashSet<_>>();

        let files = PhotoMapper::query_id_and_file_id_by_ids(&state.db, &ids).await?;
        Ok((faces, files))
    }
}

// 删除
impl PersonRepo {
    // 失效人物缓存
    pub async fn invalidate_persons(state: &PhotoState, ids: &[PersonId]) {
        let keys = ids
            .iter()
            .map(|id| RedisKeys::photo::person::person_info(*id))
            .collect::<Vec<_>>();
        state
            .cache_person
            .invalidate_batch(&keys)
            .await
            .into_contextual()
            .emit_if_err();
    }

    // 删除人物
    // 同时重置对应人脸的人物id
    // 仅可以管理员执行
    pub async fn delete_person(
        state: &PhotoState,
        person_id: PersonId,
        admin: AdminId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 清空该人物所有人脸归属
            FaceMapper::clean_person_id_by_person_id(txn, person_id).await?;

            // 删除人物
            PersonMapper::delete(txn, person_id).await?;

            AuditService::append(
                txn,
                AuditEvent::new("person_delete")
                    .with_actor(admin.into_inner())
                    .with_target("person", person_id),
            )
            .await?;
            Ok(())
        })
        .await?;

        // 失效人物缓存（L1 + L2）
        PersonRepo::invalidate_persons(state, &[person_id]).await;

        Ok(())
    }
}
