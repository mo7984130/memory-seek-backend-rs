use clap::Parser;
use common::Result;
use common::time::Duration;
use common::{error::ContextualError, ext::IntoContextualExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

mod config;
mod metrics;
mod middlewares;
mod setup;
mod state;

use config::AppConfig;
use setup::AppSetup;

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Memory Seek 后端服务
#[derive(Parser)]
#[command(name = "memory-seek-server")]
struct Cli {
    /// 配置文件路径
    #[arg(short = 'c', long = "config")]
    config: Option<String>,
}

#[tokio::main]
/// 加载配置, 初始化应用并启动 HTTP 服务.
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 提前初始化日志系统，确保配置加载等早期阶段也能记录错误详情
    // 日志统一输出到 stdout/stderr，由 systemd journald 捕获管理
    setup::bases::log::init();

    // 加载配置
    let cfg = AppConfig::load(cli.config).map_err(|source| {
        ContextualError::error(
            "config_load_err",
            "加载配置失败",
            source,
            common::error::AppError::InternalServerError,
        )
    })?;

    // 初始化应用（内部会初始化日志、数据库、Redis、metrics 等）
    let app_setup = AppSetup::init(&cfg).await?;

    // 创建全局取消令牌，用于通知所有后台任务退出
    let cancel_token = CancellationToken::new();

    // 启动后台指标采集（传入 cancel token，采集任务会在收到取消信号后退出）
    #[cfg(feature = "metrics")]
    metrics::start_collector(
        app_setup.state.db.clone(),
        app_setup.state.redis.clone(),
        Duration::from_secs(cfg.metrics.interval_seconds),
        cancel_token.child_token(),
    );

    // 克隆 state 用于优雅关闭（router 会消费 app_setup.state）
    let graceful_state = app_setup.state.clone();

    // 合并路由并添加中间件
    let app = app_setup
        .public_router
        .route("/health", axum::routing::get(|| async { "ok" }));

    // Prometheus metrics 由主服务暴露，不再单独监听端口
    #[cfg(feature = "metrics")]
    let app = app.route(
        "/metrics",
        axum::routing::get(crate::metrics::render_metrics),
    );

    let app = app
        .merge(
            app_setup
                .protected_router
                .layer(axum::middleware::from_fn_with_state(
                    app_setup.state.clone(),
                    middlewares::auth::auth_middleware,
                )),
        )
        .layer(middlewares::cors::layer());

    #[cfg(feature = "metrics")]
    let app = app.layer(axum::middleware::from_fn(
        middlewares::metrics::metrics_middleware,
    ));

    let app = app
        .layer(axum::middleware::from_fn(
            middlewares::tracing_span::tracing_span,
        ))
        .layer(axum::middleware::from_fn(
            middlewares::trace_id::trace_id_middleware,
        ))
        .layer(axum::middleware::from_fn(
            middlewares::client_ip::client_ip_middleware,
        ))
        .with_state(app_setup.state);

    // 启动服务器
    tracing::info!("尝试监听{}端口", cfg.server.port);
    let listener = TcpListener::bind(&cfg.server_addr())
        .await
        .into_contextual()?;
    tracing::info!("Server listening on {}", cfg.server_addr());

    let shutdown_signal = shutdown_signal(graceful_state, cancel_token);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
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
            Ok(Err(e)) => common::caller_error!(error = %e, "停止备份调度器失败"),
            Err(_) => common::caller_error!("停止备份调度器超时"),
        }
    }

    // ---- 4. 关闭数据库连接池 ----
    tracing::info!("正在关闭数据库连接池...");
    // close() 消费 self，需要从 Arc 中 clone 出一份来关闭
    if let Err(e) = state.db.clone().close().await {
        common::caller_error!(error = %e, "关闭数据库连接池失败");
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
