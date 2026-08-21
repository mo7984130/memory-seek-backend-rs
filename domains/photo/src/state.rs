use std::sync::Arc;
#[cfg(feature = "face")]
use std::sync::Mutex;

use common::models::CursorPage;
use deadpool_redis::Pool;
use multi_level_cache::CacheConfig;
use multi_level_cache::MultiLevelCache;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

#[cfg(feature = "face")]
use backup::BackupState;

use common::error::ContextualError;
use types::photo::dto::timeline_stat::MonthStat;
use types::photo::photo::{PhotoId, PhotoRecord};

#[cfg(feature = "face")]
use crate::models::PersonBriefRow;

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct CachedPhotoLike {
    pub(crate) photo_id: PhotoId,
    pub(crate) is_liked: bool,
}

#[allow(dead_code)]
pub struct PhotoState {
    pub(crate) db: DatabaseConnection,
    pub(crate) cache_photo_info: MultiLevelCache<PhotoRecord, ContextualError>,
    pub(crate) cache_photo_like: MultiLevelCache<CachedPhotoLike, ContextualError>,
    pub(crate) cache_photo_cursor_ids: MultiLevelCache<CursorPage<PhotoId, ()>, ContextualError>,
    pub(crate) cache_photo_dimensions: MultiLevelCache<(i32, i32), ContextualError>,
    pub(crate) cache_timeline_stat: MultiLevelCache<Vec<MonthStat>, ContextualError>,
    #[cfg(feature = "face")]
    pub(crate) cache_person: MultiLevelCache<PersonBriefRow, ContextualError>,
    pub redis: Pool,
    pub s3_client: Arc<S3Client>,
    #[cfg(feature = "face")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
    #[cfg(feature = "face")]
    pub backup_state: Arc<BackupState>,
}

impl PhotoState {
    /// 组装照片域所需的仓储, 对象存储和备份组件.
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_config: CacheConfig,
        s3_client: Arc<S3Client>,
        #[cfg(feature = "face")] face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
        #[cfg(feature = "face")] backup_state: Arc<BackupState>,
    ) -> Self {
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
            cache_photo_dimensions: MultiLevelCache::new_with_name(
                "photo_dimensions",
                redis.clone(),
                cache_config,
            ),
            cache_timeline_stat: MultiLevelCache::new_with_name(
                "timeline_stat",
                redis.clone(),
                cache_config,
            ),
            #[cfg(feature = "face")]
            cache_person: MultiLevelCache::new_with_name("person", redis.clone(), cache_config),
            redis,
            s3_client,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_state,
        }
    }
}
