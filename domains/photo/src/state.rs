use std::sync::Arc;
#[cfg(feature = "face")]
use std::sync::Mutex;
use std::time::Duration;

use common::cache::MultiLevelCache;
use deadpool_redis::Pool;
use oss::S3Client;
use sea_orm::DatabaseConnection;

#[cfg(feature = "face")]
use backup::storage::BackupStorage;

use types::photo::photo::PhotoRecord;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    /// 照片信息三级缓存（本地 moka → Redis → 数据库）
    pub cache_photo_info: MultiLevelCache<PhotoRecord>,
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
        cache_local_capacity: u64,
        cache_local_ttl_secs: u64,
        s3_client: Arc<S3Client>,
        #[cfg(feature = "face")] face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
        #[cfg(feature = "face")] backup_storage: BackupStorage,
    ) -> Self {
        let cache_photo_info = MultiLevelCache::new(
            "photo_info",
            redis.clone(),
            cache_local_capacity,
            Duration::from_secs(cache_local_ttl_secs),
        );
        Self {
            db,
            redis,
            cache_photo_info,
            s3_client,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_storage,
        }
    }
}
