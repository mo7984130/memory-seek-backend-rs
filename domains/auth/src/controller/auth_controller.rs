use crate::AuthState;
use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};
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
    pub fn routes() -> Router<Arc<AuthState>> {
        Router::new()
            .route("/login", post(Self::login))
            .route("/register", post(Self::register))
            .route("/email-verify-code", post(Self::send_email_code))
            .route("/access-token", get(Self::refresh_access_token))
    }

    async fn login(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(req): ValidatedJson<LoginRequest>
    ) -> Result<R<UserDTO>, AppError> {
        auth_service::login(&state.db, &state.redis, &state.hasher, req, &state.encryption_key, &state.password_verify_semaphore).await.into_ok_res()
    }

    async fn register(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(payload): ValidatedJson<RegisterRequest>
    ) -> Result<R<UserDTO>, AppError> {
        auth_service::register(&state.db, &state.redis, &state.hasher, payload).await.into_ok_res()
    }

    async fn send_email_code(
        State(state): State<Arc<AuthState>>,
        ValidatedJson(payload): ValidatedJson<SendEmailCodeRequest>
    ) -> Result<R<()>, AppError> {
        auth_service::send_email_code(&state.redis, &state.email_client, payload).await.into_ok_res()
    }

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
        auth_service::refresh_access_token(
            &state.db, &state.redis, user_id, refresh_token_str
        ).await.into_ok_res()
    }
}
