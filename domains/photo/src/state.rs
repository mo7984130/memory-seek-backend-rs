use std::sync::Arc;
#[cfg(feature = "face")]
use std::sync::Mutex;

use common::error::DeferredError;
use deadpool_redis::Pool;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use oss::S3Client;
use sea_orm::DatabaseConnection;

#[cfg(feature = "face")]
use backup::storage::BackupStorage;

use types::photo::dto::timeline_stat::MonthStat;
use types::photo::photo::PhotoRecord;

#[cfg(feature = "face")]
use crate::models::PersonBriefRow;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    /// 照片信息三级缓存（本地 moka → Redis → 数据库）
    pub cache_photo_info: MultiLevelCache<PhotoRecord, DeferredError>,
    /// 时间线月度统计三级缓存（整表一条）
    pub cache_timeline_stat: MultiLevelCache<Vec<MonthStat>, DeferredError>,
    /// 照片尺寸三级缓存（按 file_id）
    pub cache_photo_dimensions: MultiLevelCache<(i32, i32), DeferredError>,
    /// 照片 MD5 去重三级缓存（按 md5）
    pub cache_photo_md5: MultiLevelCache<bool, DeferredError>,
    /// 人物轻量摘要三级缓存（face-engine feature）
    #[cfg(feature = "face")]
    pub cache_person: MultiLevelCache<PersonBriefRow, DeferredError>,
    pub s3_client: Arc<S3Client>,
    #[cfg(feature = "face")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
    #[cfg(feature = "face")]
    pub backup_storage: BackupStorage,
}

impl PhotoState {
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_config: CacheConfig,
        s3_client: Arc<S3Client>,
        #[cfg(feature = "face")] face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
        #[cfg(feature = "face")] backup_storage: BackupStorage,
    ) -> Self {
        let cache_photo_info =
            MultiLevelCache::new_with_name("photo_info", redis.clone(), cache_config);
        let cache_timeline_stat =
            MultiLevelCache::new_with_name("timeline_stat", redis.clone(), cache_config);
        let cache_photo_dimensions =
            MultiLevelCache::new_with_name("photo_dimensions", redis.clone(), cache_config);
        let cache_photo_md5 =
            MultiLevelCache::new_with_name("photo_md5", redis.clone(), cache_config);
        #[cfg(feature = "face")]
        let cache_person = MultiLevelCache::new_with_name("person", redis.clone(), cache_config);
        Self {
            db,
            redis,
            cache_photo_info,
            cache_timeline_stat,
            cache_photo_dimensions,
            cache_photo_md5,
            #[cfg(feature = "face")]
            cache_person,
            s3_client,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_storage,
        }
    }
}
