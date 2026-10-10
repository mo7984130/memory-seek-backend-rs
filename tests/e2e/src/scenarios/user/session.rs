//! user 模块场景共享的会话预置: 受保护端点所需的登录凭证.

use std::sync::Arc;

use memory_seek_api::{Client as ApiClient, api};
use memseek_test::ctxlibs::http_client::HttpError;
use types_identity::auth::{LoginRequest, user::UserId};

use crate::context::Context;
use crate::error::api_to_http;

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

/// 已登录会话: 携带已注入登录态的 api-rs 客户端, 供受保护端点复用.
pub struct Session {
    pub user_id: UserId,
    pub access_token: String,
    pub refresh_token: String,
    client: Option<Arc<ApiClient>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            user_id: UserId(0),
            access_token: String::new(),
            refresh_token: String::new(),
            client: None,
        }
    }
}

impl Session {
    /// 认证中间件要求的头: `Authorization: Bearer {user_id} {access_token}`.
    ///
    /// 仅供仍走裸请求的场景(batch 等负向用例)构造请求使用;已迁移场景请用
    /// [`Session::client`]。
    pub fn auth_header(&self) -> String {
        format!("Bearer {} {}", self.user_id, self.access_token)
    }

    /// 已登录的 api-rs 客户端.
    pub fn client(&self) -> &ApiClient {
        self.client.as_deref().expect("session client 未初始化")
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
    let client = ctx.api_client();
    let request = LoginRequest {
        account: account.to_owned(),
        password: password.to_owned(),
    };
    let response = api::auth::login(&client, &request)
        .await
        .map_err(api_to_http)?;
    client.auth_storage().set_login_result(&response);
    Ok(Session {
        user_id: response.user.id,
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        client: Some(Arc::new(client)),
    })
}
