/// HTTP 中间件模块
///
/// 包含请求处理管线中的中间件：
/// - `trace_id`: 为每个请求生成唯一追踪 ID 并注入 tracing span
/// - `auth`: 验证用户身份的认证中间件（需要 `user` 或 `photo` feature）
pub(crate) mod trace_id;
pub use trace_id::trace_id_middleware;

pub(crate) mod auth;
pub use auth::auth_middleware;
