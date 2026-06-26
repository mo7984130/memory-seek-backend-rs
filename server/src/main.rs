use tokio::net::TcpListener;

mod config;
mod metrics;
mod middlewares;
mod setup;
mod state;

use config::AppConfig;
use setup::AppSetup;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _graud = crate::setup::bases::log::init();

    // 加载配置
    let cfg = AppConfig::load()?;

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
    let listener = TcpListener::bind(&cfg.server_addr()).await?;
    tracing::info!("Server listening on {}", cfg.server_addr());

    axum::serve(listener, app).await?;

    Ok(())
}
