use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use crate::config::AppConfig;

pub async fn init(cfg: &AppConfig) -> anyhow::Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(&cfg.database.url);
    opt.max_connections(cfg.database.max_connections);
    let db = Database::connect(opt).await?;
    tracing::info!("Database connected (max_connections: {})", cfg.database.max_connections);
    Ok(db)
}
