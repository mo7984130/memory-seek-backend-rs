use chrono::{Duration, Utc};
use common::constants::RedisKeys;
use common::{metrics_group, metrics_success, metrics_timer_name, timed};
use entities::user;
use sea_orm::sea_query::Expr;
use sea_orm::sqlx::types::uuid;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
use tracing::{info, warn};
use tokio::task::spawn_blocking;
use std::sync::LazyLock;
use tokio::sync::Semaphore;

use crate::UserState;
use crate::config::GET_USER_INFO_BATCH_MAX_LEN;
use crate::models::{ChangePasswordRequest, InviterCodeDTO, UserInfoDTO, UserInfoVO};
use common::constants::HASHER;
use common::error::AppError;
use common::utils::{DbUtils, MetricsTimerExt};
use common::utils::{rand_utils, FileValidator, encrypt_avatar_token};
use common::utils::{CacheExtension, RedisExt, ResultExt, OptionExt};

use crate::config::{GENERATE_INVITER_CODE_MAX_RETRY, INVITER_CODE_LEN, INVITER_CODE_TTL_SECONDS, USER_INFO_CACHE_TTL_SECS};

/// 密码验证并发信号量，限制同时进行的密码验证数量，防止 CPU 密集型操作抢占 runtime 资源
static PASSWORD_VERIFY_SEM: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(common::constants::get_password_verify_max_concurrency()));

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
    name = "user_get_user_info",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn get_user_info(
    state: &UserState,
    user_id: i64,
) -> Result<user::UserDTO, AppError> {
    metrics_group!("get_user_info");

    // 获取用户
    let user = user::Entity::find()
        .filter(user::Column::Id.eq(user_id))
        .one(&state.db)
        .timed(metrics_timer_name!("get_user_info", "db_query"))
        .await
        .trace_internal_err("db_query_error", "在获取用户信息时 查询数据库错误")?
        .ok_or_warn("user_not_found", "获取用户信息时用户不存在", "用户不存在")?;
    // 加密头像
    let avatar_token = timed!("get_user_info", "encrypt_avatar",
        encrypt_avatar_token(user.avatar_file_id.as_deref(), &state.token_cipher)
    );

    metrics_success!("get_user_info");
    info!(status = "success", user_id = %user_id, "获取用户信息成功");

    Ok(user::UserDTO {
        id: user.id.to_string(),
        username: user.username,
        nickname: user.nickname,
        email: user.email,
        avatar_token,
        created_at: user.created_at.into(),
        refresh_token: None,
        refresh_token_expire_at: None,
        access_token: None,
        access_token_expire_at: None,
    })
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
    name = "user_generate_inviter_code",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn generate_inviter_code(
    state: &UserState,
    user_id: i64
) -> Result<InviterCodeDTO, AppError> {
    metrics_group!("generate_inviter_code");

    // 循环生成邀请码, 防止冲突
    // 最大生成次数为3
    let mut conn = state.redis.get_conn().await.trace_internal_err("redis_conn_error", "生成邀请码时获取Redis连接错误")?;
    for _ in 0..GENERATE_INVITER_CODE_MAX_RETRY {
        let code: String = rand_utils::generate_random_uppercase_str(INVITER_CODE_LEN);
        let key = RedisKeys::user::inviter_code(&code);

        let success: bool = redis::cmd("SET")
            .arg(&key)
            .arg(user_id)
            .arg("EX")
            .arg(INVITER_CODE_TTL_SECONDS)
            .arg("NX")
            .query_async(&mut conn)
            .timed(metrics_timer_name!("generate_inviter_code", "redis_set"))
            .await
            .trace_internal_err("redis_set_error", "生成邀请码时 redis错误")?;

        if success {
            metrics_success!("generate_inviter_code");

            info!(status = "success", "生成邀请码成功");

            return Ok(InviterCodeDTO {
                inviter_code: code,
                expire_at: Utc::now() + Duration::try_seconds(INVITER_CODE_TTL_SECONDS).unwrap()
            });
        }
    }

    warn!("邀请码生成重试耗尽");
    Err(AppError::InternalServerError)
}

/// 修改用户昵称
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
/// - `new_nickname`: 新的昵称字符串
///
/// # 返回
/// 返回更新后的昵称字符串
///
/// # 错误
/// - `AppError`: 用户不存在或数据库更新失败时返回错误
#[tracing::instrument(
    name = "user_change_nickname",
    skip_all,
    fields(user_id = %user_id, new_nickname = %new_nickname)
)]
pub async fn change_nickname(
    state: &UserState,
    user_id: i64,
    new_nickname: String
) -> Result<String, AppError> {
    metrics_group!("change_nickname");

    // 更新昵称
    let update_res = user::Entity::update_many()
        .col_expr(user::Column::Nickname, Expr::value(new_nickname.clone()))
        .filter(user::Column::Id.eq(user_id))
        .exec(&state.db)
        .timed(metrics_timer_name!("change_nickname", "db_update"))
        .await
        .trace_internal_err("db_update_error", "数据库更新昵称失败")?;

    if update_res.rows_affected == 0 {
        return Err(AppError::bad_request("用户不存在"));
    }

    // 删除用户缓存
    // 错误不返回
    let _ = state.redis.delete(&RedisKeys::user::user_info_cache(user_id))
        .timed(metrics_timer_name!("change_nickname", "redis_delete")).await
        .trace_internal_err("redis_delete_error", "删除用户信息缓存失败");

    metrics_success!("change_nickname");
    info!(status = "success", user_id = %user_id, "修改昵称成功");

    Ok(new_nickname)
}

