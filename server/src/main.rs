use clap::Parser;
use common::ext::ResultErrExt;
use std::sync::Arc;
use tokio::net::TcpListener;

mod config;
mod metrics;
mod middlewares;
mod setup;
mod state;

use config::AppConfig;
use setup::AppSetup;

/// Memory Seek 后端服务
#[derive(Parser)]
#[command(name = "memory-seek-server")]
struct Cli {
    /// 配置文件路径
    #[arg(short = 'c', long = "config")]
    config: Option<String>,

    /// 日志目录
    #[arg(long = "log-dir")]
    log_dir: Option<String>,

    /// 日志文件名
    #[arg(long = "log-file")]
    log_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), common::error::AppError> {
    let cli = Cli::parse();

    // 提前初始化日志系统，确保配置加载等早期阶段也能记录错误详情
    // 文件写入器生命周期由 _log_guard 持有，进程运行期间持续生效
    let _log_guard = setup::bases::log::init(cli.log_dir, cli.log_file);

    // 加载配置
    let cfg = AppConfig::load(cli.config).trace_internal_err("config_load_err", "加载配置失败")?;

    // 初始化应用（内部会初始化日志、数据库、Redis、metrics 等）
    let app_setup = AppSetup::init(&cfg).await?;

    // 启动后台指标采集
    #[cfg(feature = "metrics")]
    {
        metrics::start_collector(app_setup.state.db.clone(), app_setup.state.redis.clone());
    }

    // 克隆 state 用于优雅关闭（router 会消费 app_setup.state）
    let graceful_state = app_setup.state.clone();

    // 合并路由并添加中间件
    let app = app_setup
        .public_router
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(
            app_setup
                .protected_router
                .layer(axum::middleware::from_fn_with_state(
                    app_setup.state.clone(),
                    middlewares::auth::auth_middleware,
                )),
        )
        .layer(middlewares::cors::layer())
        .layer(axum::middleware::from_fn(
            middlewares::trace_id::trace_id_middleware,
        ))
        .with_state(app_setup.state);

    // 启动服务器
    tracing::info!("尝试监听{}端口", cfg.server.port);
    let listener = TcpListener::bind(&cfg.server_addr())
        .await
        .trace_internal_err("tcp_bind_err", "端口绑定失败")?;
    tracing::info!("Server listening on {}", cfg.server_addr());

    let shutdown_signal = shutdown_signal(graceful_state);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .trace_internal_err("server_err", "服务器运行异常")?;

    Ok(())
}

/// 优雅关闭信号处理：等待 SIGINT/SIGTERM，停止备份调度器
async fn shutdown_signal(_state: Arc<crate::state::AppState>) {
    let sigint = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    tokio::select! {
        _ = sigint => {},
        _ = sigterm => {},
    }

    #[cfg(not(unix))]
    sigint.await;

    tracing::info!("收到关闭信号，开始优雅关闭");

    #[cfg(feature = "backup")]
    if let Some(scheduler) = &_state.backup_scheduler {
        tracing::info!("正在停止备份调度器...");
        if let Err(e) = scheduler.stop().await {
            tracing::error!("停止备份调度器失败: {}", e);
        }
    }
}
