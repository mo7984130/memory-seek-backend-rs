use std::sync::Arc;

use backup::BackupState;
use common_core::error::ContextualError;
use common_redis::Pool;
use common_runtime::TaskManager;
use dashmap::DashMap;
use multi_level_cache::{CacheConfig, MultiLevelCache};
use sea_orm::DatabaseConnection;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use user::UserState;
use visual::VisualState;

/// 管理端协调状态，仅聚合对应业务域状态，不承载业务数据访问逻辑。
pub struct AdminState {
    pub db: DatabaseConnection,
    pub user: Arc<UserState>,
    pub visual: Arc<VisualState>,
    pub backup: Arc<BackupState>,
    pub task_manager: TaskManager,
    pub(crate) task_tokens: DashMap<String, CancellationToken>,
    pub(crate) task_start_lock: Mutex<()>,
    pub(crate) cursor_counts: MultiLevelCache<u64, ContextualError>,
}

impl AdminState {
    pub fn new(
        db: DatabaseConnection,
        user: Arc<UserState>,
        visual: Arc<VisualState>,
        backup: Arc<BackupState>,
        task_manager: TaskManager,
        redis: Pool,
        cache_config: CacheConfig,
    ) -> Self {
        Self {
            db,
            user,
            visual,
            backup,
            task_manager,
            task_tokens: DashMap::new(),
            task_start_lock: Mutex::new(()),
            cursor_counts: MultiLevelCache::new_with_name(
                "admin_cursor_counts",
                redis,
                cache_config,
            ),
        }
    }
}
