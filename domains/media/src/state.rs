#[cfg(feature = "face")]
use std::sync::Arc;

use common::{Pool, types::CursorPage};
use multi_level_cache::CacheConfig;
use multi_level_cache::MultiLevelCache;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

#[cfg(feature = "face")]
use backup::BackupState;

use common::error::ContextualError;
use types::media::dto::timeline_stat::MonthStat;
use types::media::media::{MediaId, MediaRecord};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct CachedMediaLike {
    pub(crate) media_id: MediaId,
    pub(crate) is_liked: bool,
}

#[allow(dead_code)]
pub struct MediaState {
    pub(crate) db: DatabaseConnection,
    pub(crate) cache_media_info: MultiLevelCache<MediaRecord, ContextualError>,
    pub(crate) cache_media_like: MultiLevelCache<CachedMediaLike, ContextualError>,
    pub(crate) cache_media_cursor_ids: MultiLevelCache<CursorPage<MediaId, ()>, ContextualError>,
    pub(crate) cache_media_dimensions: MultiLevelCache<(u32, u32), ContextualError>,
    pub(crate) cache_timeline_stat: MultiLevelCache<Vec<MonthStat>, ContextualError>,
    pub redis: Pool,
    pub s3_client: S3Client,
    #[cfg(feature = "face")]
    pub face_engine: Arc<insight_face_rs::FaceEngine>,
    #[cfg(feature = "face")]
    pub backup_state: Arc<BackupState>,
}

impl MediaState {
    /// 组装照片域所需的仓储, 对象存储和备份组件.
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_config: CacheConfig,
        s3_client: S3Client,
        #[cfg(feature = "face")] face_engine: Arc<insight_face_rs::FaceEngine>,
        #[cfg(feature = "face")] backup_state: Arc<BackupState>,
    ) -> Self {
        Self {
            db,
            cache_media_info: MultiLevelCache::new_with_name(
                "media_info",
                redis.clone(),
                cache_config,
            ),
            cache_media_like: MultiLevelCache::new_with_name(
                "media_like",
                redis.clone(),
                cache_config,
            ),
            cache_media_cursor_ids: MultiLevelCache::new_with_name(
                "media_cursor_ids",
                redis.clone(),
                cache_config,
            ),
            cache_media_dimensions: MultiLevelCache::new_with_name(
                "media_dimensions",
                redis.clone(),
                cache_config,
            ),
            cache_timeline_stat: MultiLevelCache::new_with_name(
                "timeline_stat",
                redis.clone(),
                cache_config,
            ),
            redis,
            s3_client,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_state,
        }
    }
}
