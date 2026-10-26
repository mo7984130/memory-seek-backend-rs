use std::sync::Arc;

use common_runtime::TaskSchedule;

use crate::service::BackupService;
use crate::state::BackupState;

/// 每日定时备份的执行时刻（本地时间 06:00）
const BACKUP_HOUR: u32 = 6;
const BACKUP_MINUTE: u32 = 0;

/// 注册每日 06:00（本地时间）的定时备份任务，由 TaskManager 托管并常驻循环执行
pub fn register_scheduled(state: Arc<BackupState>) {
    let task_manager = state.task_manager.clone();
    let backup_state = state.clone();

    task_manager.spawn_schedule(
        "backup_scheduler",
        TaskSchedule::Daily {
            hour: BACKUP_HOUR,
            minute: BACKUP_MINUTE,
        },
        move || {
            let state = backup_state.clone();
            async move {
                if let Err(error) = BackupService::execute_scheduled(state).await {
                    tracing::error!(error = %error, "定时备份执行失败");
                }
            }
        },
    );
}
