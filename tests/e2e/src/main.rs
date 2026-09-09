use clap::Parser;
use e2e::{config::E2eConfig, context::Context, preprea};
use memseek_test::{
    ctxlibs::http_client::Client,
    manager::{ManagerConfig, ScenarioManager},
};
use sea_orm::Database;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "e2e")]
struct Cli {
    /// 配置文件路径
    #[arg(short = 'c', long = "config")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 日志级别由 RUST_LOG 控制(默认 info), sqlx 固定 warn, 与 server 保持一致
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    fmt()
        .with_env_filter(EnvFilter::new(format!("{log_level},sqlx=warn")))
        .init();

    let cli = Cli::parse();

    // 加载配置(与 server 一致: CLI > E2E_CONFIG_PATH > 默认路径)
    let cfg = E2eConfig::load(cli.config);

    let ctx = Context {
        client: Client::new(cfg.base_url())?,
        db: Database::connect(&cfg.database.url).await?,
        mailhog: Client::new(cfg.mailhog.url)?,
    };
    // 前置准备: 灌入种子数据(先清空再灌入)
    let ctx = preprea::init(ctx, &cfg.seed).await?;

    let report = ScenarioManager::new(ManagerConfig::new(32))
        .run_all(&ctx)
        .await;
    info!("{:#?}", report);

    Ok(())
}
