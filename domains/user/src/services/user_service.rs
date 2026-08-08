use bytes::Bytes;
use chrono::{Duration, Utc};
use common::{metrics_group, metrics_name, metrics_success, timed};
use constants::{PasswordHasher, RedisKeys};
use sea_orm::sea_query::Expr;
use sea_orm::sqlx::types::uuid;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
use std::sync::LazyLock;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;

use crate::UserState;
use crate::models::{UserInfoRow, user_brief_view_from_dto};
use common::ext::{CacheExtension, OptionExt, RedisExt, ResultInspectErrAsync, TraceExt, log_err};
use common::utils::{DbUtils, MetricsTimerExt};
use common::utils::{FileValidator, rand_utils};
use common::{Result, error::AppError};
use types::auth::user::{self, UserId};
use types::photo::ImageToken;
use types::user::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeView,
    UpdateAvatarParam, UserBriefView, UserInfo,
};

use crate::config::{
    GENERATE_INVITER_CODE_MAX_RETRY, INVITER_CODE_LEN, INVITER_CODE_TTL_SECONDS,
    USER_INFO_CACHE_TTL_SECS,
};

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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn get_user_info(state: &UserState, user_id: UserId) -> Result<UserInfo> {
    metrics_group!();

    // 获取用户
    let user = user::Entity::find()
        .filter(user::Column::Id.eq(user_id))
        .one(&state.db)
        .timed(metrics_name!("db_query"))
        .await?
        .ok_or_warn_bad_request("user_not_found", "用户不存在", "用户不存在")?;
    metrics_success!();

    let user_record = user::UserRecord::from(user);
    let mut user_info = user::create_user_info(&user_record);
    user_info.avatar_token = ImageToken::encrypt_avatar_token(
        &state.token_cipher,
        user_record.avatar_file_id.as_deref(),
        user_id,
    );
    Ok(user_info)
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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn generate_inviter_code(state: &UserState, user_id: UserId) -> Result<InviterCodeView> {
    metrics_group!();

    // 循环生成邀请码, 防止冲突
    // 最大生成次数为3
    let mut conn = state.redis.get_conn().await?;
    for _ in 0..GENERATE_INVITER_CODE_MAX_RETRY {
        let code: String = rand_utils::generate_random_uppercase_str(INVITER_CODE_LEN);
        let key = RedisKeys::auth::inviter_code(&code);

        let success: bool = redis::cmd("SET")
            .arg(&key)
            .arg(user_id.0)
            .arg("EX")
            .arg(INVITER_CODE_TTL_SECONDS)
            .arg("NX")
            .query_async(&mut conn)
            .timed(metrics_name!("redis_set"))
            .await?;

        if success {
            metrics_success!();

            return Ok(InviterCodeView {
                inviter_code: code,
                expire_at: Utc::now() + Duration::try_seconds(INVITER_CODE_TTL_SECONDS).unwrap(),
            });
        }
    }

    Err(log_err(
        "inviter_code_generate_max_loop",
        "生成邀请码达到最大尝试次数",
        AppError::InternalServerError,
    ))
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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id, new_nickname = %req.new_nickname)
)]
pub async fn change_nickname(
    state: &UserState,
    user_id: UserId,
    req: ChangeNicknameParam,
) -> Result<String> {
    metrics_group!();

    // 更新昵称
    let new_nickname = req.new_nickname;
    user::Entity::update_many()
        .col_expr(user::Column::Nickname, Expr::value(new_nickname.clone()))
        .filter(user::Column::Id.eq(user_id))
        .exec(&state.db)
        .timed(metrics_name!("db_update"))
        .await?;

    // 删除用户缓存
    // 错误不返回
    let _ = state
        .redis
        .del(&RedisKeys::auth::user_info_cache(user_id))
        .timed(metrics_name!("redis_delete"))
        .await
        .trace();

    metrics_success!();

    Ok(new_nickname)
}

