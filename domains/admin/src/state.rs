use std::sync::Arc;

use backup::BackupState;
use common_runtime::TaskManager;
use sea_orm::DatabaseConnection;
use user::UserState;
use visual::VisualState;

use crate::tasks::AdminJobConfig;

/// 管理端协调状态，仅聚合对应业务域状态与共享基础设施。
///
/// 任务状态完全由数据库维护；这里只保存实例标识与运行时配置，
/// 进程重启不会丢失任何任务状态。
pub struct AdminState {
    pub db: DatabaseConnection,
    pub user: Arc<UserState>,
    pub visual: Arc<VisualState>,
    pub backup: Arc<BackupState>,
    pub task_manager: TaskManager,
    /// 当前实例标识，写入任务的 `worker_id` 便于诊断。
    pub(crate) instance_id: Arc<str>,
    /// 后台任务运行时配置。
    pub(crate) jobs: Arc<AdminJobConfig>,
    /// 新任务入队时唤醒本实例的 supervisor，减少轮询延迟。
    pub(crate) job_wake: Arc<tokio::sync::Notify>,
}

impl AdminState {
    /// 聚合各业务域状态与共享基础设施；参数较多故关闭 `too_many_arguments` lint
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: DatabaseConnection,
        user: Arc<UserState>,
        visual: Arc<VisualState>,
        backup: Arc<BackupState>,
        task_manager: TaskManager,
        jobs: AdminJobConfig,
    ) -> Self {
        let instance_id = format!(
            "{}:{}",
            std::process::id(),
            uuid::Uuid::new_v4().as_simple()
        );
        Self {
            db,
            user,
            visual,
            backup,
            task_manager,
            instance_id: instance_id.into(),
            jobs: Arc::new(jobs),
            job_wake: Arc::new(tokio::sync::Notify::new()),
        }
    }
}
