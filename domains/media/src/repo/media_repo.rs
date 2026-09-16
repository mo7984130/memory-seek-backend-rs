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
use types::auth::user::UserId;
use types::media::ImageDimensions;
use types::media::dto::media::{PageDirection, MediaCursorParam};
use types::media::media::{ActiveModel, Model, NewMediaRecord, MediaId, MediaRecord};

use crate::{
    mappers::{media_like_mapper::MediaLikeMapper, media_mapper::MediaMapper},
    state::{CachedMediaLike, MediaState},
};

const MEDIA_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// 首屏缓存统一保存 API 允许的最大页，避免 `size` 不在缓存键中造成结果串页。
const MEDIA_CURSOR_CACHE_SIZE: u64 = 1024;

/// 媒体领域数据访问仓储，统一封装数据库与多级缓存。
pub struct MediaRepo;

impl MediaRepo {
    /// 加载媒体记录.
    /// 返回媒体记录 和 是否被喜欢的媒体Id
    pub async fn load_media_records(
        state: &MediaState,
        user_id: UserId,
        media_ids: &[MediaId],
    ) -> Result<(Vec<Option<MediaRecord>>, HashSet<MediaId>)> {
        let (medias, cached_media_likes) =
            tokio::join!(
            // 获取媒体记录
            state.cache_media_info.get_or_load_batch(
                media_ids,
                |id| RedisKeys::media::media::media_info(*id),
                MEDIA_CACHE_TTL,
                |miss_ids| async move { MediaMapper::query_by_ids(&state.db, &miss_ids).await },
                |media| media.id,
            )
            .timed(metrics_name!("cache_get_or_load_batch")),
            // 获取是否被喜欢
            state.cache_media_like.get_or_load_batch(
                media_ids,
                |id| RedisKeys::media::media::media_is_liked(user_id, *id),
                MEDIA_CACHE_TTL,
                |miss_ids| async move {
                    let liked_media_ids =
                        MediaLikeMapper::query_is_like_by_media_ids(&state.db, user_id, &miss_ids)
                            .await?;
                    miss_ids
                        .into_iter()
                        .map(|media_id| CachedMediaLike {
                            is_liked: liked_media_ids.contains(&media_id),
                            media_id,
                        })
                        .collect::<Vec<_>>()
                        .to_ok()
                },
                |cached| cached.media_id,
            )
            .timed(metrics_name!("cache_get_or_load_batch")),
        );
        let medias = medias?;
        let liked_media_ids = cached_media_likes?
            .into_iter()
            .zip(media_ids)
            .filter_map(|(cached, &media_id)| {
                cached.and_then(|cached| cached.is_liked.then_some(media_id))
            })
            .collect();
        Ok((medias, liked_media_ids))
    }

    /// 缓存媒体喜欢状态.
    pub(super) async fn cache_media_like_status(
        state: &MediaState,
        user_id: UserId,
        media_id: MediaId,
        is_liked: bool,
    ) {
        let key = RedisKeys::media::media::media_is_liked(user_id, media_id);
        state
            .cache_media_like
            .put(
                &key,
                CachedMediaLike { media_id, is_liked },
                MEDIA_CACHE_TTL,
            )
            .timed(metrics_name!("cache_put"))
            .await
            .into_contextual()
            .emit_if_err();
    }

    /// 媒体聚合字段变更后失效详情缓存。
    pub(super) async fn invalidate_media_info(state: &MediaState, media_id: MediaId) {
        let key = RedisKeys::media::media::media_info(media_id);
        state
            .cache_media_info
            .invalidate(&key)
            .timed(metrics_name!("cache_invalidate"))
            .await
            .into_contextual()
            .emit_if_err();
    }

