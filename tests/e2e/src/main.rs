use clap::Parser;
use deadpool_redis::{Config as RedisConfig, PoolConfig, Runtime};
use e2e::{config::E2eConfig, context::Context, preprea};
use memseek_test::{
    Report,
    ctxlibs::http_client::Client,
    manager::{ManagerConfig, ScenarioManager},
};
use sea_orm::Database;
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

    // 初始化全局 token 加密器: 必须与 server 的 token_cipher 一致,
    // 否则响应中的 avatar_token(ImageTokenStr)无法解密, 反序列化会失败
    common::utils::init_token_cipher(&common::utils::TokenCipherConfig {
        key: cfg.token_cipher.key.clone(),
        salt: cfg.token_cipher.salt.clone(),
    });

    let mut redis_cfg = RedisConfig::from_url(&cfg.redis.url);
    redis_cfg.pool = Some(PoolConfig::new(16));
    let redis = redis_cfg.create_pool(Some(Runtime::Tokio1))?;

    let ctx = Context {
        client: Client::new(cfg.base_url())?,
        db: Database::connect(&cfg.database.url).await?,
        mailhog: Client::new(cfg.mailhog.url)?,
        redis,
        s3: oss::S3Client::new(&cfg.s3.to_oss()),
    };
    // 前置准备: 灌入种子数据(先清空再灌入)
    let ctx = preprea::init(ctx, &cfg.seed).await?;

    // 并发由 Manager 统一管理; 功能验证由各场景自己的 Times(n) 决定
    let manager = ScenarioManager::new(ManagerConfig::new(32).install_ctrl_c());
    let reports = manager.run_all(&ctx).await;
    for report in &reports {
        println!("{}", report.report_with_color());
    }

    Ok(())
}
