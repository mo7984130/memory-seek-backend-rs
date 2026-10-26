//! 认证相关 API。
//!
//! 路由前缀:`/auth`(公开,无需鉴权)。

use crate::client::{Client, Request};
use crate::error::Result;
use types_core::UserId;
use types_identity::auth::{
    LoginRequest, LoginResponse, RefreshAccessTokenResponse, RegisterRequest, SendEmailCodeRequest,
};
use types_identity::user::UserInfo;

/// 登录。`POST /auth/login`
pub async fn login(client: &Client, request: &LoginRequest) -> Result<LoginResponse> {
    client
        .send_json(Request::post("/auth/login").no_auth().json(request))
        .await
}

/// 注册。`POST /auth/register`
pub async fn register(client: &Client, request: &RegisterRequest) -> Result<UserInfo> {
    client
        .send_json(Request::post("/auth/register").no_auth().json(request))
        .await
}

/// 发送邮箱验证码。`POST /auth/verification-codes`
pub async fn send_email_code(client: &Client, request: &SendEmailCodeRequest) -> Result<()> {
    client
        .send_unit(
            Request::post("/auth/verification-codes")
                .no_auth()
                .json(request),
        )
        .await
}

/// 刷新访问令牌。`POST /auth/token`
///
/// 通常由客户端在 401 时自动调用;此函数供需要手动刷新时使用(不会写回存储)。
pub async fn refresh_token(
    client: &Client,
    user_id: UserId,
    refresh_token: &str,
) -> Result<RefreshAccessTokenResponse> {
    client.fetch_refresh(user_id, refresh_token).await
}
