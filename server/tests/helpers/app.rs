use axum::Router;
use server::middlewares;
use server::setup::AppSetup;

use super::test_config;

/// 构建测试用 Router
///
/// 使用 tests/test.config.json 配置，初始化完整的应用路由。
/// 包含真实的 auth_middleware，测试需通过登录获取 token 来访问受保护路由。
///
/// 返回 `Router<()>`（state 已通过 `with_state()` 消费）。
pub async fn build_test_router() -> Router {
    // 加载配置并初始化应用
    let cfg = test_config();
    let app_setup = AppSetup::init(&cfg).await.expect("初始化测试应用失败");

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
