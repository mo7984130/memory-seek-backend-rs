//! user 模块场景共享的会话预置: 受保护端点所需的登录凭证.

use common::axum::SucR;
use memseek_test::ctxlibs::http_client::{HttpError, reqwest};
use serde_json::json;
use types::auth::{LoginResponse, user::UserId};

use crate::context::Context;

/// 测试用户池统一初始密码(与 `preprea.rs` 的 `PASS_HASH` 对应).
pub const PASSWORD: &str = "Test123456";
/// 改密正例使用的新密码.
pub const NEW_PASSWORD: &str = "NewPass123";

/// 通用池账号(nickname / avatar / logout 及改密负例).
pub fn user_account(index: usize) -> String {
    format!("uit_user_{}", index + 1)
}

/// me 专用池账号(仅 MeScenario 使用, 与写场景隔离避免并发竞态).
pub fn me_account(index: usize) -> String {
    format!("uit_me_{}", index + 1)
}

/// logout 专用池账号(仅 LogoutScenario 使用, 登出会清除会话, 隔离避免踢掉共享池 token).
pub fn logout_account(index: usize) -> String {
    format!("uit_logout_{}", index + 1)
}

/// 改密正例专用池账号.
pub fn pwd_account(index: usize) -> String {
    format!("uit_pwd_{}", index + 1)
}

/// 已登录会话, 供受保护端点的 run/validate 复用.
pub struct Session {
    pub user_id: UserId,
    pub access_token: String,
    pub refresh_token: String,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            user_id: UserId(0),
            access_token: String::new(),
            refresh_token: String::new(),
        }
    }
}

impl Session {
    /// 认证中间件要求的头: `Authorization: Bearer {user_id} {access_token}`.
    pub fn auth_header(&self) -> String {
        format!("Bearer {} {}", self.user_id, self.access_token)
    }
}

/// 用固定密码登录指定账号.
pub async fn login(ctx: &Context, account: &str) -> Result<Session, HttpError> {
    login_with(ctx, account, PASSWORD).await
}

/// 用指定密码登录(改密正例回登时使用新密码).
pub async fn login_with(
    ctx: &Context,
    account: &str,
    password: &str,
) -> Result<Session, HttpError> {
    let resp: SucR<LoginResponse> = ctx
        .client
        .request(reqwest::Method::POST, "/auth/login")
        .json_unwrap(&json!({ "account": account, "password": password }))
        .send_checked()
        .await?
        .json()
        .await?;
    Ok(Session {
        user_id: resp.data.user.id,
        access_token: resp.data.access_token,
        refresh_token: resp.data.refresh_token,
    })
}
