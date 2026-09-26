use std::path::PathBuf;

use common_runtime::TaskManager;
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

use crate::setup::AppSetup;
use crate::util::MissDepError;

pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub task_manager: TaskManager,
    /// 上传请求体上限(字节), 供大小预检中间件使用
    pub max_upload_bytes: u64,
    /// 统一临时文件目录, 优雅关闭时删除
    pub tmp_path: PathBuf,
}

impl AppState {
    pub fn from_setup(
        setup: &AppSetup,
        max_upload_bytes: u64,
        tmp_path: PathBuf,
    ) -> common_core::Result<Self> {
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
            max_upload_bytes,
            tmp_path,
        })
    }
}
