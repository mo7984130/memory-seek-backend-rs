use axum::Router;
use axum::middleware::{from_fn, from_fn_with_state};

use clap::Parser;
use common::Result;
use common::error::contextual::ext::IntoContextualExt;
use common::time::Duration;
use tracing::{error, info};

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

mod config;
mod middlewares;
mod setup;
mod state;
mod util;

use config::AppConfig;
use setup::AppSetup;

use crate::setup::AppRouter;
use crate::state::AppState;

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
#[command(name = "memory-seek-server")]
struct Cli {
    /// 配置文件路径
    #[arg(short = 'c', long = "config")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 初始化日志
    setup::bases::log::init();

    // 加载配置
    let cfg = AppConfig::load(cli.config);

    // 初始化应用
    let mut app_setup = AppSetup::init(&cfg).await?;

    let state = AppState::from_setup(&app_setup)?;
    let state = Arc::new(state);

    // 合并路由并添加中间件
    let router = {
        let router = Router::new()
            .route("/health", axum::routing::get(|| async { "ok" }))
            .route("/hello", axum::routing::get(|| async { "hello" }));
        app_setup.router.add_public(router);

        let AppRouter { protected, public } = app_setup.router;
        let router = Router::new();
        let protected = protected.layer(from_fn_with_state(
            Arc::clone(&state),
            middlewares::auth::auth_middleware,
        ));

        router
            .merge(public)
            .merge(protected)
            .layer(from_fn(middlewares::tracing_span::tracing_span))
            .layer(from_fn(middlewares::trace_id::trace_id_middleware))
            .layer(from_fn(middlewares::client_ip::client_ip_middleware))
            .layer(middlewares::cors::layer())
    };

    // 启动服务器
    tracing::info!("尝试监听{}端口", cfg.server.port);
    let listener = TcpListener::bind(&cfg.server_addr())
        .await
        .into_contextual()?;
    tracing::info!("Server listening on {}", cfg.server_addr());

    let shutdown_signal = shutdown_signal(Arc::clone(&state));

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal)
    .await
    .into_contextual()?;

    tracing::info!("服务已完全关闭");
    Ok(())
}

/// 优雅关闭信号处理
///
/// 流程：
/// 1. 等待 SIGINT 或 SIGTERM
/// 2. 通过 TaskManager 取消所有后台任务并等待（带超时）。
///    定时备份、指标采集等任务共享根取消令牌，一次取消全部生效。
/// 3. 关闭数据库连接池
/// 4. 关闭 Redis 连接池
async fn shutdown_signal(state: Arc<crate::state::AppState>) {
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

    tracing::info!("收到关闭信号，开始关闭");

    // 关闭后台任务
    state.task_manager.shutdown(Duration::from_secs(0)).await;

    if let Err(e) = state.db.clone().close().await {
        error!(error = %e, "关闭数据库连接池失败");
    } else {
        info!("数据库连接池已关闭");
    }
    state.redis.close();
    info!("Redis 连接池已关闭");

    info!("关闭完成，服务退出");
}
