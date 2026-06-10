use deadpool_redis::Pool;

/// 全局应用共享状态
///
/// 用于需要跨中间件共享的资源，目前主要用于认证中间件访问 Redis。
#[allow(dead_code)]
pub struct AppState {
    pub redis: Pool,
}