    /// 游标查询媒体id.
    pub async fn query_media_cursor_ids(
        state: &MediaState,
        req: MediaCursorParam,
    ) -> Result<CursorPage<MediaId, ()>> {
        let media_ids = if req.cursor.is_none() && req.anchor_time.is_none() {
            let key = RedisKeys::media::media::media_cursor_page_ids(req.direction);
            let direction = req.direction;
            let size = req.size;
            let page = state
                .cache_media_cursor_ids
                .get_or_load(key, MEDIA_CACHE_TTL, || async move {
                    MediaMapper::query_cursor_page_ids(
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
            MediaMapper::query_cursor_page_ids(
                &state.db,
                req.cursor,
                req.size,
                req.direction,
                req.anchor_time,
            )
            .timed(metrics_name!("db_query"))
            .await?
        };

        Ok(media_ids)
    }

    /// 失效媒体游标 ID 缓存.
    async fn invalidate_media_cursor_ids(state: &MediaState) {
        let keys = [
            RedisKeys::media::media::media_cursor_page_ids(PageDirection::Next).to_owned(),
            RedisKeys::media::media::media_cursor_page_ids(PageDirection::Prev).to_owned(),
        ];
        state
            .cache_media_cursor_ids
            .invalidate_batch(&keys)
            .await
            .into_contextual()
            .emit_if_err();
    }

    fn resize_cached_first_page(
        mut page: CursorPage<MediaId, ()>,
        size: u64,
    ) -> CursorPage<MediaId, ()> {
        if page.records.len() > size as usize {
            page.records.truncate(size as usize);
            page.has_more = true;
        }
        page
    }

    /// 插入媒体.
    pub async fn insert_media(state: &MediaState, media: NewMediaRecord) -> Result<Model> {
        db_transaction!(contextual & state.db, |txn| {
            let media: ActiveModel = media.into();
            let media = media.insert(txn).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("upload")
                    .with_actor(media.user_id.0)
                    .with_target("media", media.id.0),
            )
            .await?;
            Ok(media)
        })
        .timed(metrics_name!("db_insert"))
        .await
    }

    pub async fn ensure_exist(state: &MediaState, media_id: MediaId) -> Result<()> {
        MediaMapper::ensure_exist(&state.db, media_id)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// 批量查询图片 MD5 是否存在.
    pub async fn exists_by_md5_batch(state: &MediaState, md5s: &[String]) -> Result<Vec<bool>> {
        let existing = MediaMapper::exists_by_md5_batch(&state.db, md5s)
            .timed(metrics_name!("db_query"))
            .await?;
        Ok(md5s.iter().map(|md5| existing.contains(md5)).collect())
    }

    /// 查询单个图片 MD5 是否存在.
    pub async fn exists_by_md5(state: &MediaState, md5: &str) -> Result<bool> {
        MediaMapper::exists_by_md5(&state.db, md5)
            .timed(metrics_name!("db_query"))
            .await
    }

    /// 处理媒体上传完成后的媒体域缓存更新.
    pub async fn after_media_upload(state: &MediaState) {
        Self::invalidate_media_cursor_ids(state)
            .timed(metrics_name!("cache_invalidate"))
            .await;
    }

    /// 通过file_id 获取对应的图片尺寸.
    pub async fn get_media_dimensions(
        state: &MediaState,
        file_id: &str,
    ) -> Result<ImageDimensions> {
        let key = RedisKeys::media::media::media_dimensions(file_id);
        state
            .cache_media_dimensions
            .get_or_load(key.as_str(), MEDIA_CACHE_TTL, || async move {
                MediaMapper::query_dimensions_by_file_id(&state.db, file_id)
                    .await?
                    .ok_or_warn(
                        "media_not_found",
                        "裁剪图片不存在",
                        AppError::bad_request("媒体不存在"),
                    )
            })
            .timed(metrics_name!("cache_get_or_load"))
            .await
            .map(|dimensions| ImageDimensions {
                width: dimensions.0,
                height: dimensions.1,
            })
    }

    /// 失效媒体删除后受影响的媒体和人物缓存.
    pub async fn invalidate_deleted_medias(state: &MediaState, medias: &[MediaRecord]) {
        let media_keys = medias
            .iter()
            .map(|media| RedisKeys::media::media::media_info(media.id))
            .collect::<Vec<_>>();
        let dimension_keys = medias
            .iter()
            .map(|media| RedisKeys::media::media::media_dimensions(&media.file_id))
            .collect::<Vec<_>>();

        let _ = tokio::join!(
            state
                .cache_media_info
                .invalidate_batch(&media_keys)
                .timed(metrics_name!("cache_invalidate")),
            state
                .cache_media_dimensions
                .invalidate_batch(&dimension_keys)
                .timed(metrics_name!("cache_invalidate_dimensions")),
            Self::invalidate_media_cursor_ids(state)
                .timed(metrics_name!("cache_invalidate_cursor_ids")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_first_page_is_trimmed_to_requested_size() {
        let page = CursorPage::from_has_more(vec![MediaId(1), MediaId(2), MediaId(3)], false);

        let page = MediaRepo::resize_cached_first_page(page, 2);

        assert_eq!(page.records, vec![MediaId(1), MediaId(2)]);
        assert!(page.has_more);
    }

    #[test]
    fn cached_first_page_preserves_source_has_more() {
        let page = CursorPage::from_has_more(vec![MediaId(1)], true);

        let page = MediaRepo::resize_cached_first_page(page, 32);

        assert_eq!(page.records, vec![MediaId(1)]);
        assert!(page.has_more);
    }
}
