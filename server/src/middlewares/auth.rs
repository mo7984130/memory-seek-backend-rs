use std::sync::Arc;
use axum::{extract::Request, middleware::Next, response::Response};
use common::{error::AppError, ext::OptionExt};
use crate::state::AppState;

/// 认证中间件
///
/// 从 Authorization header 中提取 `Bearer user_id access_token` 格式，
/// 验证 token 是否是该 user_id 的有效 token，然后将 user_id 注入到请求扩展中
pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 从 Authorization header 中提取 Bearer user_id access_token
    let (user_id, token) = extract_bearer(&request)?;

    // 验证 token 是否是该 user_id 的有效 token
    verify_token(&state, user_id, token).await?;

    // 将 user_id 注入到请求扩展中
    let mut request = request;
    request.extensions_mut().insert(user_id);

    Ok(next.run(request).await)
}

/// 从 Authorization header 中解析 user_id 和 access_token
///
/// 格式: `Bearer user_id access_token`
fn extract_bearer(request: &Request) -> Result<(i64, &str), AppError> {
    let header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_warn("auth_err", "请求缺少 Authorization header", AppError::Unauthorized)?;

    let content = header
        .strip_prefix("Bearer ")
        .ok_or_warn("auth_err", "Authorization header 格式错误，缺少 Bearer 前缀", AppError::Unauthorized)?;

    let (user_id_str, token) = content
        .split_once(' ')
        .ok_or_warn("auth_err", "Authorization header 格式错误，应为: Bearer user_id access_token", AppError::Unauthorized)?;

    let user_id: i64 = user_id_str.parse().map_err(|_| {
        tracing::warn!(user_id_str, "user_id 不是有效的数字");
        AppError::Unauthorized
    })?;

    Ok((user_id, token))
}

/// 验证 token 是否是该 user_id 的有效 token
///
/// 从 Redis 获取 `a:u:at:{user_id}` 对应的 token，与请求中的 token 比对
async fn verify_token(state: &AppState, user_id: i64, token: &str) -> Result<(), AppError> {
    use deadpool_redis::redis::AsyncCommands;
    use constants::RedisKeys;

    let mut conn = state.redis.get().await.map_err(|e| {
        tracing::error!("获取 Redis 连接失败: {}", e);
        AppError::InternalServerError
    })?;

    let key = RedisKeys::auth::user_access_token(user_id);
    let stored_token: Option<String> = conn.get(&key).await.map_err(|e| {
        tracing::error!("查询 Redis token 失败: {}", e);
        AppError::InternalServerError
    })?;

    match stored_token {
        Some(stored) if stored == token => Ok(()),
        _ => {
            tracing::warn!(user_id, "access_token 无效或已过期");
            Err(AppError::Unauthorized)
        }
    }
}
