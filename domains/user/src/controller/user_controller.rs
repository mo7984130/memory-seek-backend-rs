use axum::extract::State;
use axum::routing::{get, post};
use axum::{Extension, Router};
use common::error::AppError;
use common::extractors::ValidatedJson;
use common::models::UserId;
use common::r::R;
use common::utils::ResultExt;
use entities::user::UserDTO;
use std::sync::Arc;

use crate::controller::UserState;
use crate::models::{ChangeNicknameRequest, ChangePasswordRequest, GetUserInfoBatchRequest, InviterCodeDTO, UserInfoVO};
use crate::services as user_service;

pub struct UserController;

impl UserController {
    pub fn routes() -> Router<Arc<UserState>> {
        Router::new()
            .route("/info", get(Self::get_user_info))
            .route("/inviter-code", get(Self::generate_inviter_code))
            .route("/nickname", post(Self::change_nickname))
            .route("/password", post(Self::change_password))
            .route("/logout", post(Self::logout))
            .route("/info/batch", post(Self::get_user_info_batch))
    }

    async fn get_user_info(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>
    ) -> Result<R<UserDTO>, AppError> {
        user_service::get_user_info(&state.db, user_id.0, &state.encryption_key).await.into_ok_res()
    }

    async fn generate_inviter_code(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>
    ) -> Result<R<InviterCodeDTO>, AppError> {
        user_service::generate_inviter_code(&state.redis, user_id.0).await.into_ok_res()
    }

    async fn change_nickname(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangeNicknameRequest>
    ) -> Result<R<String>, AppError> {
        user_service::change_nickname(&state.db, &state.redis, user_id.0, req.new_nickname).await.into_ok_res()
    }

    async fn change_password(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangePasswordRequest>
    ) -> Result<R<()>, AppError> {
        user_service::change_password(&state.db, &state.redis, user_id.0, req).await.into_ok_res()
    }

    async fn logout(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>
    ) -> Result<R<()>, AppError> {
        user_service::logout(&state.db, &state.redis, user_id.0).await.into_ok_res()
    }

    async fn get_user_info_batch(
        State(state): State<Arc<UserState>>,
        ValidatedJson(req): ValidatedJson<GetUserInfoBatchRequest>,
    ) -> Result<R<Vec<Option<UserInfoVO>>>, AppError> {
        let user_ids = req.user_ids.into_iter()
            .map(|id| id.parse::<i64>())
            .collect::<Result<Vec<i64>, _>>()
            .map_bad_request_err("id格式错误")?;

        user_service::get_user_info_batch(&state.db, &state.redis, user_ids, &state.encryption_key).await.into_ok_res()
    }
}
