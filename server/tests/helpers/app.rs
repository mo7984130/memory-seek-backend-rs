use axum::Router;
use server::config::AppConfig;
use server::setup::AppSetup;
use server::middlewares;

/// 构建测试用 Router
///
/// 使用 tests/integration/config.json 配置，初始化完整的应用路由。
/// 包含真实的 auth_middleware，测试需通过登录获取 token 来访问受保护路由。
///
/// 返回 `Router<()>`（state 已通过 `with_state()` 消费）。
pub async fn build_test_router() -> Router {
    // 设置配置文件路径（使用 CARGO_MANIFEST_DIR 定位）
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let config_path = format!("{}/tests/config.json", manifest_dir);
    std::env::set_var("MEMORY_SEEK_CONFIG_PATH", &config_path);

    // 加载配置并初始化应用
    let cfg = AppConfig::load().expect("加载测试配置失败");
    let app_setup = AppSetup::init(&cfg)
        .await
        .expect("初始化测试应用失败");

    // 合并路由并添加中间件（与 main.rs 一致）
    app_setup
        .public_router
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
        .with_state(app_setup.state)
}
