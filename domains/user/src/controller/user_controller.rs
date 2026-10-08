use axum::body::Body;
use axum::extract::State;
use axum::routing::{get, patch, post, put};
use axum::{Extension, Router};
use common_core::Result;
use common_core::error::{AppError, ContextualError};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedJson,
};
use file_validator::FileValidator;
use std::sync::Arc;
use types_identity::auth::user::UserId;
use types_visual_token::VisualTokenStr;

use crate::UserState;
use crate::services as user_service;
use types_identity::user::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeView,
    UserBriefView, UserInfo,
};

/// 用户模块 HTTP 控制器，处理用户相关的 API 请求
pub struct UserController;

impl ControllerRouter for UserController {
    type State = UserState;

    /// 用户模块的受保护路由
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/me", get(Self::get_user_info))
            .route("/inviter-code", post(Self::generate_inviter_code))
            .route("/nickname", patch(Self::change_nickname))
            .route("/avatar", put(Self::upload_avatar))
            .route("/password", patch(Self::change_password))
            .route("/logout", post(Self::logout))
            .route("/batch", post(Self::get_user_info_batch))
    }

    /// 用户模块无公开路由
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

impl UserController {
    /// 获取当前登录用户的个人信息
    async fn get_user_info(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<UserInfo>> {
        user_service::get_user_info(&state, user_id).await.to_r_ok()
    }

    /// 为当前用户生成唯一邀请码
    async fn generate_inviter_code(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<InviterCodeView>> {
        user_service::generate_inviter_code(&state, user_id)
            .await
            .to_r_ok()
    }

    /// 修改当前用户的昵称
    async fn change_nickname(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangeNicknameParam>,
    ) -> Result<R<String>> {
        user_service::change_nickname(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 上传并更新当前用户头像
    async fn upload_avatar(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        body: Body,
    ) -> Result<R<VisualTokenStr>> {
        // 头像为小文件, 限制在图片校验上限内整体读入(复用 FileValidator 上限常量, 避免两处漂移)
        let file_data = axum::body::to_bytes(body, FileValidator::ALLOW_IMAGE_MAX_SIZE as usize)
            .await
            .map_err(|error| {
                if common_web::body_util::is_body_limit_error(&error) {
                    return ContextualError::warn_without_source(
                        "avatar_too_large",
                        "头像文件大小超过服务器限制",
                        AppError::PayloadTooLarge,
                    )
                    .emit();
                }
                ContextualError::warn(
                    "read_avatar_err",
                    "读取头像文件失败",
                    error,
                    AppError::bad_request("读取头像文件失败"),
                )
                .emit()
            })?;
        if file_data.is_empty() {
            return Err(ContextualError::warn_without_source(
                "avatar_not_found",
                "未找到上传文件",
                AppError::bad_request("未找到上传文件"),
            )
            .emit());
        }

        user_service::update_avatar(&state, user_id, file_data)
            .await
            .to_r_ok()
    }

    /// 修改当前用户的登录密码
    async fn change_password(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangePasswordParam>,
    ) -> Result<R<()>> {
        user_service::change_password(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 登出当前用户并清除所有令牌
    async fn logout(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        user_service::logout(&state, user_id).await.to_r_ok()
    }

    /// 批量获取多个用户的基本信息
    async fn get_user_info_batch(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<GetUserInfoBatchParam>,
    ) -> Result<R<Vec<Option<UserBriefView>>>> {
        user_service::get_user_info_batch(&state, user_id, req)
            .await
            .to_r_ok()
    }
}
