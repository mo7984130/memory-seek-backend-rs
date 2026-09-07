use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

use crate::setup::AppSetup;
#[cfg(feature = "backup")]
use crate::setup::domains::backup::BackupRuntime;
use crate::util::MissDepError;

pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    #[cfg(feature = "backup")]
    pub backup_scheduler: backup::BackupScheduler,
}

impl AppState {
    pub fn from_setup(setup: &AppSetup) -> common::Result<Self> {
        Ok(Self {
            db: setup
                .registry
                .get::<DatabaseConnection>()
                .miss_dep("AppState", "DatabaseConnection")?
                .clone(),
            redis: setup
                .registry
                .get::<Pool>()
                .miss_dep("AppState", "RedisPool")?
                .clone(),
            #[cfg(feature = "backup")]
            backup_scheduler: setup
                .registry
                .get::<BackupRuntime>()
                .miss_dep("AppState", "BackupRuntime")?
                .scheduler
                .clone(),
        })
    }
}
