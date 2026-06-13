use crate::config::AppConfig;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tracing::info;

pub async fn init(cfg: &AppConfig) -> anyhow::Result<DatabaseConnection> {
    info!("初始化数据库");
    let mut opt = ConnectOptions::new(&cfg.database.url);
    opt.max_connections(cfg.database.max_connections);
    let db = Database::connect(opt).await?;
    info!(
        "数据库连接成功, max_connections: {}",
        cfg.database.max_connections
    );
    Ok(db)
}
