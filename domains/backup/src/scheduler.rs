use std::sync::Arc;

use common_runtime::TaskSchedule;
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};

use crate::service::BackupService;
use crate::state::BackupState;

/// 每日定时备份的执行时刻（本地时间 06:00）
const BACKUP_HOUR: u32 = 6;
const BACKUP_MINUTE: u32 = 0;

/// 多实例互斥用的 PostgreSQL advisory lock key。
///
/// 每个实例都会注册同一个定时器，只有抢到锁的实例真正执行备份。
const SCHEDULE_LOCK_KEY: i64 = 0x4D53_4253_4348;

/// 注册每日 06:00（本地时间）的定时备份任务，由 TaskManager 托管并常驻循环执行。
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
                let mut lock = match try_lock(&state).await {
                    Ok(Some(lock)) => lock,
                    Ok(None) => {
                        tracing::info!("其它实例正在执行定时备份，跳过本轮");
                        return;
                    }
                    Err(error) => {
                        tracing::error!(error = %error, "获取定时备份互斥锁失败");
                        return;
                    }
                };

                let result = BackupService::execute_scheduled(state).await;
                if let Err(error) = lock.release().await {
                    // 解锁失败时由 Drop 关闭连接，确保 session 级 advisory lock 被释放。
                    tracing::warn!(error = %error, "释放定时备份互斥锁失败，将关闭连接");
                }
                drop(lock);

                if let Err(error) = result {
                    tracing::error!(error = %error, "定时备份执行失败");
                }
            }
        },
    );
}

/// session 级 advisory lock 的 RAII 包装。
///
/// 任何未显式释放的退出路径（含 panic）都会关闭连接，让 PostgreSQL 自动释放锁；
/// 否则连接会带着锁回到池里，导致后续所有实例都无法再执行定时备份。
struct ScheduleLock(Option<PoolConnection<Postgres>>);

impl ScheduleLock {
    /// 显式释放锁；成功后连接可以安全回到池中。
    async fn release(&mut self) -> Result<(), sqlx::Error> {
        let Some(connection) = self.0.as_mut() else {
            return Ok(());
        };
        let unlocked: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
            .bind(SCHEDULE_LOCK_KEY)
            .fetch_one(&mut **connection)
            .await?;
        if !unlocked {
            tracing::warn!("定时备份互斥锁已不属于当前会话");
        }
        self.0 = None;
        Ok(())
    }
}

impl Drop for ScheduleLock {
    fn drop(&mut self) {
        if let Some(connection) = self.0.as_mut() {
            connection.close_on_drop();
        }
    }
}

/// 尝试获取 session 级 advisory lock；未抢到返回 `None`。
async fn try_lock(state: &BackupState) -> Result<Option<ScheduleLock>, sqlx::Error> {
    let pool: &PgPool = state.db.get_postgres_connection_pool();
    let mut connection = pool.acquire().await?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(SCHEDULE_LOCK_KEY)
        .fetch_one(&mut *connection)
        .await?;
    Ok(acquired.then_some(ScheduleLock(Some(connection))))
}
