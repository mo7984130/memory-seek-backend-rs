mod config;
mod middlewares;
mod state;

mod setup;

use axum::Router;
use axum::http::Method;
use log::info;
use std::cmp::max;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "metrics")]
mod metrics;
#[cfg(feature = "metrics")]
use crate::metrics::{init_metrics_system, spawn_monitoring_tasks};

#[cfg(any(feature = "photo", feature = "user"))]
use oss::S3Client;

#[cfg(any(feature = "user", feature = "photo"))]
use crate::middlewares::auth::auth_middleware;

use crate::setup::{database::init_db, log::init_log, redis::init_redis};

const VERSION: &str = "0.0.1";

/// 应用程序入口函数
///
/// 初始化日志系统，计算最优 worker 线程数（可用并行度的一半，最少 1 个），
/// 构建 tokio 多线程运行时并启动异步主逻辑。
fn main() {
    init_log();

    info!("MEMORY SEEK BACKEND RS VERSION: {}", VERSION);

    let workers = std::thread::available_parallelism()
        .map(|n| max(n.get() / 2, 1))
        .unwrap_or(1);

    info!("axum worker 数: {}", workers);

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .unwrap()
        .block_on(logic_main())
        .unwrap();
}

/// 异步主逻辑入口
///
/// 按顺序初始化各组件：metrics（可选）、配置、数据库、Redis、TokenCipher、
/// OSS 客户端（可选），然后按 feature flag 初始化各业务模块（auth、user、photo），
/// 组装路由和中间件，最后在 `0.0.0.0:3000` 启动 HTTP 服务。
///
/// # 返回
/// 返回 `Ok(())` 表示服务正常退出
///
/// # 错误
/// - `Box<dyn std::error::Error>`: 数据库连接、Redis 连接或 TCP 监听失败时返回
async fn logic_main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "metrics")]
    init_metrics_system();

    info!("初始化config");
    let config_path =
        std::env::var("MEMORY_SEEK_CONFIG_PATH").unwrap_or_else(|_| "./config.json".to_string());
    info!("config文件路径: {}", config_path);
    let cfg = config::AppConfig::from_json(&config_path);

    info!("初始化数据库");
    let db = init_db(&cfg).await?;

    info!("初始化redis");
    let redis = init_redis(&cfg)?;

    info!("初始化TokenCipher");
    let token_cipher = {
        use common::utils::TokenCipher;
        Arc::new(TokenCipher::from_config(&cfg.token_cipher_config))
    };

    #[cfg(any(feature = "user", feature = "photo"))]
    let s3_client = Arc::new(S3Client::new(&cfg.oss_config));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any);

    #[cfg(any(feature = "auth", feature = "photo"))]
    let mut public_routers = Router::new();
    #[cfg(not(any(feature = "auth", feature = "photo")))]
    let public_routers = Router::new();

    #[cfg(any(feature = "user", feature = "photo"))]
    let mut protected_routes = Router::new();
    #[cfg(not(any(feature = "user", feature = "photo")))]
    let protected_routes = Router::new();

    #[cfg(feature = "auth")]
    {
        use crate::setup::auth::{init_auth, mount_public};

        info!("初始化auth模块");
        let auth_state = init_auth(&cfg, db.clone(), redis.clone(), token_cipher.clone());
        public_routers = mount_public(public_routers, auth_state);
    }

    #[cfg(feature = "user")]
    {
        use crate::setup::user::{init_user, mount_protected};

        info!("初始化user模块");
        let user_state = init_user(
            &cfg,
            db.clone(),
            redis.clone(),
            s3_client.clone(),
            token_cipher.clone(),
        );
        protected_routes = mount_protected(protected_routes, user_state);
    }

    #[cfg(feature = "photo")]
    {
        use crate::setup::photo::{init_photo, mount_protected, mount_public};

        info!("初始化photo模块");
        let photo_state = init_photo(
            &cfg,
            db.clone(),
            redis.clone(),
            s3_client.clone(),
            token_cipher.clone(),
        )
        .await;
        protected_routes = mount_protected(protected_routes, photo_state.clone());
        public_routers = mount_public(public_routers, photo_state.clone());
    }

    #[cfg(any(feature = "user", feature = "photo"))]
    {
        use crate::state::AppState;
        let state = Arc::new(AppState {
            redis: redis.clone(),
        });
        protected_routes = protected_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));
    }

    let app = Router::new()
        .merge(public_routers)
        .merge(protected_routes)
        .layer(cors)
        .layer(axum::middleware::from_fn(middlewares::trace_id_middleware));

    #[cfg(feature = "metrics")]
    spawn_monitoring_tasks(db.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("服务启动于 http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
