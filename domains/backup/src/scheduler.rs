use crate::error::BackupError;
use crate::runner::BackupRunner;
use crate::state::BackupState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

/// 备份调度器
pub struct BackupScheduler {
    scheduler: Mutex<JobScheduler>,
}

impl BackupScheduler {
    /// 创建新的调度器
    pub async fn new(state: Arc<BackupState>) -> Result<Self, BackupError> {
        let scheduler = JobScheduler::new().await?;

        let schedule = state.config.scheduled.schedule.clone();
        let state_clone = state.clone();

        let job = Job::new(schedule.as_str(), move |_, _| {
            let state = state_clone.clone();
            tokio::spawn(async move {
                let _ = BackupRunner::execute_scheduled(state).await;
            });
        })?;

        scheduler.add(job).await?;

        Ok(Self {
            scheduler: Mutex::new(scheduler),
        })
    }

    /// 启动调度器
    pub async fn start(&self) -> Result<(), BackupError> {
        self.scheduler.lock().await.start().await?;
        tracing::info!("Backup scheduler started");
        Ok(())
    }

    /// 停止调度器
    pub async fn stop(&self) -> Result<(), BackupError> {
        self.scheduler.lock().await.shutdown().await?;
        tracing::info!("Backup scheduler stopped");
        Ok(())
    }
}
