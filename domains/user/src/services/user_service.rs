use bytes::Bytes;
use common::error::contextual::ext::{BoolExt, ContextualResultExt, IntoContextualExt};
use common::ext::{RedisExt, ResultInspectErrAsync, ToOk};
use common::time::after;
use common::utils::{MetricsTimerExt, rand_utils};
use common::{
    Result,
    error::{AppError, ContextualError},
    metrics_name, timed,
};
use constants::{PasswordHasher, RedisKeys};
use file_validator::FileValidator;
use sea_orm::sqlx::types::uuid;
use std::sync::LazyLock;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;

use crate::UserState;
use types::auth::user::UserId;
use types::photo::{ImageToken, ImageTokenStr};
use types::user::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeView,
    UpdateAvatarParam, UserBriefView, UserInfo,
};

use crate::config::{GENERATE_INVITER_CODE_MAX_RETRY, INVITER_CODE_LEN, INVITER_CODE_TTL};

/// 密码验证并发信号量，限制同时进行的密码验证数量，防止 CPU 密集型操作抢占 runtime 资源
static PASSWORD_VERIFY_SEM: LazyLock<Semaphore> = LazyLock::new(|| {
    Semaphore::new(
        std::thread::available_parallelism()
            .expect("获取可用并行数错误")
            .into(),
    )
});

/// 获取用户个人信息
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
///
/// # 返回
/// 返回用户 DTO，包含 id、用户名、昵称、邮箱、头像 token 和注册时间
///
/// # 错误
/// - `AppError`: 用户不存在或数据库查询失败时返回错误
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn get_user_info(state: &UserState, user_id: UserId) -> Result<UserInfo> {
    let info = state.repo.get_user_info(user_id).await?;

    info.to_ok()
}

/// 为用户生成唯一邀请码
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
///
/// # 返回
/// 返回邀请码 DTO，包含随机生成的邀请码字符串和过期时间
///
/// # 错误
/// - `AppError`: 邀请码生成重试耗尽（冲突）或 Redis 操作失败时返回内部服务器错误
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn generate_inviter_code(state: &UserState, user_id: UserId) -> Result<InviterCodeView> {
    // 循环生成邀请码, 防止冲突
    // 最大生成次数为3
    let mut conn = state.redis.get_conn().await.into_contextual()?;
    for _ in 0..GENERATE_INVITER_CODE_MAX_RETRY {
        let code: String = rand_utils::generate_random_uppercase_str(INVITER_CODE_LEN);
        let key = RedisKeys::auth::inviter_code(&code);

        let success: bool = redis::cmd("SET")
            .arg(&key)
            .arg(user_id.0)
            .arg("EX")
            .arg(INVITER_CODE_TTL.as_secs())
            .arg("NX")
            .query_async(&mut conn)
            .timed(metrics_name!("redis_set"))
            .await
            .into_contextual()?;

        if success {
            return Ok(InviterCodeView {
                inviter_code: code,
                expire_at: after(INVITER_CODE_TTL),
            });
        }
    }

    Err(ContextualError::error_without_source(
        "inviter_code_generate_max_loop",
        "生成邀请码达到最大尝试次数",
        AppError::InternalServerError,
    )
    .emit())
}

/// 修改用户昵称
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
/// - `param`: 包含新昵称的请求体
///
/// # 返回
/// 返回更新后的昵称字符串
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id, new_nickname = %req.new_nickname)
)]
pub async fn change_nickname(
    state: &UserState,
    user_id: UserId,
    req: ChangeNicknameParam,
) -> Result<String> {
    let new_nickname = state
        .repo
        .change_nickname(user_id, req.new_nickname)
        .await?;

    Ok(new_nickname)
}

