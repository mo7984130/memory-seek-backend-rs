use axum::extract::{Multipart, State};
use axum::routing::{get, post};
use axum::{Extension, Router};
use common::error::AppError;
use common::ext::ResultErrExt;
use common::ext::{OptionExt, ResultRExt};
use common::extractors::ValidatedJson;
use common::models::UserId;
use common::r::R;
use entities::user::UserDTO;
use std::sync::Arc;

use crate::UserState;
use crate::models::{
    ChangeNicknameRequest, ChangePasswordRequest, GetUserInfoBatchRequest, InviterCodeDTO,
    UserInfoVO,
};
use crate::services as user_service;

/// 用户模块 HTTP 控制器，处理用户相关的 API 请求
pub struct UserController;

impl UserController {
    /// 构建用户模块的路由表
    ///
    /// # 返回
    /// 返回包含所有用户相关路由的 `Router`
    pub fn routes() -> Router<Arc<UserState>> {
        Router::new()
            .route("/info", get(Self::get_user_info))
            .route("/inviter-code", get(Self::generate_inviter_code))
            .route("/nickname", post(Self::change_nickname))
            .route("/avatar", post(Self::upload_avatar))
            .route("/password", post(Self::change_password))
            .route("/logout", post(Self::logout))
            .route("/info/batch", post(Self::get_user_info_batch))
    }

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
    ) -> Result<R<UserDTO>, AppError> {
        user_service::get_user_info(&state, user_id.0)
            .await
            .to_r_ok()
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
    ) -> Result<R<InviterCodeDTO>, AppError> {
        user_service::generate_inviter_code(&state, user_id.0)
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
        ValidatedJson(req): ValidatedJson<ChangeNicknameRequest>,
    ) -> Result<R<String>, AppError> {
        user_service::change_nickname(&state, user_id.0, req.new_nickname)
            .await
            .to_r_ok()
    }

    /// 上传并更新当前用户的头像
    ///
    /// # 参数
    /// - `state`: 用户模块共享状态
    /// - `user_id`: 当前登录用户的 ID（从认证中间件提取）
    /// - `multipart`: 包含头像文件的 multipart 表单数据
    ///
    /// # 返回
    /// 返回封装后的头像访问 token
    ///
    /// # 错误
    /// - `AppError`: 表单数据无效、文件校验失败、上传失败或数据库更新失败时返回错误
    async fn upload_avatar(
        State(state): State<Arc<UserState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<String>, AppError> {
        let field = multipart
            .next_field()
            .await
            .trace_to_bad_request_warn("invaild_multipart", "无效的表单数据")?
            .ok_or_warn("mutipart_not_found", "未找到上传文件")?;

        let file_name = field.file_name().unwrap_or("avatar.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpg").to_string();
        let file_data = field
            .bytes()
            .await
            .trace_to_bad_request_warn("read_file_err", "读取文件失败")?;

        let res = user_service::update_avatar(
            &state,
            user_id.0,
            file_name,
            file_data.to_vec(),
            content_type,
        )
        .await?;
        Ok(R::ok(res))
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
        ValidatedJson(req): ValidatedJson<ChangePasswordRequest>,
    ) -> Result<R<()>, AppError> {
        user_service::change_password(&state, user_id.0, req)
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
    ) -> Result<R<()>, AppError> {
        user_service::logout(&state, user_id.0).await.to_r_ok()
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
        ValidatedJson(req): ValidatedJson<GetUserInfoBatchRequest>,
    ) -> Result<R<Vec<Option<UserInfoVO>>>, AppError> {
        let user_ids = req
            .user_ids
            .into_iter()
            .map(|id| id.parse::<i64>())
            .collect::<Result<Vec<i64>, _>>()
            .trace_to_bad_request_warn("invalid_id_format", "id格式错误")?;

        user_service::get_user_info_batch(&state, user_ids)
            .await
            .to_r_ok()
    }
}
