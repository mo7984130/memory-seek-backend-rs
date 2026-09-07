use common::tokio::TaskManager;
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

use crate::setup::AppSetup;
use crate::util::MissDepError;

pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub task_manager: TaskManager,
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
            task_manager: setup
                .registry
                .get::<TaskManager>()
                .miss_dep("AppState", "TaskManager")?
                .clone(),
        })
    }
}