/// 上传并更新用户头像
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_id`: 用户 ID
/// - `file_name`: 上传文件的原始文件名
/// - `file_data`: 头像文件的二进制数据
/// - `content_type`: 文件的 MIME 类型
///
/// # 返回
/// 返回新头像的加密访问 token
///
/// # 错误
/// - `AppError`: 图片校验失败、S3 上传失败、数据库更新失败或用户不存在时返回错误
#[tracing::instrument(
    name = "user_update_avatar",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn update_avatar(
    state: &UserState,
    user_id: i64,
    file_name: String,
    file_data: Vec<u8>,
    content_type: String,
) -> Result<String, AppError> {
    metrics_group!("update_avatar");

    // 校验图片
    let img_metadata = timed!("update_avatar", "validate_image",
        FileValidator::validate_image(&file_data, &file_name, &content_type)
            .trace_bad_request_err("invalid_image", "文件验证失败")?
    ) ;

    // 上传图片
    let new_key = format!("avatars/{}/{}.{}", user_id, uuid::Uuid::new_v4(), &img_metadata.format);
    state.s3_client.upload(&new_key, file_data, &img_metadata.mime_type)
        .timed(metrics_timer_name!("update_avatar", "s3_upload")).await
        .trace_internal_err("s3_upload_error", "上传头像到S3失败")?;

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
                .await
                .trace_internal_err("db_query_error", "在上传头像时 查询头像url发生错误")?
                .ok_or_warn("user_not_found", "上传头像", "用户不存在")?;

            user::ActiveModel {
                id: Set(user_id),
                avatar_file_id: Set(Some(new_key_inner)),
                ..Default::default()
            }.update(txn).await
                .trace_internal_err("db_update_error", "在上传头像时 更新头像url发送错误")?;

            Ok(old_key)
        })
    })
    .timed(metrics_timer_name!("update_avatar", "db_transaction"))
    .await
    // 如果更新数据库失败的话, 删除刚才上传的文件
    .inspect_err(|_| {
        let client = state.s3_client.clone();
        let key = new_key.clone();
        tokio::spawn(async move {
            let _ = client.delete(&key).await
                .trace_internal_err("s3_delete_error", "删除上传的头像失败");
        });
    })?;

    // 删除用户信息缓存
    state.redis.delete(&RedisKeys::user::user_info_cache(user_id))
        .timed(metrics_timer_name!("update_avatar", "redis_delete")).await
        .trace_internal_err("redis_delete_error", "删除用户信息缓存失败")?;

    // 删除旧头像
    // 删除失败, 不返回错误
    if let Some(old_key) = old_key {
        let _ = state.s3_client.delete(&old_key)
            .timed(metrics_timer_name!("update_avatar", "s3_delete")).await
            .trace_internal_err("s3_delete_error", "删除旧头像失败");
    }

    // 生成头像Token
    let avatar_token = encrypt_avatar_token(Some(&new_key), &state.token_cipher)
        .ok_or_else(|| AppError::InternalServerError)?;

    metrics_success!("update_avatar");

    info!(status = "success", user_id = %user_id, "更新头像成功");

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
    name = "user_change_password",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn change_password(
    state: &UserState,
    user_id: i64,
    req: ChangePasswordRequest
) -> Result<(), AppError> {
    metrics_group!("change_password");

    // 新旧密码不可相同
    if req.old_password == req.new_password {
        return Err(AppError::bad_request("新密码不能与旧密码相同"))
    }

    //  获取旧密码
    let old_password: String = user::Entity::find_by_id(user_id)
        .select_only()
        .column(user::Column::Password)
        .into_tuple()
        .one(&state.db)
        .timed(metrics_timer_name!("change_password", "db_query"))
        .await
        .trace_internal_err("db_query_error", "更改密码: 数据库查询用户失败")?
        .ok_or_warn("user_not_found", "更改密码", "用户不存在")?;

    // 获取信号量许可，限制并发密码验证
    let _permit = PASSWORD_VERIFY_SEM.acquire().await
        .map_err(|_| AppError::InternalServerError)?;

    // 校验旧密码
    let is_valid = {
        spawn_blocking(move || {
            HASHER.verify(&req.old_password, &old_password)
        })
        .timed(metrics_timer_name!("change_password", "verify_password")).await
        .map_err(|_| AppError::InternalServerError)?
        .trace_bad_request_err("verify_error", "密码校验错误")?
    };
    if !is_valid {
        return Err(AppError::bad_request("原密码错误"));
    }

    // 加密新密码
    let new_password_hash = {
        let password = req.new_password;
        spawn_blocking(move || {
            HASHER.hash(&password)
        })
        .timed(metrics_timer_name!("change_password", "hash_password")).await
        .map_err(|_| AppError::InternalServerError)?
        .trace_bad_request_err("hash_error", "加密新密码失败")?
    };

    // 更新数据库
    user::ActiveModel {
        id: Set(user_id),
        password: Set(new_password_hash),
        ..Default::default()
    }
    .update(&state.db)
    .timed(metrics_timer_name!("change_password", "db_update"))
    .await
    .trace_internal_err("db_update_error", "更改密码: 数据库更新错误")?;

    // 登出. 清除token
    logout(state, user_id).await?;

    metrics_success!("change_password");
    info!(status = "success", user_id = %user_id, "修改密码成功");

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
    name = "user_logout",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn logout(
    state: &UserState,
    user_id: i64
) -> Result<(), AppError> {
    metrics_group!("logout");

    // 清除refresh_token
    // 清除access_token
    // 清除用户信息缓存
    let cache_key = RedisKeys::user::user_info_cache(user_id);
    let (refresh_token_result, access_token_result, _) = tokio::join!(
        user::ActiveModel {
            id: Set(user_id),
            refresh_token: Set(None),
            refresh_token_expire_at: Set(None),
            ..Default::default()
        }
            .update(&state.db)
            .timed(metrics_timer_name!("logout", "db_update")),
        state.redis.delete(RedisKeys::user::user_access_token(user_id))
            .timed(metrics_timer_name!("logout", "redis_delete")),
        state.redis.delete(&cache_key)
            .timed(metrics_timer_name!("logout", "redis_delete_cache"))
    );
    refresh_token_result.trace_internal_err("db_update_error", "登出时 清除refresh_token失败")?;
    access_token_result.trace_internal_err("redis_delete_error", "删除访问令牌失败")?;

    metrics_success!("logout");
    info!(status = "success", user_id = %user_id, "登出成功");

    Ok(())
}

/// 批量获取多个用户的基本信息（带 Redis 缓存）
///
/// # 参数
/// - `state`: 用户模块共享状态
/// - `user_ids`: 要查询的用户 ID 列表
///
/// # 返回
/// 返回用户信息列表，未找到的用户对应位置为 `None`；空列表直接返回空结果
///
/// # 错误
/// - `AppError`: 列表长度超出限制或数据库查询失败时返回错误
#[tracing::instrument(
    name = "user_get_user_info_batch",
    skip_all,
    fields(user_count = %user_ids.len())
)]
pub async fn get_user_info_batch(
    state: &UserState,
    user_ids: Vec<i64>,
) -> Result<Vec<Option<UserInfoVO>>, AppError> {
    metrics_group!("get_user_info_batch");

    // 空列表直接返回
    if user_ids.is_empty() {
        return Ok(vec![]);
    }

    // 单次获取的最大长度限制为1000
    if user_ids.len() > GET_USER_INFO_BATCH_MAX_LEN {
        return Err(AppError::bad_request("超出了单次获取的最大长度限制"));
    }

    // 带redis缓存的获取用户信息
    let result: Vec<Option<UserInfoDTO>> = state.redis.get_or_load_batch(
        &user_ids,
        |id| RedisKeys::user::user_info_cache(*id),
        USER_INFO_CACHE_TTL_SECS as u64,
        |miss_ids| {
            Box::pin(async move {
                // 只需要id, nickname, avatar_file_id 这三个
                user::Entity::find()
                    .filter(user::Column::Id.is_in(miss_ids))
                    .select_only()
                    .column_as(user::Column::Id, "user_id")
                    .column(user::Column::Nickname)
                    .column(user::Column::AvatarFileId)
                    .into_model::<UserInfoDTO>()
                    .all(&state.db)
                    .timed(metrics_timer_name!("get_user_info_batch", "db_query"))
                    .await
                    .trace_internal_err("db_query_error", "在批量获取用户信息时, 从数据库获取失败")
            })
        },
        |dto| dto.user_id
    )
    .timed(metrics_timer_name!("get_user_info_batch", "redis_cache"))
    .await?;

    metrics_success!("get_user_info_batch");
    info!(status = "success", "批量获取用户信息成功");

    Ok(result.into_iter().map(|opt| {
        opt.map(|dto| UserInfoVO::from_dto(dto, &state.token_cipher))
    }).collect())
}
