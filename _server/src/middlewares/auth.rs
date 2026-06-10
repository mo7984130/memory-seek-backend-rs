use crate::state::AppState;
use axum::extract::Request;
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;
use common::constants::RedisKeys;
use common::error::AppError;
use common::ext::OptionExt;
use common::ext::RedisExt;
use common::ext::ResultErrExt;
use entities::auth::user::UserId;
use std::sync::Arc;

/// 认证中间件
///
/// 从请求头 `Authorization` 中提取用户 ID 和 access token，
/// 与 Redis 中存储的 token 进行比对验证。验证通过后将 `UserId` 注入请求扩展。
///
/// # 参数
/// - `state`: 应用状态，包含 Redis 连接池
/// - `request`: HTTP 请求
/// - `next`: 下一个中间件/处理器
///
/// # 返回
/// 返回下游处理器的响应
///
/// # 错误
/// - `AppError::BadRequest`: 认证头缺失、格式错误或用户 ID 解析失败
/// - `AppError::Unauthorized`: Redis 中无对应 token 或 token 不匹配
/// - `AppError`: Redis 操作失败
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .ok_or_warn(
            "authorization_header_miss",
            "认证中间件, 认证时, 认证头缺失",
            AppError::bad_request("认证头缺失"),
        )?;

    let (id_str, access_token) = auth_header.split_once(' ').ok_or_warn(
        "authorization_header_format_error",
        "认证中间件, 认证时, 认证头格式错误",
        AppError::bad_request("认证头格式错误"),
    )?;
    let id = id_str.parse::<i64>().trace_warn_bad_request(
        "user_id_format_error",
        "认证中间件, 认证时, 用户ID格式错误",
        "用户ID格式错误",
    )?;

    let stored_access_token: String = state
        .redis
        .get_as(&RedisKeys::auth::user_access_token(id))
        .await?
        .ok_or_warn(
            "user_access_token_not_found",
            "认证中间件, 认证时, 用户access_token未找到",
            AppError::Unauthorized,
        )?;

    if stored_access_token != access_token {
        return Err(AppError::Unauthorized);
    }

    request.extensions_mut().insert(UserId(id));

    Ok(next.run(request).await)
}
