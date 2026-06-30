use tracing::info;

use crate::config::BackupConfig;

/// Scheduler for periodic backup execution
pub struct BackupScheduler {
    config: BackupConfig,
}

impl BackupScheduler {
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }

    pub fn start(&self) {
        info!(
            schedule = %self.config.schedule,
            "Backup scheduler started"
        );
    }
}
