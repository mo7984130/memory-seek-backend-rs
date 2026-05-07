use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

use ::auth::client::{AuthClient, TokenStore};
use benches::config::BenchConfig;
use benches::runner::Runner;
use benches::scenarios::auth::{LoginScenario, RegisterScenario, TokenRefreshScenario};
use benches::scenarios::user::GetProfileScenario;
use benches::scenarios::WeightedScenario;

#[derive(Parser)]
#[command(name = "stress", about = "HTTP stress testing tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to config file
    #[arg(short, long, default_value = "benches/config/bench.toml", global = true)]
    config: String,

    /// Override concurrency
    #[arg(short = 'n', long, global = true)]
    concurrency: Option<usize>,

    /// Override duration in seconds
    #[arg(short, long, global = true)]
    duration: Option<u64>,

    /// Export results (json, csv)
    #[arg(long, value_delimiter = ',', global = true)]
    export: Option<Vec<String>>,

    /// Output directory for exported results
    #[arg(long, default_value = "benches/results", global = true)]
    output_dir: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 一键初始化所有测试数据（数据库 + Redis）
    Seed,
    /// 写入 100 个测试用户到 auth_user 表
    InsertTestUsers,
    /// 设置邀请码到 Redis（默认 user_id=1）
    SetInviterCode {
        /// 邀请码对应的用户 ID
        #[arg(long, default_value = "1")]
        user_id: i64,
    },
    /// 批量设置邮箱验证码到 Redis（注册场景用）
    SetEmailCodes,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            dotenvy::from_filename_override(".env.test").ok();
            match cmd {
                Commands::Seed => benches::seed::init_all().await?,
                Commands::InsertTestUsers => benches::seed::insert_test_users().await?,
                Commands::SetInviterCode { user_id } => benches::seed::set_inviter_code(user_id)?,
                Commands::SetEmailCodes => benches::seed::set_email_verify_codes()?,
            }
            return Ok(());
        }
        None => {}
    }

    dotenvy::dotenv().ok();

    let mut config = BenchConfig::load(&cli.config)?;

    if let Some(c) = cli.concurrency {
        config.bench.concurrency = c;
    }
    if let Some(d) = cli.duration {
        config.bench.duration_secs = d;
    }

    let token_store = Arc::new(TokenStore::new(&config.server.base_url, config.auth.refresh_before_expiry_secs));
    let client = Arc::new(AuthClient::new(&config.server.base_url, token_store.clone()));

    // Warmup: login all users to populate token store
    let credentials: Vec<(&str, &str)> = config.users.iter().map(|u| (u.account.as_str(), u.password.as_str())).collect();
    let user_ids = token_store.warmup(&credentials).await?;

    // Build account -> user_id mapping
    let account_to_id: HashMap<&str, i64> = config
        .users
        .iter()
        .zip(user_ids.iter())
        .map(|(u, &id)| (u.account.as_str(), id))
        .collect();

    // Allocate users to scenarios based on weights
    let allocated = config.allocate_users();

    // Build scenarios based on config
    let mut scenarios: Vec<WeightedScenario> = Vec::new();
    for sw in &config.scenarios {
        let scenario: Arc<dyn benches::scenarios::Scenario> = match sw.name.as_str() {
            "auth/login" => {
                let users = allocated.get(&sw.name).cloned().unwrap_or_default();
                Arc::new(LoginScenario::new(users))
            }
            "auth/token_refresh" => {
                let users = allocated.get(&sw.name).cloned().unwrap_or_default();
                let ids: Vec<i64> = users
                    .iter()
                    .filter_map(|u| account_to_id.get(u.account.as_str()).copied())
                    .collect();
                Arc::new(TokenRefreshScenario::new(ids, token_store.clone()))
            }
            "auth/register" => Arc::new(RegisterScenario::new("bench_user")),
            "user/get_profile" => {
                let users = allocated.get(&sw.name).cloned().unwrap_or_default();
                Arc::new(GetProfileScenario::new(users))
            }
            _ => {
                eprintln!("Unknown scenario: {}", sw.name);
                continue;
            }
        };
        scenarios.push(WeightedScenario {
            scenario,
            weight: sw.weight,
        });
    }

    if scenarios.is_empty() {
        anyhow::bail!("No scenarios configured");
    }

    let run_mode = config.bench.mode.clone();
    let duration_secs = config.bench.duration_secs;
    let staircase_config = config.bench.staircase.clone();
    let runner = Runner::new(config, client, token_store);

    match run_mode {
        benches::config::RunMode::Fixed => {
            runner.run(scenarios).await?;
        }
        benches::config::RunMode::Staircase => {
            let staircase = staircase_config
                .ok_or_else(|| anyhow::anyhow!("Staircase config required when mode=staircase"))?;
            let results = runner.run_staircase(scenarios, staircase).await?;

            // Export if requested
            if let Some(formats) = &cli.export {
                benches::exporter::export_results(
                    &results,
                    duration_secs,
                    &cli.output_dir,
                    formats,
                )?;
            }
        }
    }

    Ok(())
}
