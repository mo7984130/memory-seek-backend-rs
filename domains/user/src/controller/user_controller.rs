use crate::middlewares::auth::UserId;
use crate::middlewares::ValidatedJson;
use crate::state::AppState;
#[cfg(feature = "photo")]
use axum::extract::Multipart;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use common::error::AppError;
use common::r::R;
use common::utils::ResultExt;
use entities::user::UserDTO;
use std::sync::Arc;
use user::models::{ChangeNicknameRequest, ChangePasswordRequest, GetUserInfoBatchRequest, InviterCodeDTO, UserInfoVO};
use user::services as user_service;

pub struct UserController;

impl UserController {

    pub fn routes() -> Router<Arc<AppState>> {
        let router = Router::new()
            .route("/info", get(Self::get_user_info))
            .route("/inviter-code", get(Self::generate_inviter_code))
            .route("/nickname", post(Self::change_nickname))
            .route("/password", post(Self::change_password))
            .route("/logout", get(Self::logout))
            .route("/info/batch", post(Self::get_user_info_batch));
        
        #[cfg(feature = "photo")]
        let router = router.route("/avatar", post(Self::upload_avatar));
        
        router
    }

    async fn get_user_info(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>
    ) -> Result<R<UserDTO>, AppError> {
        user_service::get_user_info(&state.db, user_id.0, &state.encryption_key).await.into_ok_res()
    }

    async fn generate_inviter_code(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>
    ) -> Result<R<InviterCodeDTO>, AppError> {
        user_service::generate_inviter_code(&state.redis, user_id.0).await.into_ok_res()
    }

    async fn change_nickname(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangeNicknameRequest>
    ) -> Result<R<String>, AppError> {
        user_service::change_nickname(&state.db, &state.redis, user_id.0, req.new_nickname).await.into_ok_res()
    }

    #[cfg(feature = "photo")]
    async fn upload_avatar(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<String>, AppError> {
        let field = multipart
            .next_field()
            .await
            .map_bad_request_err("无效的表单数据")?
            .ok_or_else(|| AppError::bad_request("未找到上传文件"))?;

        let file_name = field.file_name().unwrap_or("avatar.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpg").to_string();

        let file_data = field.bytes().await
            .map_internal_err("读取文件流失败")?
            .to_vec();

        user_service::update_avatar(
            &state.db,
            &state.redis,
            &state.s3_client,
            user_id.0,
            file_name,
            file_data,
            content_type,
            &state.encryption_key,
        ).await.into_ok_res()
    }

    async fn change_password(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangePasswordRequest>
    ) -> Result<R<()>, AppError> {
        user_service::change_password(&state.db, &state.redis, user_id.0, req).await.into_ok_res()
    }

    async fn logout(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>
    ) -> Result<R<()>, AppError> {
        user_service::logout(&state.db, &state.redis, user_id.0).await.into_ok_res()
    }

    async fn get_user_info_batch(
        State(state): State<Arc<AppState>>,
        Json(req): Json<GetUserInfoBatchRequest>,
    ) -> Result<R<Vec<Option<UserInfoVO>>>, AppError> {
        let user_ids = req.user_ids.into_iter()
            .map(|id| id.parse::<i64>())
            .collect::<Result<Vec<i64>, _>>()
            .map_bad_request_err("id格式错误")?;

        user_service::get_user_info_batch(&state.db, &state.redis, user_ids, &state.encryption_key).await.into_ok_res()
    }
}
