use crate::middlewares::ValidatedJson;
use crate::state::AppState;
use auth::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};
use auth::service as auth_service;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::Router;
use common::error::AppError;
use common::r::R;
use common::utils::ResultExt;
use entities::user::UserDTO;
use std::sync::Arc;
use crate::utils::ClientIp;

pub struct AuthController;

impl AuthController {
    pub fn routes() -> Router<Arc<AppState>> {
        Router::new()
            .route("/login", post(Self::login))
            .route("/register", post(Self::register))
            .route("/email-verify-code", get(Self::send_email_code))
            .route("/access-token", get(Self::refresh_access_token))
    }

    async fn login(
        State(state): State<Arc<AppState>>,
        ClientIp(_client_ip): ClientIp,
        ValidatedJson(req): ValidatedJson<LoginRequest>
    ) -> Result<R<UserDTO>, AppError> {
        auth_service::login(&state.db, &state.redis, req, &state.encryption_key).await.into_ok_res()
    }

    async fn register(
        State(state): State<Arc<AppState>>,
        ValidatedJson(payload): ValidatedJson<RegisterRequest>
    ) -> Result<R<UserDTO>, AppError> {
        auth_service::register(&state.db, &state.redis, payload).await.into_ok_res()
    }

    async fn send_email_code(
        State(state): State<Arc<AppState>>,
        ValidatedJson(payload): ValidatedJson<SendEmailCodeRequest>
    ) -> Result<R<()>, AppError> {
        auth_service::send_email_code(&state.redis, &state.email_client, payload).await.into_ok_res()
    }

    async fn refresh_access_token(
        State(state): State<Arc<AppState>>,
        headers: HeaderMap
    ) -> Result<R<AccessTokenResponse>, AppError> {
        let user_id = headers.get("x-user-id")
            .ok_or_else(|| AppError::bad_request("X-User_Id 头缺失"))?
            .to_str()
            .map_err(|_| AppError::bad_request("X-User-Id 格式非法"))?
            .parse::<i64>()
            .map_err(|_| AppError::bad_request("X-User-Id 必须是数字"))?;

        tracing::Span::current().record("user_id", &user_id.to_string().as_str());

        let refresh_token_str = headers.get("X-Refresh-Token")
            .ok_or_else(|| AppError::bad_request("X-Refresh-Token 头缺失"))?
            .to_str()
            .map_err(|_| AppError::bad_request("X-Refresh-Token 格式非法"))?
            .to_string();
        auth_service::refresh_access_token(
            &state.db, &state.redis, user_id, refresh_token_str
        ).await.into_ok_res()
    }
}
