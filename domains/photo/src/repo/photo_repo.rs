use std::time::Duration;

use common::error::{AppError, ContextualError, contextual::Result};
use common::ext::ContextOptionExt;
use common::metrics_name;
use common::models::CursorPage;
use common::utils::MetricsTimerExt;
use constants::RedisKeys;
use deadpool_redis::Pool;
use futures::future::BoxFuture;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use sea_orm::{ActiveModelTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use types::auth::user::UserId;
use types::photo::dto::{
    photo::{PageDirection, PhotoCursorParam},
    timeline_stat::MonthStat,
};
use types::photo::photo::{ActiveModel, Model, NewPhotoRecord, PhotoId, PhotoRecord};

#[cfg(feature = "face")]
use types::photo::person::PersonId;

use crate::mappers::{
    photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper,
    timeline_stat_mapper::TimelineStatMapper,
};
#[cfg(feature = "face")]
use crate::models::PersonBriefRow;

const PHOTO_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const TIMELINE_STAT_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const PHOTO_CURSOR_CACHE_MAX_SIZE: u64 = 1024;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct CachedPhotoLike {
    pub(super) photo_id: PhotoId,
    pub(super) is_liked: bool,
}

/// 照片领域数据访问仓储，统一封装数据库与多级缓存。
pub struct PhotoRepo {
    pub(super) db: DatabaseConnection,
    cache_photo_info: MultiLevelCache<PhotoRecord, ContextualError>,
    cache_photo_like: MultiLevelCache<CachedPhotoLike, ContextualError>,
    cache_photo_cursor_ids: MultiLevelCache<Vec<PhotoId>, ContextualError>,
    cache_timeline_stat: MultiLevelCache<Vec<MonthStat>, ContextualError>,
    cache_photo_dimensions: MultiLevelCache<(i32, i32), ContextualError>,
    #[cfg(feature = "face")]
    pub(super) cache_person: MultiLevelCache<PersonBriefRow, ContextualError>,
}

impl PhotoRepo {
    /// 为保留在 service 的领域不变量编排提供事务边界；连接本身不向外暴露。
    pub(crate) async fn transaction<T>(
        &self,
        operation: impl for<'a> FnOnce(
            &'a sea_orm::DatabaseTransaction,
        ) -> BoxFuture<'a, std::result::Result<T, AppError>>,
    ) -> Result<T> {
        common::db_transaction!(scoped & self.db, |txn| { operation(txn).await })
            .await
            .map_err(|error| {
                ContextualError::error(
                    "photo_repo_transaction",
                    "执行照片领域事务失败",
                    error.to_string(),
                    error,
                )
            })
    }
    /// 创建照片仓储并初始化数据库, Redis 和缓存配置.
    pub fn new(db: DatabaseConnection, redis: Pool, cache_config: CacheConfig) -> Self {
        Self {
            db,
            cache_photo_info: MultiLevelCache::new_with_name(
                "photo_info",
                redis.clone(),
                cache_config,
            ),
            cache_photo_like: MultiLevelCache::new_with_name(
                "photo_like",
                redis.clone(),
                cache_config,
            ),
            cache_photo_cursor_ids: MultiLevelCache::new_with_name(
                "photo_cursor_ids",
                redis.clone(),
                cache_config,
            ),
            cache_timeline_stat: MultiLevelCache::new_with_name(
                "timeline_stat",
                redis.clone(),
                cache_config,
            ),
            cache_photo_dimensions: MultiLevelCache::new_with_name(
                "photo_dimensions",
                redis.clone(),
                cache_config,
            ),
            #[cfg(feature = "face")]
            cache_person: MultiLevelCache::new_with_name("person", redis, cache_config),
        }
    }

    /// 批量加载用户照片记录及其点赞状态.
    pub async fn load_photo_records(
        &self,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<(Vec<Option<PhotoRecord>>, std::collections::HashSet<PhotoId>)> {
        let (photos, cached_photo_likes) = tokio::join!(
            self.cache_photo_info.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_info(*id),
                PHOTO_CACHE_TTL,
                |miss_ids| async move { PhotoMapper::query_by_ids(&self.db, &miss_ids).await },
                |photo| photo.id,
            ),
            self.cache_photo_like.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_is_liked(user_id, *id),
                PHOTO_CACHE_TTL,
                |miss_ids| async move {
                    let liked_photo_ids =
                        PhotoLikeMapper::query_is_like_by_photo_ids(&self.db, user_id, &miss_ids)
                            .await?;
                    Ok(miss_ids
                        .into_iter()
                        .map(|photo_id| CachedPhotoLike {
                            is_liked: liked_photo_ids.contains(&photo_id),
                            photo_id,
                        })
                        .collect())
                },
                |cached| cached.photo_id,
            ),
        );
        let liked_photo_ids = cached_photo_likes?
            .into_iter()
            .zip(photo_ids)
            .filter_map(|(cached, &photo_id)| {
                cached.and_then(|cached| cached.is_liked.then_some(photo_id))
            })
            .collect();
        Ok((photos?, liked_photo_ids))
    }

    /// 将照片点赞状态写入本地和远端缓存.
    pub(super) async fn cache_photo_like_status(
        &self,
        user_id: UserId,
        photo_id: PhotoId,
        is_liked: bool,
    ) {
        let key = RedisKeys::photo::photo::photo_is_liked(user_id, photo_id);
        let _ = self
            .cache_photo_like
            .put(
                &key,
                CachedPhotoLike { photo_id, is_liked },
                PHOTO_CACHE_TTL,
            )
            .await;
    }

    /// 查询用户照片的游标分页 ID.
    pub async fn query_photo_cursor_ids(
        &self,
        req: PhotoCursorParam,
    ) -> Result<CursorPage<PhotoId, ()>> {
        if req.cursor.is_none() && req.anchor_time.is_none() {
            let key = RedisKeys::photo::photo::photo_cursor_page_ids(req.direction);
            let photo_ids = self
                .cache_photo_cursor_ids
                .get_or_load(key, PHOTO_CACHE_TTL, || async move {
                    PhotoMapper::query_cursor_page_ids(
                        &self.db,
                        None,
                        PHOTO_CURSOR_CACHE_MAX_SIZE,
                        req.direction,
                        None,
                    )
                    .await
                })
                .await?;
            return Ok(CursorPage::from_oversize(
                photo_ids
                    .into_iter()
                    .take((req.size + 1) as usize)
                    .collect(),
                req.size,
            ));
        }

        Ok(CursorPage::from_oversize(
            PhotoMapper::query_cursor_page_ids(
                &self.db,
                req.cursor,
                req.size,
                req.direction,
                req.anchor_time,
            )
            .await?,
            req.size,
        ))
    }

    /// 失效照片游标 ID 缓存.
    async fn invalidate_photo_cursor_ids(&self) {
        let keys = [
            RedisKeys::photo::photo::photo_cursor_page_ids(PageDirection::Next).to_owned(),
            RedisKeys::photo::photo::photo_cursor_page_ids(PageDirection::Prev).to_owned(),
        ];
        let _ = self.cache_photo_cursor_ids.invalidate_batch(&keys).await;
    }

    /// 插入照片主记录.
    pub async fn insert_photo(&self, photo: NewPhotoRecord) -> Result<Model> {
        let photo: ActiveModel = photo.into();
        photo.insert(&self.db).await.map_err(Into::into)
    }

    /// 批量查询图片 MD5 是否存在.
    pub async fn exists_by_md5_batch(&self, md5s: &[String]) -> Result<Vec<bool>> {
        let existing = PhotoMapper::exists_by_md5_batch(&self.db, md5s).await?;
        Ok(md5s.iter().map(|md5| existing.contains(md5)).collect())
    }

    /// 在单个事务中执行照片删除管道.
    pub(crate) async fn delete_photos(
        &self,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<PhotoDeleteContext> {
        let photos = PhotoMapper::query_by_user_id_and_ids(&self.db, user_id, photo_ids).await?;
        let mut ctx = PhotoDeleteContext {
            photos,
            #[cfg(feature = "face")]
            affected_person_ids: Vec::new(),
        };
        PHOTO_DELETE_PIPELINE
            .run(&self.db, &mut ctx)
            .await
            .map_err(|error| {
                ContextualError::error(
                    "photo_delete_pipeline",
                    "执行照片删除事务失败",
                    error.to_string(),
                    error,
                )
            })?;
        Ok(ctx)
    }

    /// 查询单个图片 MD5 是否存在.
    pub async fn exists_by_md5(&self, md5: &str) -> Result<bool> {
        PhotoMapper::exists_by_md5(&self.db, md5).await
    }

    /// 记录新上传照片对应月份的时间线统计.
    pub async fn record_uploaded_photo(
        &self,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        TimelineStatMapper::incr_stat(&self.db, created_at).await?;
        let _ = self
            .cache_timeline_stat
            .invalidate(RedisKeys::photo::timeline_stat::monthly_stats())
            .timed(metrics_name!("cache_invalidate"))
            .await;
        Ok(())
    }

    /// 失效照片上传后受影响的游标缓存.
    pub async fn invalidate_uploaded_photo_cursor(&self) {
        self.invalidate_photo_cursor_ids()
            .timed(metrics_name!("cache_invalidate"))
            .await;
    }

    /// 查询对象存储文件对应的图片尺寸.
    pub async fn get_photo_dimensions(&self, file_id: &str) -> Result<(i32, i32)> {
        let key = RedisKeys::photo::photo::photo_dimensions(file_id);
        self.cache_photo_dimensions
            .get_or_load(key.as_str(), PHOTO_CACHE_TTL, || async move {
                PhotoMapper::query_dimensions_by_file_id(&self.db, file_id)
                    .await?
                    .context_warn_none(
                        "photo_not_found",
                        "裁剪图片不存在",
                        AppError::bad_request("照片不存在"),
                    )
            })
            .timed(metrics_name!("cache_get_or_load"))
            .await
    }

    /// 获取带缓存的月度照片统计.
    pub async fn get_monthly_stats(&self) -> Result<Vec<MonthStat>> {
        self.cache_timeline_stat
            .get_or_load(
                RedisKeys::photo::timeline_stat::monthly_stats(),
                TIMELINE_STAT_CACHE_TTL,
                || async move { TimelineStatMapper::query_monthly_stats(&self.db).await },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await
    }

    /// 失效照片删除后受影响的照片, 人物和统计缓存.
    pub async fn invalidate_deleted_photos(
        &self,
        photos: &[PhotoRecord],
        #[cfg(feature = "face")] affected_person_ids: &[PersonId],
    ) {
        let photo_keys = photos
            .iter()
            .map(|photo| RedisKeys::photo::photo::photo_info(photo.id))
            .collect::<Vec<_>>();
        let dimension_keys = photos
            .iter()
            .map(|photo| RedisKeys::photo::photo::photo_dimensions(&photo.file_id))
            .collect::<Vec<_>>();

        #[cfg(feature = "face")]
        let person_keys = affected_person_ids
            .iter()
            .map(|&id| RedisKeys::photo::person::person_info(id))
            .collect::<Vec<_>>();

        #[cfg(feature = "face")]
        let _ = tokio::join!(
            self.cache_photo_info.invalidate_batch(&photo_keys),
            self.cache_photo_dimensions
                .invalidate_batch(&dimension_keys),
            self.cache_timeline_stat
                .invalidate(RedisKeys::photo::timeline_stat::monthly_stats()),
            self.invalidate_photo_cursor_ids(),
            self.cache_person.invalidate_batch(&person_keys),
        );
        #[cfg(not(feature = "face"))]
        let _ = tokio::join!(
            self.cache_photo_info.invalidate_batch(&photo_keys),
            self.cache_photo_dimensions
                .invalidate_batch(&dimension_keys),
            self.cache_timeline_stat
                .invalidate(RedisKeys::photo::timeline_stat::monthly_stats()),
            self.invalidate_photo_cursor_ids(),
        );
    }
}

/// 照片删除步骤共享上下文，由 repo 查询并鉴权后在单个事务管道内消费。
pub(crate) struct PhotoDeleteContext {
    pub photos: Vec<PhotoRecord>,
    #[cfg(feature = "face")]
    /// 删除人脸步骤更新过统计的人物 ID，供事务提交后失效缓存。
    pub affected_person_ids: Vec<PersonId>,
}

impl PhotoDeleteContext {
    /// 返回当前删除管道中的照片 ID.
    pub fn photo_ids(&self) -> Vec<PhotoId> {
        self.photos.iter().map(|photo| photo.id).collect()
    }
}

step_derive::declare_pipeline!(
    PhotoDeleteContext,
    PHOTO_DELETE_STEPS,
    PHOTO_DELETE_PIPELINE
);