/// 上传并更新用户头像
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn update_avatar(
    state: &UserState,
    user_id: UserId,
    file_data: Bytes,
    req: UpdateAvatarParam,
) -> Result<String> {
    metrics_group!();

    // 校验图片
    let img_metadata = timed!("validate_image", {
        FileValidator::validate_image(file_data.as_ref(), &req.file_name, &req.content_type)?
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
        .await?;

    // 获取旧头像key并更新数据库
    let new_key_for_db = new_key.clone();
    let old_key = DbUtils::write(&state.db, move |txn| {
        let new_key_inner = new_key_for_db;
        Box::pin(async move {
            let old_key: Option<String> = user::Entity::find_by_id(user_id)
                .select_only()
                .column(user::Column::AvatarFileId)
                .into_values::<Option<String>, user::Column>()
                .one(txn)
                .await?
                .ok_or_warn_bad_request("user_not_found", "用户不存在", "用户不存在")?;

            user::ActiveModel {
                id: Set(user_id),
                avatar_file_id: Set(Some(new_key_inner)),
                ..Default::default()
            }
            .update(txn)
            .await?;

            Ok(old_key)
        })
    })
    .timed(metrics_name!("db_transaction"))
    .await
    // 如果更新数据库失败的话, 删除刚才上传的文件
    .inspect_err_async(|_| async {
        let _ = state
            .s3_client
            .delete(&new_key)
            .await
            .map_err(AppError::from);
    })
    .await?;

    // 删除用户信息缓存
    state
        .redis
        .del(&RedisKeys::auth::user_info_cache(user_id))
        .timed(metrics_name!("redis_delete"))
        .await?;

    // 删除旧头像
    // 删除失败, 不返回错误
    if let Some(old_key) = old_key {
        let _ = state
            .s3_client
            .delete(&old_key)
            .timed(metrics_name!("s3_delete"))
            .await
            .map_err(AppError::from);
    }

    // 生成头像Token
    let avatar_token =
        ImageToken::encrypt_avatar_token(&state.token_cipher, Some(&new_key), user_id).ok_or_warn(
            "encrypt_avatar_token_err",
            "加密头像Token错误",
            AppError::InternalServerError,
        )?;

    metrics_success!();

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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn change_password(
    state: &UserState,
    user_id: UserId,
    req: ChangePasswordParam,
) -> Result<()> {
    metrics_group!();

    // 新旧密码不可相同
    if req.old_password == req.new_password {
        return Err(AppError::bad_request("新密码不能与旧密码相同"));
    }

    // 确认密码必须与新密码一致
    if req.new_password != req.confirm_password {
        return Err(AppError::bad_request("两次输入的新密码不一致"));
    }

    //  获取旧密码
    let old_password: String = user::Entity::find_by_id(user_id)
        .select_only()
        .column(user::Column::Password)
        .into_tuple()
        .one(&state.db)
        .timed(metrics_name!("db_query"))
        .await?
        .ok_or_warn_bad_request("user_not_found", "用户不存在", "用户不存在")?;

    // 获取信号量许可，限制并发密码验证
    let _permit = PASSWORD_VERIFY_SEM
        .acquire()
        .timed(metrics_name!("acquire_permit"))
        .await?;

    // 校验旧密码
    let is_valid = {
        spawn_blocking(move || PasswordHasher.verify(&req.old_password, &old_password))
            .timed(metrics_name!("verify_password"))
            .await??
    };
    if !is_valid {
        return Err(AppError::bad_request("原密码错误"));
    }

    // 加密新密码
    let new_password_hash = {
        let password = req.new_password;
        spawn_blocking(move || PasswordHasher.hash(&password))
            .timed(metrics_name!("hash_password"))
            .await??
    };

    // 更新数据库
    user::ActiveModel {
        id: Set(user_id),
        password: Set(new_password_hash),
        ..Default::default()
    }
    .update(&state.db)
    .timed(metrics_name!("db_update"))
    .await?;

    // 登出. 清除token
    logout(state, user_id).await?;

    metrics_success!();

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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn logout(state: &UserState, user_id: UserId) -> Result<()> {
    metrics_group!();

    // 清除refresh_token
    // 清除access_token
    // 清除用户信息缓存
    let cache_key = RedisKeys::auth::user_info_cache(user_id);
    let (refresh_token_result, access_token_result, _) = tokio::join!(
        user::ActiveModel {
            id: Set(user_id),
            refresh_token: Set(None),
            refresh_token_expire_at: Set(None),
            ..Default::default()
        }
        .update(&state.db)
        .timed(metrics_name!("db_update")),
        state
            .redis
            .del(RedisKeys::auth::user_access_token(user_id))
            .timed(metrics_name!("redis_delete")),
        state
            .redis
            .del(&cache_key)
            .timed(metrics_name!("redis_delete_cache"))
    );
    refresh_token_result?;
    access_token_result?;

    metrics_success!();

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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id, count = %req.user_ids.len())
)]
pub async fn get_user_info_batch(
    state: &UserState,
    user_id: UserId,
    req: GetUserInfoBatchParam,
) -> Result<Vec<Option<UserBriefView>>> {
    metrics_group!();

    let user_ids = req.user_ids.into_inner();

    // 带redis缓存的获取用户信息
    let result: Vec<Option<UserInfoRow>> = state
        .redis
        .get_or_load_batch(
            &user_ids,
            |id| RedisKeys::auth::user_info_cache(*id),
            USER_INFO_CACHE_TTL_SECS as u64,
            |miss_ids| {
                Box::pin(async move {
                    // 使用 ? 运算符进行错误处理
                    let users: Vec<UserInfoRow> = user::Entity::find()
                        .filter(user::Column::Id.is_in(miss_ids))
                        .select_only()
                        .column_as(user::Column::Id, "user_id")
                        .column(user::Column::Nickname)
                        .column(user::Column::AvatarFileId)
                        .into_model::<UserInfoRow>()
                        .all(&state.db)
                        .await?; // 这里现在可以安全使用 ? 了，因为它在一个返回 Result 的块中

                    // 返回 Result::Ok
                    Ok(users)
                })
            },
            |dto| dto.user_id,
        )
        .timed(metrics_name!("redis_cache"))
        .await?;

    metrics_success!();

    Ok(result
        .into_iter()
        .map(|opt| opt.map(|dto| user_brief_view_from_dto(dto, &state.token_cipher, user_id)))
        .collect())
}
