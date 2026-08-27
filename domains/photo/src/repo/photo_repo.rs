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
use types::photo::ImageDimensions;
use types::photo::dto::photo::{PageDirection, PhotoCursorParam};
use types::photo::photo::{ActiveModel, Model, NewPhotoRecord, PhotoId, PhotoRecord};

use crate::{
    mappers::{photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper},
    state::{CachedPhotoLike, PhotoState},
};

const PHOTO_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// 首屏缓存统一保存 API 允许的最大页，避免 `size` 不在缓存键中造成结果串页。
const PHOTO_CURSOR_CACHE_SIZE: u64 = 1024;

/// 照片领域数据访问仓储，统一封装数据库与多级缓存。
pub struct PhotoRepo;

impl PhotoRepo {
    /// 加载照片记录.
    /// 返回照片记录 和 是否被喜欢的照片Id
    pub async fn load_photo_records(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<(Vec<Option<PhotoRecord>>, HashSet<PhotoId>)> {
        let (photos, cached_photo_likes) = tokio::join!(
            // 获取照片记录
            state.cache_photo_info.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_info(*id),
                PHOTO_CACHE_TTL,
                |miss_ids| async move { PhotoMapper::query_by_ids(&state.db, &miss_ids).await },
                |photo| photo.id,
            ),
            // 获取是否被喜欢
            state.cache_photo_like.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_is_liked(user_id, *id),
                PHOTO_CACHE_TTL,
                |miss_ids| async move {
                    let liked_photo_ids =
                        PhotoLikeMapper::query_is_like_by_photo_ids(&state.db, user_id, &miss_ids)
                            .await?;
                    miss_ids
                        .into_iter()
                        .map(|photo_id| CachedPhotoLike {
                            is_liked: liked_photo_ids.contains(&photo_id),
                            photo_id,
                        })
                        .collect::<Vec<_>>()
                        .to_ok()
                },
                |cached| cached.photo_id,
            ),
        );
        let photos = photos?;
        let liked_photo_ids = cached_photo_likes?
            .into_iter()
            .zip(photo_ids)
            .filter_map(|(cached, &photo_id)| {
                cached.and_then(|cached| cached.is_liked.then_some(photo_id))
            })
            .collect();
        Ok((photos, liked_photo_ids))
    }

    /// 缓存照片喜欢状态.
    pub(super) async fn cache_photo_like_status(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
        is_liked: bool,
    ) {
        let key = RedisKeys::photo::photo::photo_is_liked(user_id, photo_id);
        state
            .cache_photo_like
            .put(
                &key,
                CachedPhotoLike { photo_id, is_liked },
                PHOTO_CACHE_TTL,
            )
            .await
            .into_contextual()
            .emit_if_err();
    }

    /// 照片聚合字段变更后失效详情缓存。
    pub(super) async fn invalidate_photo_info(state: &PhotoState, photo_id: PhotoId) {
        let key = RedisKeys::photo::photo::photo_info(photo_id);
        state
            .cache_photo_info
            .invalidate(&key)
            .await
            .into_contextual()
            .emit_if_err();
    }

    /// 游标查询照片id.
    pub async fn query_photo_cursor_ids(
        state: &PhotoState,
        req: PhotoCursorParam,
    ) -> Result<CursorPage<PhotoId, ()>> {
        let photo_ids = if req.cursor.is_none() && req.anchor_time.is_none() {
            let key = RedisKeys::photo::photo::photo_cursor_page_ids(req.direction);
            let direction = req.direction;
            let size = req.size;
            let page = state
                .cache_photo_cursor_ids
                .get_or_load(key, PHOTO_CACHE_TTL, || async move {
                    PhotoMapper::query_cursor_page_ids(
                        &state.db,
                        None,
                        PHOTO_CURSOR_CACHE_SIZE,
                        direction,
                        None,
                    )
                    .await
                })
                .await?;
            Self::resize_cached_first_page(page, size)
        } else {
            PhotoMapper::query_cursor_page_ids(
                &state.db,
                req.cursor,
                req.size,
                req.direction,
                req.anchor_time,
            )
            .await?
        };

        Ok(photo_ids)
    }

    /// 失效照片游标 ID 缓存.
    async fn invalidate_photo_cursor_ids(state: &PhotoState) {
        let keys = [
            RedisKeys::photo::photo::photo_cursor_page_ids(PageDirection::Next).to_owned(),
            RedisKeys::photo::photo::photo_cursor_page_ids(PageDirection::Prev).to_owned(),
        ];
        state
            .cache_photo_cursor_ids
            .invalidate_batch(&keys)
            .await
            .into_contextual()
            .emit_if_err();
    }

    fn resize_cached_first_page(
        mut page: CursorPage<PhotoId, ()>,
        size: u64,
    ) -> CursorPage<PhotoId, ()> {
        if page.records.len() > size as usize {
            page.records.truncate(size as usize);
            page.has_more = true;
        }
        page
    }

    /// 插入照片.
    pub async fn insert_photo(state: &PhotoState, photo: NewPhotoRecord) -> Result<Model> {
        db_transaction!(contextual & state.db, |txn| {
            let photo: ActiveModel = photo.into();
            let photo = photo.insert(txn).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("upload")
                    .with_actor(photo.user_id.0)
                    .with_target("photo", photo.id.0),
            )
            .await?;
            Ok(photo)
        })
        .await
    }

    pub async fn ensure_exist(state: &PhotoState, photo_id: PhotoId) -> Result<()> {
        PhotoMapper::ensure_exist(&state.db, photo_id).await
    }

    /// 批量查询图片 MD5 是否存在.
    pub async fn exists_by_md5_batch(state: &PhotoState, md5s: &[String]) -> Result<Vec<bool>> {
        let existing = PhotoMapper::exists_by_md5_batch(&state.db, md5s).await?;
        Ok(md5s.iter().map(|md5| existing.contains(md5)).collect())
    }

    /// 查询单个图片 MD5 是否存在.
    pub async fn exists_by_md5(state: &PhotoState, md5: &str) -> Result<bool> {
        PhotoMapper::exists_by_md5(&state.db, md5).await
    }

    /// 处理照片上传完成后的照片域缓存更新.
    pub async fn after_photo_upload(state: &PhotoState) {
        Self::invalidate_photo_cursor_ids(state)
            .timed(metrics_name!("cache_invalidate"))
            .await;
    }

    /// 通过file_id 获取对应的图片尺寸.
    pub async fn get_photo_dimensions(
        state: &PhotoState,
        file_id: &str,
    ) -> Result<ImageDimensions> {
        let key = RedisKeys::photo::photo::photo_dimensions(file_id);
        state
            .cache_photo_dimensions
            .get_or_load(key.as_str(), PHOTO_CACHE_TTL, || async move {
                PhotoMapper::query_dimensions_by_file_id(&state.db, file_id)
                    .await?
                    .ok_or_warn(
                        "photo_not_found",
                        "裁剪图片不存在",
                        AppError::bad_request("照片不存在"),
                    )
            })
            .timed(metrics_name!("cache_get_or_load"))
            .await
            .map(|dimensions| ImageDimensions {
                width: dimensions.0,
                height: dimensions.1,
            })
    }

    /// 失效照片删除后受影响的照片和人物缓存.
    pub async fn invalidate_deleted_photos(state: &PhotoState, photos: &[PhotoRecord]) {
        let photo_keys = photos
            .iter()
            .map(|photo| RedisKeys::photo::photo::photo_info(photo.id))
            .collect::<Vec<_>>();
        let dimension_keys = photos
            .iter()
            .map(|photo| RedisKeys::photo::photo::photo_dimensions(&photo.file_id))
            .collect::<Vec<_>>();

        let _ = tokio::join!(
            state.cache_photo_info.invalidate_batch(&photo_keys),
            state
                .cache_photo_dimensions
                .invalidate_batch(&dimension_keys),
            Self::invalidate_photo_cursor_ids(state),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_first_page_is_trimmed_to_requested_size() {
        let page = CursorPage::from_has_more(vec![PhotoId(1), PhotoId(2), PhotoId(3)], false);

        let page = PhotoRepo::resize_cached_first_page(page, 2);

        assert_eq!(page.records, vec![PhotoId(1), PhotoId(2)]);
        assert!(page.has_more);
    }

    #[test]
    fn cached_first_page_preserves_source_has_more() {
        let page = CursorPage::from_has_more(vec![PhotoId(1)], true);

        let page = PhotoRepo::resize_cached_first_page(page, 32);

        assert_eq!(page.records, vec![PhotoId(1)]);
        assert!(page.has_more);
    }
}
