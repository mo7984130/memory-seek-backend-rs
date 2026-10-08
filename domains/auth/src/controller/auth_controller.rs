use crate::AuthState;
use crate::services as auth_service;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use common_core::error::{AppError, ContextualError};
use common_core::{Result, error::contextual::ext::OptionExt};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedJson,
};
use std::sync::Arc;
use types_identity::auth::user::UserId;
use types_identity::auth::{
    LoginRequest, LoginResponse, RefreshAccessTokenResponse, RegisterRequest, SendEmailCodeRequest,
};
use types_identity::user::UserInfo;

pub struct AuthController;

impl ControllerRouter for AuthController {
    type State = AuthState;

    /// 返回空的路由集合，本控制器不提供受保护路由
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }

    /// 返回认证相关的公开路由
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/login", post(Self::login))
            .route("/register", post(Self::register))
            .route("/token", post(Self::refresh_access_token))
            .route("/verification-codes", post(Self::send_email_code))
    }
}

impl AuthController {
    /// 用户登录
    async fn login(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(req): ValidatedJson<LoginRequest>,
    ) -> Result<R<LoginResponse>> {
        auth_service::login(&state, req).await.to_r_ok()
    }

    /// 用户注册
    async fn register(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(req): ValidatedJson<RegisterRequest>,
    ) -> Result<R<UserInfo>> {
        auth_service::register(&state, req).await.to_r_ok()
    }

    /// 发送邮箱验证码
    async fn send_email_code(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(req): ValidatedJson<SendEmailCodeRequest>,
    ) -> Result<R<()>> {
        auth_service::send_email_code(&state, req).await.to_r_ok()
    }

    /// 从请求头 `x-user-id` 与 `x-refresh-token` 提取凭据并签发新的 access_token
    async fn refresh_access_token(
        State(state): State<Arc<AuthState>>,
        headers: HeaderMap,
    ) -> Result<R<RefreshAccessTokenResponse>> {
        let user_id = headers
            .get("x-user-id")
            .ok_or_warn(
                "x-user-id_missing",
                "鉴权时, x-user-id 头缺失",
                AppError::bad_request("x-user-id 头缺失"),
            )?
            .to_str()
            .map_err(|error| {
                ContextualError::warn(
                    "x-user-id_illegal",
                    "鉴权时, x-user-id 格式非法",
                    error,
                    AppError::bad_request("x-user-id 格式非法"),
                )
                .emit()
            })?
            .parse::<UserId>()
            .map_err(|error| {
                ContextualError::warn(
                    "x-user-id_invalid",
                    "鉴权时, x-user-id 必须是数字",
                    error,
                    AppError::bad_request("x-user-id 必须是数字"),
                )
                .emit()
            })?;

        let refresh_token = headers
            .get("x-refresh-token")
            .ok_or_warn(
                "x-refresh-token_missing",
                "鉴权时, x-refresh-token 头缺失",
                AppError::bad_request("x-refresh-token 头缺失"),
            )?
            .to_str()
            .map_err(|error| {
                ContextualError::warn(
                    "x-refresh-token_illegal",
                    "鉴权时, x-refresh-token 格式非法",
                    error,
                    AppError::bad_request("x-refresh-token 格式非法"),
                )
                .emit()
            })?
            .to_string();
        auth_service::refresh_access_token(&state, user_id, refresh_token)
            .await
            .to_r_ok()
    }
}
