use std::sync::Arc;

use common::tokio::TaskSchedule;

use crate::service::BackupService;
use crate::state::BackupState;

/// 每日定时备份的执行时刻（本地时间 06:00）
const BACKUP_HOUR: u32 = 6;
const BACKUP_MINUTE: u32 = 0;

/// 注册每日凌晨 6 点（本地时间）的定时备份任务，由 TaskManager 托管。
///
/// 任务常驻循环：等待到下一个触发点执行一次备份，然后继续等待下一次，
/// 直到应用关闭时随 TaskManager 一起被取消。
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
