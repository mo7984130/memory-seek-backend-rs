use std::collections::HashSet;

use common::time::Duration;

use audit::{AuditEvent, AuditRecorder};
use common::db_transaction;
use common::error::contextual::ext::{ContextualResultExt, IntoContextualExt, OptionExt};
use common::error::{AppError, contextual::Result};
use common::ext::ToOk;
use common::metrics_name;
use common::types::CursorPage;
use common::utils::MetricsTimerExt;
use constants::RedisKeys;
use sea_orm::ActiveModelTrait;
use types_core::UserId;
use types_visual::ImageDimensions;
use types_visual::dto::visual::{PageDirection, VisualCursorParam};
use types_visual::visual::{ActiveModel, Model, NewVisualRecord, VisualId, VisualRecord};

use crate::{
    mappers::{visual_like_mapper::VisualLikeMapper, visual_mapper::VisualMapper},
    state::{CachedVisualLike, VisualState},
};

const MEDIA_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// 首屏缓存统一保存 API 允许的最大页，避免 `size` 不在缓存键中造成结果串页。
const MEDIA_CURSOR_CACHE_SIZE: u64 = 1024;

/// 影像领域数据访问仓储，统一封装数据库与多级缓存。
pub struct VisualRepo;

impl VisualRepo {
    /// 加载影像记录.
    /// 返回影像记录 和 是否被喜欢的影像Id
    pub async fn load_visual_records(
        state: &VisualState,
        user_id: UserId,
        visual_ids: &[VisualId],
    ) -> Result<(Vec<Option<VisualRecord>>, HashSet<VisualId>)> {
        let (visuals, cached_visual_likes) = tokio::join!(
            // 获取影像记录
            state.cache_visual_info.get_or_load_batch(
                visual_ids,
                |id| RedisKeys::visual::visual::visual_info(*id),
                MEDIA_CACHE_TTL,
                |miss_ids| async move { VisualMapper::query_by_ids(&state.db, &miss_ids).await },
                |visual| visual.id,
            )
            .timed(metrics_name!("cache_get_or_load_batch")),
            // 获取是否被喜欢
            state.cache_visual_like.get_or_load_batch(
                visual_ids,
                |id| RedisKeys::visual::visual::visual_is_liked(user_id, *id),
                MEDIA_CACHE_TTL,
                |miss_ids| async move {
                    let liked_visual_ids =
                        VisualLikeMapper::query_is_like_by_visual_ids(&state.db, user_id, &miss_ids)
                            .await?;
                    miss_ids
                        .into_iter()
                        .map(|visual_id| CachedVisualLike {
                            is_liked: liked_visual_ids.contains(&visual_id),
                            visual_id,
                        })
                        .collect::<Vec<_>>()
                        .to_ok()
                },
                |cached| cached.visual_id,
            )
            .timed(metrics_name!("cache_get_or_load_batch")),
        );
        let visuals = visuals?;
        let liked_visual_ids = cached_visual_likes?
            .into_iter()
            .zip(visual_ids)
            .filter_map(|(cached, &visual_id)| {
                cached.and_then(|cached| cached.is_liked.then_some(visual_id))
            })
            .collect();
        Ok((visuals, liked_visual_ids))
    }

    /// 缓存影像喜欢状态.
    pub(super) async fn cache_visual_like_status(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
        is_liked: bool,
    ) {
        let key = RedisKeys::visual::visual::visual_is_liked(user_id, visual_id);
        state
            .cache_visual_like
            .put(
                &key,
                CachedVisualLike {
                    visual_id,
                    is_liked,
                },
                MEDIA_CACHE_TTL,
            )
            .timed(metrics_name!("cache_put"))
            .await
            .into_contextual()
            .emit_if_err();
    }

    /// 影像聚合字段变更后失效详情缓存。
    pub(super) async fn invalidate_visual_info(state: &VisualState, visual_id: VisualId) {
        let key = RedisKeys::visual::visual::visual_info(visual_id);
        state
            .cache_visual_info
            .invalidate(&key)
            .timed(metrics_name!("cache_invalidate"))
            .await
            .into_contextual()
            .emit_if_err();
    }

    /// 游标查询影像id.
    pub async fn query_visual_cursor_ids(
        state: &VisualState,
        req: VisualCursorParam,
    ) -> Result<CursorPage<VisualId, ()>> {
        let visual_ids = if req.cursor.is_none() && req.anchor_time.is_none() {
            let key = RedisKeys::visual::visual::visual_cursor_page_ids(req.direction);
            let direction = req.direction;
            let size = req.size;
            let page = state
                .cache_visual_cursor_ids
                .get_or_load(key, MEDIA_CACHE_TTL, || async move {
                    VisualMapper::query_cursor_page_ids(
                        &state.db,
                        None,
                        MEDIA_CURSOR_CACHE_SIZE,
                        direction,
                        None,
                    )
                    .await
                })
                .timed(metrics_name!("cache_get_or_load"))
                .await?;
            Self::resize_cached_first_page(page, size)
        } else {
            VisualMapper::query_cursor_page_ids(
                &state.db,
                req.cursor,
                req.size,
                req.direction,
                req.anchor_time,
            )
            .timed(metrics_name!("db_query"))
            .await?
        };

        Ok(visual_ids)
    }

