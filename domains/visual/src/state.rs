use std::sync::Arc;

use common::tokio::TaskManager;
use common::{Pool, types::CursorPage};
use multi_level_cache::CacheConfig;
use multi_level_cache::MultiLevelCache;
use oss::S3Client;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(feature = "face")]
use backup::BackupState;

use common::error::ContextualError;
use types::visual::dto::timeline_stat::MonthStat;
use types::visual::visual::{VisualId, VisualRecord};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct CachedVisualLike {
    pub(crate) visual_id: VisualId,
    pub(crate) is_liked: bool,
}

#[allow(dead_code)]
pub struct VisualState {
    pub(crate) db: DatabaseConnection,
    pub(crate) cache_visual_info: MultiLevelCache<VisualRecord, ContextualError>,
    pub(crate) cache_visual_like: MultiLevelCache<CachedVisualLike, ContextualError>,
    pub(crate) cache_visual_cursor_ids: MultiLevelCache<CursorPage<VisualId, ()>, ContextualError>,
    pub(crate) cache_visual_dimensions: MultiLevelCache<(u32, u32), ContextualError>,
    pub(crate) cache_timeline_stat: MultiLevelCache<Vec<MonthStat>, ContextualError>,
    pub redis: Pool,
    pub s3_client: S3Client,
    /// 后台任务托管(人脸检测常驻 worker 等)
    pub(crate) task_manager: TaskManager,
    /// 上传接口并发信号量(按 CPU 核数), 控制同时落盘/校验的上传请求数
    pub(crate) upload_semaphore: Arc<tokio::sync::Semaphore>,
    /// 上传落盘临时目录
    pub(crate) tmp_dir: PathBuf,
    #[cfg(feature = "face")]
    pub face_engine: Arc<insight_face_rs::FaceEngine>,
    #[cfg(feature = "face")]
    pub backup_state: Arc<BackupState>,
}

impl VisualState {
    /// 组装影像域所需的仓储, 对象存储和备份组件.
    pub fn new(
        db: DatabaseConnection,
        redis: Pool,
        cache_config: CacheConfig,
        s3_client: S3Client,
        tmp_dir: PathBuf,
        #[cfg(feature = "face")] face_engine: Arc<insight_face_rs::FaceEngine>,
        #[cfg(feature = "face")] backup_state: Arc<BackupState>,
        task_manager: TaskManager,
    ) -> Arc<Self> {
        let state = Arc::new(Self {
            db,
            cache_visual_info: MultiLevelCache::new_with_name(
                "visual_info",
                redis.clone(),
                cache_config,
            ),
            cache_visual_like: MultiLevelCache::new_with_name(
                "visual_like",
                redis.clone(),
                cache_config,
            ),
            cache_visual_cursor_ids: MultiLevelCache::new_with_name(
                "visual_cursor_ids",
                redis.clone(),
                cache_config,
            ),
            cache_visual_dimensions: MultiLevelCache::new_with_name(
                "visual_dimensions",
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
            task_manager,
            // 与密码校验的信号量策略一致: 按 CPU 核数默认
            upload_semaphore: Arc::new(tokio::sync::Semaphore::new(
                std::thread::available_parallelism()
                    .expect("获取可用并行数错误")
                    .into(),
            )),
            tmp_dir,
            #[cfg(feature = "face")]
            face_engine,
            #[cfg(feature = "face")]
            backup_state,
        });

        #[cfg(all(feature = "face", feature = "controller"))]
        crate::services::face_service::FaceService::start_face_consumer(Arc::clone(&state));

        state
    }
}
