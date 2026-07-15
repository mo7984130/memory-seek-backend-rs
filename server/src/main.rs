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

#[tokio::main]
async fn main() -> Result<(), common::error::AppError> {
    let _graud = crate::setup::bases::log::init();

    // 加载配置
    let cfg = AppConfig::load().trace_internal_err("config_load_err", "加载配置失败")?;

    // 初始化应用
    let app_setup = AppSetup::init(&cfg).await?;

    // 初始化 Prometheus metrics exporter
    #[cfg(feature = "metrics")]
    {
        let metrics_cfg = cfg
            .metrics
            .as_ref()
            .expect("metrics config is required when metrics feature is enabled");
        setup::bases::metrics::init(&metrics_cfg.host, metrics_cfg.port);
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
    let listener = TcpListener::bind(&cfg.server_addr()).await.trace_internal_err("tcp_bind_err", "端口绑定失败")?;
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
