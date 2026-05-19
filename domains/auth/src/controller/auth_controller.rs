use crate::AuthState;
use crate::models::SendEmailCodeRequest;
use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest};
use crate::services as auth_service;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::Router;
use common::error::AppError;
use common::extractors::{ValidatedJson};
use common::r::R;
use common::utils::ResultExt;
use entities::user::UserDTO;
use std::sync::Arc;

pub struct AuthController;

impl AuthController {
    /// 构建认证模块的路由表
    ///
    /// # 返回
    /// 返回包含所有认证相关端点的 `Router`，包括登录、注册、刷新 token 和发送邮箱验证码
    pub fn routes() -> Router<Arc<AuthState>> {
        Router::new()
            .route("/login", post(Self::login))
            .route("/register", post(Self::register))
            .route("/access-token", get(Self::refresh_access_token))
            .route("/email-verify-code", post(Self::send_email_code))
    }

    /// 用户登录
    ///
    /// # 参数
    /// - `state`: 认证服务共享状态
    /// - `req`: 登录请求，包含账号和密码
    ///
    /// # 返回
    /// 返回登录成功的用户信息（含 access_token 和 refresh_token）
    ///
    /// # 错误
    /// - `AppError::bad_request`: 账号不存在或密码错误
    /// - `AppError::InternalServerError`: 数据库或 Redis 操作失败
    async fn login(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(req): ValidatedJson<LoginRequest>
    ) -> Result<R<UserDTO>, AppError> {
        auth_service::login(&state, req).await.into_ok_res()
    }

    /// 用户注册
    ///
    /// # 参数
    /// - `state`: 认证服务共享状态
    /// - `payload`: 注册请求，包含用户名、邮箱、密码、昵称、邀请码和邮箱验证码
    ///
    /// # 返回
    /// 返回注册成功的用户信息（不含 token，需单独登录获取）
    ///
    /// # 错误
    /// - `AppError::bad_request`: 邮箱验证码错误、邀请码无效、用户名或邮箱已存在
    /// - `AppError::InternalServerError`: 数据库操作失败
    async fn register(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(payload): ValidatedJson<RegisterRequest>
    ) -> Result<R<UserDTO>, AppError> {
        auth_service::register(&state, payload).await.into_ok_res()
    }

    /// 发送邮箱验证码
    ///
    /// # 参数
    /// - `state`: 认证服务共享状态
    /// - `payload`: 包含目标邮箱地址的请求
    ///
    /// # 返回
    /// 返回空成功响应
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: Redis 操作或邮件发送失败
    async fn send_email_code(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(payload): ValidatedJson<SendEmailCodeRequest>
    ) -> Result<R<()>, AppError> {
        auth_service::send_email_code(&state, payload).await.into_ok_res()
    }

    /// 刷新 access_token
    ///
    /// 从请求头 `x-user-id` 和 `x-refresh-token` 中提取凭据，验证后签发新的 access_token。
    ///
    /// # 参数
    /// - `state`: 认证服务共享状态
    /// - `headers`: HTTP 请求头，需包含 `x-user-id` 和 `x-refresh-token`
    ///
    /// # 返回
    /// 返回新的 access_token 及其过期时间
    ///
    /// # 错误
    /// - `AppError::bad_request`: 请求头缺失或格式非法
    /// - `AppError::Unauthorized`: refresh_token 不存在、不匹配或已过期
    async fn refresh_access_token(
        State(state): State<Arc<AuthState>>,
        headers: HeaderMap
    ) -> Result<R<AccessTokenResponse>, AppError> {
        let user_id = headers.get("x-user-id")
            .ok_or_else(|| AppError::bad_request("x-user-id 头缺失"))?
            .to_str()
            .map_err(|_| AppError::bad_request("x-user-id 格式非法"))?
            .parse::<i64>()
            .map_err(|_| AppError::bad_request("x-user-id 必须是数字"))?;

        tracing::Span::current().record("user_id", user_id);

        let refresh_token_str = headers.get("x-refresh-token")
            .ok_or_else(|| AppError::bad_request("x-refresh-token 头缺失"))?
            .to_str()
            .map_err(|_| AppError::bad_request("x-refresh-token 格式非法"))?
            .to_string();
        auth_service::refresh_access_token(&state, user_id, refresh_token_str).await.into_ok_res()
    }
}
