use audit::{AuditEvent, AuditRecorder};
use common::{
    db_transaction,
    error::contextual::ext::UintExt,
    error::{AppError, ContextualError, contextual::Result},
    ext::ToErr,
    metrics_name,
    types::CursorPage,
    utils::MetricsTimerExt,
};
use types::visual::{
    collection::CollectionRecord,
    dto::collection::{
        CollectionCreateParam, CollectionVisualCursorPageParam, CollectionUpdateParam,
    },
};
use types::{
    auth::user::UserId, visual::collection::CollectionId, visual::models::VisualIds,
    visual::visual::VisualId,
};

use crate::mappers::{
    collection_mapper::CollectionMapper, collection_visual_mapper::CollectionVisualMapper,
    visual_mapper::VisualMapper,
};
use crate::state::VisualState;

pub(crate) struct CollectionRepo;

impl CollectionRepo {
    /// 查询影像所属的收藏夹
    pub(crate) async fn query_collection_ids_by_visual(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<Vec<CollectionId>> {
        CollectionVisualMapper::query_collection_ids_by_visual_id(&state.db, user_id, visual_id)
            .timed(metrics_name!("query_by_visual_id"))
            .await
    }

    /// 查询收藏夹id 和 描述
    pub(crate) async fn query_collection_briefs(
        state: &VisualState,
        ids: &[CollectionId],
    ) -> Result<Vec<(CollectionId, String)>> {
        CollectionMapper::query_id_and_name_by_ids(&state.db, ids)
            .timed(metrics_name!("query_collection_briefs"))
            .await
    }

    /// 游标查询收藏夹中的影像Id
    pub(crate) async fn query_collection_visual_ids(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
        req: &CollectionVisualCursorPageParam,
    ) -> Result<CursorPage<VisualId, ()>> {
        CollectionVisualMapper::query_visual_id_by_collection_id(
            &state.db,
            user_id,
            collection_id,
            req.cursor.as_ref(),
            req.size,
        )
        .timed(metrics_name!("query_visual_ids"))
        .await
    }

    pub async fn ensure_belong(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        CollectionMapper::ensure_belong(&state.db, user_id, collection_id)
            .timed(metrics_name!("ensure_belong"))
            .await
    }

    pub async fn ensure_belong_with_return(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<CollectionRecord> {
        CollectionMapper::ensure_belong_with_return(&state.db, user_id, collection_id)
            .timed(metrics_name!("ensure_belong_with_return"))
            .await
    }

    /// 添加相册影像
    pub(crate) async fn add_collection_visuals(
        state: &VisualState,
        user_id: UserId,
        collection_id: CollectionId,
        visual_ids: &VisualIds,
    ) -> Result<u64> {
        db_transaction!(scoped & state.db, |txn| {
            // 添加影像
            let count = CollectionMapper::add_visuals_batch(
                txn,
                user_id,
                collection_id,
                visual_ids.iter().copied().collect::<Vec<_>>(),
            )
            .await?;

            // 修改封面
            if let Some(visual_id) = visual_ids.first() {
                let file_id = VisualMapper::query_file_id_by_id(txn, *visual_id).await?;
                CollectionMapper::update_cover_visual(txn, collection_id, *visual_id, file_id)
                    .await?;
            }

            AuditRecorder::append(
                txn,
                AuditEvent::new("collect")
                    .with_actor(user_id.0)
                    .with_target("collection_id", collection_id.0)
                    .with_detail(serde_json::json!({ "visualIds": visual_ids })),
            )
            .await?;
            Ok(count)
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }
    /// 移除收藏夹影像.
    pub(crate) async fn remove_collection_visuals(
        state: &VisualState,
        user_id: UserId,
        collection: CollectionRecord,
        visual_ids: &VisualIds,
    ) -> Result<u64> {
        db_transaction!(scoped & state.db, |txn| {
            // 如果封面会被移除的话, 提前保存下来
            let cover_removed = collection
                .cover_visual_id
                .is_some_and(|id| visual_ids.contains(&id));

            // 删除收藏夹影像
            let rows = CollectionVisualMapper::delete_by_collection_id_and_visual_ids(
                txn,
                user_id,
                collection.id,
                visual_ids,
            )
            .await?;

            // 计算封面
            if cover_removed {
                // 获取第一张影像
                if let Some(visual_id) = CollectionVisualMapper::query_visual_id_by_collection_id(
                    txn,
                    user_id,
                    collection.id,
                    None,
                    1,
                )
                .await?
                .records
                .first()
                {
                    let file_id = VisualMapper::query_file_id_by_id(txn, *visual_id).await?;
                    CollectionMapper::update_cover_visual(txn, collection.id, *visual_id, file_id)
                        .await?;
                }
            }

            // 更新计数
            CollectionMapper::update_visual_count_delta(txn, collection.id, -(rows as i64)).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("uncollect")
                    .with_actor(user_id.0)
                    .with_target("collection", collection.id.0)
                    .with_detail(serde_json::json!({ "visualIds": visual_ids })),
            )
            .await?;
            Ok(rows)
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }

    /// 查询收藏夹列表.
    pub(crate) async fn query_collections(
        state: &VisualState,
        user_id: UserId,
    ) -> Result<Vec<CollectionRecord>> {
        CollectionMapper::query_by_user_id(&state.db, user_id)
            .timed(metrics_name!("query_by_user_id"))
            .await
    }

    /// 创建收藏夹.
    pub(crate) async fn create_collection(
        state: &VisualState,
        user_id: UserId,
        req: CollectionCreateParam,
    ) -> Result<CollectionRecord> {
        db_transaction!(scoped & state.db, |txn| {
            // 插入
            let collection =
                CollectionMapper::insert(txn, user_id, req.name, req.description).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("visual.collection_created")
                    .with_actor(user_id.0)
                    .with_target("collection", collection.id.0),
            )
            .await?;
            Ok(collection)
        })
        .timed(metrics_name!("db_insert"))
        .await
    }

    /// 更新收藏夹信息.
    pub(crate) async fn update_collection(
        state: &VisualState,
        user_id: UserId,
        id: CollectionId,
        req: CollectionUpdateParam,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            let affected =
                CollectionMapper::update_info(txn, id, user_id, req.name, req.description).await?;
            if affected > 0 {
                AuditRecorder::append(
                    txn,
                    AuditEvent::new("visual.collection_updated")
                        .with_actor(user_id.0)
                        .with_target("collection", id.0),
                )
                .await?;
                Ok(())
            } else {
                return ContextualError::warn_without_source(
                    "collection_update_info_fail",
                    "修改收藏夹信息失败",
                    AppError::bad_request("修改收藏夹信息失败"),
                )
                .to_err();
            }
        })
        .timed(metrics_name!("db_update"))
        .await?;

        Ok(())
    }

    /// 删除收藏夹.
    /// 无需健全, 在删除的时候保证user_id相等
    pub(crate) async fn delete_collection(
        state: &VisualState,
        user_id: UserId,
        id: CollectionId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 删除收藏夹
            CollectionMapper::delete_by_id(txn, id, user_id)
                .await?
                .no_zero_or_warn(
                    "delete_collection_fail",
                    "删除收藏夹失败",
                    AppError::bad_request("删除收藏夹失败"),
                )?;

            // 删除收藏夹影像
            CollectionVisualMapper::delete_by_collection_id(txn, id, user_id).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("visual.collection_deleted")
                    .with_actor(user_id.0)
                    .with_target("collection", id.0),
            )
            .await?;
            Ok(())
        })
        .timed(metrics_name!("db_transaction"))
        .await
    }
}
