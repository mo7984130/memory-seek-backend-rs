use axum::Router;
use axum::middleware::{from_fn, from_fn_with_state};

use clap::Parser;
use common::Result;
use common::error::contextual::ext::IntoContextualExt;
use common::time::Duration;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

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

        let router = router.merge(public).merge(protected);

        let router = router
            .layer(from_fn(middlewares::tracing_span::tracing_span))
            .layer(from_fn(middlewares::trace_id::trace_id_middleware))
            .layer(from_fn(middlewares::client_ip::client_ip_middleware))
            .layer(middlewares::cors::layer());
        router
    };

    // 启动服务器
    tracing::info!("尝试监听{}端口", cfg.server.port);
    let listener = TcpListener::bind(&cfg.server_addr())
        .await
        .into_contextual()?;
    tracing::info!("Server listening on {}", cfg.server_addr());

    let shutdown_signal = shutdown_signal(Arc::clone(&state), app_setup.cancel_token);

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
/// 2. 触发 CancellationToken 通知后台任务退出
/// 3. 停止备份调度器（带超时）
/// 4. 关闭数据库连接池
/// 5. 关闭 Redis 连接池
async fn shutdown_signal(state: Arc<crate::state::AppState>, cancel_token: CancellationToken) {
    // ---- 1. 等待 OS 信号 ----
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

    tracing::info!("收到关闭信号，开始优雅关闭...");

    // ---- 2. 通知所有后台任务退出 ----
    cancel_token.cancel();
    tracing::info!("已通知后台任务退出");

    // 给后台任务一点时间响应取消信号
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ---- 3. 停止备份调度器（带超时兜底） ----
    #[cfg(feature = "backup")]
    {
        tracing::info!("正在停止备份调度器...");
        let stop_result =
            tokio::time::timeout(Duration::from_secs(10), state.backup_scheduler.stop()).await;

        match stop_result {
            Ok(Ok(())) => tracing::info!("备份调度器已停止"),
            Ok(Err(e)) => tracing::error!(error = %e, "停止备份调度器失败"),
            Err(_) => tracing::error!("停止备份调度器超时"),
        }
    }

    // ---- 4. 关闭数据库连接池 ----
    tracing::info!("正在关闭数据库连接池...");
    // close() 消费 self，需要从 Arc 中 clone 出一份来关闭
    if let Err(e) = state.db.clone().close().await {
        tracing::error!(error = %e, "关闭数据库连接池失败");
    } else {
        tracing::info!("数据库连接池已关闭");
    }

    // ---- 5. 关闭 Redis 连接池 ----
    tracing::info!("正在关闭 Redis 连接池...");
    state.redis.close();
    tracing::info!("Redis 连接池已关闭");

    // ---- 6. 完成 ----
    tracing::info!("优雅关闭完成，服务即将退出");
}
