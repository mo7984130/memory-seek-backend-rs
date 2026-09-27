use std::io::IsTerminal;
use std::time::Duration;

use clap::Parser;
use deadpool_redis::{Config as RedisConfig, PoolConfig, Runtime};
use e2e::{
    config::E2eConfig,
    context::Context,
    preprea,
    report::{JsonReport, Verdict},
};
use memseek_test::{
    Report, ReportOptions,
    ctxlibs::http_client::{Client, reqwest},
    manager::{ManagerConfig, ScenarioManager},
    registry::{ScenarioRegistration, ScenarioRegistry},
};
use sea_orm::Database;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "e2e")]
struct Cli {
    /// 配置文件路径
    #[arg(short = 'c', long = "config")]
    config: Option<String>,

    /// 运行结果 JSON 报告输出路径(也可用环境变量 `E2E_REPORT_JSON`)
    #[arg(long = "report-json")]
    report_json: Option<String>,
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

    // 运行参数自检: 非法配置直接退出, 不浪费时间跑出一堆无意义结果
    if let Err(reason) = cfg.run.validate() {
        eprintln!("运行参数非法: {reason}");
        std::process::exit(2);
    }
    let scenarios = select_scenarios(&cfg);
    if scenarios.is_empty() {
        eprintln!("没有可执行的场景(检查 run.scenarios 白名单)");
        std::process::exit(2);
    }

    // 初始化全局 token 加密器: 必须与 server 的 token_cipher 一致,
    // 否则响应中的 avatar_token(VisualTokenStr)无法解密, 反序列化会失败
    common_crypto::init_token_cipher(&common_crypto::TokenCipherConfig {
        key: cfg.token_cipher.key.clone(),
        salt: cfg.token_cipher.salt.clone(),
    });

    let mut redis_cfg = RedisConfig::from_url(&cfg.redis.url);
    redis_cfg.pool = Some(PoolConfig::new(16));
    let redis = redis_cfg.create_pool(Some(Runtime::Tokio1))?;

    // 业务请求统一 5s 超时: 快速暴露慢接口与挂死场景
    let client = Client::from_reqwest(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?,
        cfg.base_url(),
    )?;

    let ctx = Context {
        client,
        db: Database::connect(&cfg.database.url).await?,
        mailhog: Client::new(cfg.mailhog.url.clone())?,
        redis,
        s3: oss::S3Client::new(&cfg.s3.to_oss()),
    };
    // 前置准备: 灌入种子数据(先清空再灌入)。
    // 外部被测目标、或连续压测复用同一份种子时用 `run.prepare=false` 跳过
    let prepared = cfg.run.prepare;
    let ctx = if prepared {
        preprea::init(ctx, &cfg.seed).await?
    } else {
        tracing::info!("跳过前置准备(run.prepare = false)");
        ctx
    };

    // 并发与执行模式均由 Manager 统一管理; 全局 mode 覆盖所有场景注册
    let mut manager_config = ManagerConfig::new(cfg.run.concurrency)
        .with_run_mode(cfg.run.run_mode())
        .with_tui()
        .install_ctrl_c();
    if let Some(backoff) = cfg.run.backoff() {
        manager_config = manager_config.with_backoff(backoff);
    }
    let manager = ScenarioManager::new(manager_config);

    match cfg.run.mode {
        e2e::config::RunModeKind::Times => tracing::info!(
            "开始执行: 场景 {} 个, 并发 {}, 轮次 {}",
            scenarios.len(),
            cfg.run.concurrency,
            cfg.run.times
        ),
        e2e::config::RunModeKind::Duration => tracing::info!(
            "开始执行: 场景 {} 个, 并发 {}, 时长 {}s",
            scenarios.len(),
            cfg.run.concurrency,
            cfg.run.duration_secs
        ),
    }

    let reports = manager.run(&ctx, &scenarios).await;

    // 可视化报告(CI 日志无颜色, 便于阅读与抓取)
    println!(
        "{}",
        reports.report_with(ReportOptions {
            color: std::io::stdout().is_terminal(),
            ..ReportOptions::default()
        })
    );

    // 汇总判定: 供 CI 退出码使用(压测报告另由基线比对脚本生成)
    let verdict = Verdict::evaluate(&reports, &cfg.run);
    print!("{}", verdict.summary());

    // JSON 报告: 无论成败都落盘, 便于 CI 归档与基线比对
    if let Some(path) = cli
        .report_json
        .or_else(|| std::env::var("E2E_REPORT_JSON").ok())
    {
        let report = JsonReport::build(&reports, &cfg.run, cfg.base_url(), prepared, &verdict);
        std::fs::write(&path, report.to_json()?)?;
        tracing::info!("JSON 报告已写入: {path}");
    }

    if !verdict.passed {
        std::process::exit(1);
    }
    Ok(())
}

/// 按 `run.scenarios` 白名单挑场景(为空则全部)。
///
/// 白名单里出现未注册的名字时直接退出: 名字拼错不能变成"静默少跑"。
fn select_scenarios(cfg: &E2eConfig) -> Vec<&'static ScenarioRegistration> {
    let all = ScenarioRegistry::scenarios::<Context>();
    let whitelist = cfg.run.scenario_whitelist();
    if whitelist.is_empty() {
        return all;
    }

    let unknown: Vec<&str> = whitelist
        .iter()
        .copied()
        .filter(|name| !all.iter().any(|entry| entry.name == *name))
        .collect();
    if !unknown.is_empty() {
        eprintln!(
            "场景白名单包含未注册的场景: {}\n已注册: {}",
            unknown.join(", "),
            all.iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(2);
    }

    all.into_iter()
        .filter(|entry| whitelist.contains(&entry.name))
        .collect()
}
