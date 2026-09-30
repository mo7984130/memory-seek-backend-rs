use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::{from_fn, from_fn_with_state};

use clap::Parser;
use common_core::Result;
use common_core::error::contextual::ext::IntoContextualExt;
use common_core::time::Duration;
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

    // 初始化应用(临时文件目录在 AppSetup 的 base 阶段创建)
    let mut app_setup = AppSetup::init(&cfg).await?;

    let state = AppState::from_setup(&app_setup, cfg.server.max_upload_bytes)?;
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
            // 上传请求体上限: 由配置 server.max_upload_bytes 控制(默认 600MB),
            // 须大于业务校验上限(图片 20MB / 视频 512MB); axum 默认 2MB,
            // 超过会在请求体流式读取时报 failed to read stream
            .layer(DefaultBodyLimit::max(cfg.server.max_upload_bytes as usize))
            // 已知长度请求预检: 超限直接 413, 避免大文件传完才被拒
            .layer(from_fn_with_state(
                Arc::clone(&state),
                middlewares::upload_size_limit::upload_size_limit,
            ))
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
    tracing::info!("监听 {}", cfg.server_addr());

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_trigger())
    .await
    .into_contextual()?;

    // 走到这里说明在途连接已排空, 再统一收尾:
    // 停后台任务 -> 清理临时目录 -> 关闭连接池, 避免误伤在途请求。
    state.task_manager.shutdown(Duration::from_secs(0)).await;

    // 删除统一临时文件目录(优雅关闭时)
    common_core::remove_dir_all(&state.tmp_path);
    info!(path = %state.tmp_path.display(), "临时文件目录已删除");

    if let Err(e) = state.db.clone().close().await {
        error!(error = %e, "关闭数据库连接池失败");
    } else {
        info!("数据库连接池已关闭");
    }
    state.redis.close();
    info!("Redis 连接池已关闭");

    tracing::info!("服务已完全关闭");
    Ok(())
}

/// 优雅关闭信号触发器。
///
/// 仅负责等待 SIGINT / SIGTERM。真正的收尾(停止后台任务、清理临时目录、
/// 关闭数据库与 Redis)统一放在 [`axum::serve`] 返回之后执行, 确保在途请求
/// 已全部排空后再释放其依赖的资源。
async fn shutdown_trigger() {
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

    tracing::info!("收到关闭信号，开始排空在途请求");
}
