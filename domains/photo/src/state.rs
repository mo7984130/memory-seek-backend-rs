use std::sync::Arc;
#[cfg(feature = "face")]
use std::sync::Mutex;

use deadpool_redis::Pool;
use multi_level_cache::CacheConfig;
use oss::S3Client;
use sea_orm::DatabaseConnection;

#[cfg(feature = "face")]
use backup::storage::BackupStorage;

use crate::repo::{PhotoRepo, TimelineStatRepo};

pub struct PhotoState {
    /// 照片领域数据访问仓储，封装数据库与多级缓存
    pub repo: PhotoRepo,
    /// 时间线统计仓储，封装统计表与月度统计缓存
    pub timeline_stat_repo: TimelineStatRepo,
    pub redis: Pool,
    pub s3_client: Arc<S3Client>,
    #[cfg(feature = "face")]
    pub face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
    #[cfg(feature = "face")]
    pub backup_storage: BackupStorage,
}

impl PhotoState {
    /// 组装照片域所需的仓储, 对象存储和备份组件.
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_config: CacheConfig,
        s3_client: Arc<S3Client>,
        #[cfg(feature = "face")] face_engine: Arc<Mutex<insight_face_rs::FaceEngine>>,
        #[cfg(feature = "face")] backup_storage: BackupStorage,
    ) -> Self {
        Self {
            repo: PhotoRepo::new(db.clone(), redis.clone(), cache_config.clone()),
            timeline_stat_repo: TimelineStatRepo::new(db, redis.clone(), cache_config),
            redis,
            s3_client,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_storage,
        }
    }
}