/// 上传并更新用户头像
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn update_avatar(
    state: &UserState,
    user_id: UserId,
    file_data: Bytes,
    req: UpdateAvatarParam,
) -> Result<ImageTokenStr> {
    // 校验图片
    let img_metadata = timed!("validate_image", {
        FileValidator::validate_image(&file_data, &req.file_name, &req.content_type).map_err(
            |error| {
                ContextualError::warn_without_source(
                    "file_validation_error",
                    "文件校验失败",
                    AppError::bad_request(error.to_string()),
                )
            },
        )?
    });

    // 上传图片
    let new_key = format!(
        "avatars/{}/{}.{}",
        user_id,
        uuid::Uuid::new_v4(),
        img_metadata.format
    );
    state
        .s3_client
        .upload(&new_key, file_data, &img_metadata.mime_type)
        .timed(metrics_name!("s3_upload"))
        .await
        .into_contextual()?;

    // 更新数据库（事务内查旧头像并更新），失败时删除刚上传的文件
    let old_key = state
        .repo
        .update_avatar(user_id, new_key.clone())
        .await
        .inspect_err_async(|_| async {
            state
                .s3_client
                .delete(&new_key)
                .await
                .into_contextual()
                .emit_if_err();
        })
        .await?;

    // 删除旧头像
    // 删除失败, 不返回错误
    if let Some(old_key) = old_key {
        state
            .s3_client
            .delete(&old_key)
            .timed(metrics_name!("s3_delete"))
            .await
            .into_contextual()
            .emit_if_err();
    }

    // 生成头像Token
    let avatar_token = ImageToken::thumbnail(user_id, new_key).into();

    Ok(avatar_token)
}

/// 修改用户登录密码
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
/// - `req`: 包含旧密码和新密码的请求体
///
/// # 返回
/// 无返回值
///
/// # 错误
/// - `AppError`: 用户不存在、旧密码校验失败、新旧密码相同或数据库更新失败时返回错误
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn change_password(
    state: &UserState,
    user_id: UserId,
    req: ChangePasswordParam,
) -> Result<()> {
    //  获取旧密码
    let old_password = state.repo.query_password_hash(user_id).await?;

    // 获取信号量许可，限制并发密码验证
    let _permit = PASSWORD_VERIFY_SEM
        .acquire()
        .timed(metrics_name!("acquire_permit"))
        .await
        .into_contextual()?;

    // 校验旧密码
    let is_valid = {
        let verify_result =
            spawn_blocking(move || PasswordHasher.verify(&req.old_password, &old_password))
                .timed(metrics_name!("verify_password"))
                .await
                .into_contextual()?;
        verify_result?
    };
    is_valid.true_or_warn(
        "old_password_invalid",
        "修改密码时，原密码错误",
        AppError::bad_request("原密码错误"),
    )?;

    // 加密新密码
    let new_password_hash = {
        let password = req.new_password;
        let hash_result = spawn_blocking(move || PasswordHasher.hash(&password))
            .timed(metrics_name!("hash_password"))
            .await
            .into_contextual()?;
        hash_result?
    };

    // 更新数据库
    state
        .repo
        .update_password(user_id, new_password_hash)
        .await?;

    // 登出. 清除token
    logout(state, user_id).await?;

    Ok(())
}

/// 用户登出，清除所有令牌和缓存
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
///
/// # 返回
/// 无返回值
///
/// # 错误
/// - `AppError`: 数据库更新或 Redis 删除失败时返回错误
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn logout(state: &UserState, user_id: UserId) -> Result<()> {
    state.repo.logout(user_id).await?;

    Ok(())
}

/// 批量获取多个用户的基本信息（带 Redis 缓存）
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `param`: 包含用户 ID 列表的请求体
///
/// # 返回
/// 返回用户信息列表，未找到的用户对应位置为 `None`
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id, count = %req.user_ids.len())
)]
pub async fn get_user_info_batch(
    state: &UserState,
    user_id: UserId,
    req: GetUserInfoBatchParam,
) -> Result<Vec<Option<UserBriefView>>> {
    let user_ids = req.user_ids.into_inner();

    // 带三级缓存的获取用户信息
    let result = state.repo.get_user_info_batch(&user_ids).await?;

    result
        .into_iter()
        .map(|opt| opt.map(|dto| dto.into_brief_view(user_id)).transpose())
        .collect::<Result<Vec<_>>>()
}