    /// 失效影像游标 ID 缓存.
    async fn invalidate_visual_cursor_ids(state: &VisualState) {
        let keys = [
            RedisKeys::visual::visual::visual_cursor_page_ids(PageDirection::Next).to_owned(),
            RedisKeys::visual::visual::visual_cursor_page_ids(PageDirection::Prev).to_owned(),
        ];
        state
            .cache_visual_cursor_ids
            .invalidate_batch(&keys)
            .await
            .into_contextual()
            .emit_if_err();
    }

    fn resize_cached_first_page(
        mut page: CursorPage<VisualId, ()>,
        size: u64,
    ) -> CursorPage<VisualId, ()> {
        if page.records.len() > size as usize {
            page.records.truncate(size as usize);
            page.has_more = true;
        }
        page
    }

    /// 插入影像.
    pub async fn insert_visual(state: &VisualState, visual: NewVisualRecord) -> Result<Model> {
        db_transaction!(contextual & state.db, |txn| {
            let visual: ActiveModel = visual.into();
            let visual = visual.insert(txn).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("upload")
                    .with_actor(visual.user_id.0)
                    .with_target("visual", visual.id.0),
            )
            .await?;
            Ok(visual)
        })
        .timed(metrics_name!("db_insert"))
        .await
    }

    pub async fn ensure_exist(state: &VisualState, visual_id: VisualId) -> Result<()> {
        VisualMapper::ensure_exist(&state.db, visual_id)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// 批量查询影像哈希值是否存在.
    pub async fn exists_by_hash_batch(state: &VisualState, hashes: &[String]) -> Result<Vec<bool>> {
        let existing = VisualMapper::exists_by_hash_batch(&state.db, hashes)
            .timed(metrics_name!("db_query"))
            .await?;
        Ok(hashes.iter().map(|hash| existing.contains(hash)).collect())
    }

    /// 查询单个影像哈希值是否存在.
    pub async fn exists_by_hash(state: &VisualState, hash: &str) -> Result<bool> {
        VisualMapper::exists_by_hash(&state.db, hash)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// 处理影像上传完成后的影像域缓存更新.
    pub async fn after_visual_upload(state: &VisualState) {
        Self::invalidate_visual_cursor_ids(state)
            .timed(metrics_name!("cache_invalidate"))
            .await;
    }

    /// 通过file_id 获取对应的影像尺寸.
    pub async fn get_visual_dimensions(
        state: &VisualState,
        file_id: &str,
    ) -> Result<ImageDimensions> {
        let key = RedisKeys::visual::visual::visual_dimensions(file_id);
        state
            .cache_visual_dimensions
            .get_or_load(key.as_str(), MEDIA_CACHE_TTL, || async move {
                VisualMapper::query_dimensions_by_file_id(&state.db, file_id)
                    .await?
                    .ok_or_warn(
                        "visual_not_found",
                        "裁剪影像不存在",
                        AppError::bad_request("影像不存在"),
                    )
            })
            .timed(metrics_name!("cache_get_or_load"))
            .await
            .map(|dimensions| ImageDimensions {
                width: dimensions.0,
                height: dimensions.1,
            })
    }

    /// 失效影像删除后受影响的影像和人物缓存.
    pub async fn invalidate_deleted_visuals(state: &VisualState, visuals: &[VisualRecord]) {
        let visual_keys = visuals
            .iter()
            .map(|visual| RedisKeys::visual::visual::visual_info(visual.id))
            .collect::<Vec<_>>();
        let dimension_keys = visuals
            .iter()
            .map(|visual| RedisKeys::visual::visual::visual_dimensions(&visual.file_id))
            .collect::<Vec<_>>();

        let _ = tokio::join!(
            state
                .cache_visual_info
                .invalidate_batch(&visual_keys)
                .timed(metrics_name!("cache_invalidate")),
            state
                .cache_visual_dimensions
                .invalidate_batch(&dimension_keys)
                .timed(metrics_name!("cache_invalidate_dimensions")),
            Self::invalidate_visual_cursor_ids(state)
                .timed(metrics_name!("cache_invalidate_cursor_ids")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_first_page_is_trimmed_to_requested_size() {
        let page = CursorPage::from_has_more(vec![VisualId(1), VisualId(2), VisualId(3)], false);

        let page = VisualRepo::resize_cached_first_page(page, 2);

        assert_eq!(page.records, vec![VisualId(1), VisualId(2)]);
        assert!(page.has_more);
    }

    #[test]
    fn cached_first_page_preserves_source_has_more() {
        let page = CursorPage::from_has_more(vec![VisualId(1)], true);

        let page = VisualRepo::resize_cached_first_page(page, 32);

        assert_eq!(page.records, vec![VisualId(1)]);
        assert!(page.has_more);
    }
}
