use axum::body::Body;
use axum::extract::State;
use axum::routing::{get, patch, post, put};
use axum::{Extension, Router};
use common::Result;
use common::axum::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedJson,
};
use common::error::{AppError, ContextualError};
use std::sync::Arc;
use types::auth::user::UserId;
use types::visual::VisualTokenStr;

use crate::UserState;
use crate::services as user_service;
use types::user::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeView,
    UserBriefView, UserInfo,
};

/// 用户模块 HTTP 控制器，处理用户相关的 API 请求
pub struct UserController;

impl ControllerRouter for UserController {
    type State = UserState;

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

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

impl UserController {
    /// 获取当前登录用户的个人信息
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    ///
    /// # 返回
    /// 返回封装后的用户 DTO 信息
    ///
    /// # 错误
    /// - `AppError`: 用户不存在或数据库查询失败时返回错误
    async fn get_user_info(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<UserInfo>> {
        user_service::get_user_info(&state, user_id).await.to_r_ok()
    }

    /// 为当前用户生成邀请码
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    ///
    /// # 返回
    /// 返回封装后的邀请码 DTO，包含邀请码字符串和过期时间
    ///
    /// # 错误
    /// - `AppError`: 邀请码生成重试耗尽或 Redis 操作失败时返回错误
    async fn generate_inviter_code(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<InviterCodeView>> {
        user_service::generate_inviter_code(&state, user_id)
            .await
            .to_r_ok()
    }

    /// 修改当前用户的昵称
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    /// - `req`: 包含新昵称的请求体（经过参数校验）
    ///
    /// # 返回
    /// 返回封装后的新昵称字符串
    ///
    /// # 错误
    /// - `AppError`: 用户不存在或数据库更新失败时返回错误
    async fn change_nickname(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangeNicknameParam>,
    ) -> Result<R<String>> {
        user_service::change_nickname(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 上传并更新当前用户的头像
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    /// - `body`: 头像图片的原始字节(request body 即文件)
    ///
    /// # 返回
    /// 返回封装后的头像访问 token
    ///
    /// # 错误
    /// - `AppError`: 表单数据无效、文件校验失败、上传失败或数据库更新失败时返回错误
    async fn upload_avatar(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        body: Body,
    ) -> Result<R<VisualTokenStr>> {
        // 头像为小文件, 限制 20MB 内整体读入(与图片校验上限一致)
        const AVATAR_MAX_BYTES: usize = 20 * 1024 * 1024;
        let file_data = axum::body::to_bytes(body, AVATAR_MAX_BYTES)
            .await
            .map_err(|error| {
                if common::axum::body_util::is_body_limit_error(&error) {
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
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    /// - `req`: 包含旧密码和新密码的请求体（经过参数校验）
    ///
    /// # 返回
    /// 返回封装后的空成功响应
    ///
    /// # 错误
    /// - `AppError`: 用户不存在、旧密码校验失败或数据库更新失败时返回错误
    async fn change_password(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<ChangePasswordParam>,
    ) -> Result<R<()>> {
        user_service::change_password(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 登出当前用户，清除所有令牌
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    ///
    /// # 返回
    /// 返回封装后的空成功响应
    ///
    /// # 错误
    /// - `AppError`: 数据库更新或 Redis 操作失败时返回错误
    async fn logout(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        user_service::logout(&state, user_id).await.to_r_ok()
    }

    /// 批量获取多个用户的基本信息
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `req`: 包含用户 ID 列表的请求体（经过参数校验）
    ///
    /// # 返回
    /// 返回封装后的用户信息列表，未找到的用户对应位置为 `None`
    ///
    /// # 错误
    /// - `AppError`: ID 格式错误、超出批量查询限制或数据库查询失败时返回错误
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
