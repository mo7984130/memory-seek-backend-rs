use axum::body::Bytes;
use chrono::{Duration, Utc};
use common::constants::RedisKeys;
use common::{metrics_group, metrics_success, timed};
use common::utils::HashAlgorithm;
use deadpool_redis::Pool;
use entities::user;
use common::models::ImageToken;
use common::utils::TokenCipher;
use sea_orm::sea_query::Expr;
use sea_orm::sqlx::types::uuid;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info, warn};
use tokio::task::spawn_blocking;

use crate::config::GET_USER_INFO_BATCH_MAX_LEN;
use crate::models::{ChangePasswordRequest, InviterCodeDTO, UserInfoDTO, UserInfoVO};
use common::error::AppError;
use common::utils::{DbUtils, MetricsTimerExt};
use common::utils::{rand_utils, FileValidator};
use common::utils::{CacheExtension, RedisExt, ResultExt, OptionExt};
use oss::S3Client;

use crate::config::{GENERATE_INVITER_CODE_MAX_RETRY, INVITER_CODE_LEN, INVITER_CODE_TTL_SECONDS, USER_INFO_CACHE_TTL_SECS};

/// 获取用户个人信息
#[tracing::instrument(
    name = "user_get_user_info",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn get_user_info(
    db: &DatabaseConnection,
    user_id: i64,
    token_cipher: &TokenCipher,
) -> Result<user::UserDTO, AppError> {
    metrics_group!("user::get_user_info");

    // 获取用户
    let user = user::Entity::find()
        .filter(user::Column::Id.eq(user_id))
        .one(db)
        .timed("user::get_user_info:db_query")
        .await
        .trace_internal_err("db_query_error", "在获取用户信息时 查询数据库错误")?
        .ok_or_warn("user_not_found", "获取用户信息时用户不存在", "用户不存在")?;
    // 加密头像
    let avatar_token = timed!("user::get_user_info:encrypt_avatar",
        user.avatar_file_id.as_ref()
            .and_then(|key|
                token_cipher.encrypt(&ImageToken::thumbnail(key.clone()), Some(key))
                .inspect_err(|e| warn!(error = %e, "加密头像失败"))
                .ok()
            )
    );

    metrics_success!("user::get_user_info");
    info!(status = "success", user_id = %user_id, "获取用户信息成功");

    Ok(user::UserDTO {
        id: user.id.to_string(),
        username: user.username,
        nickname: user.nickname,
        email: user.email,
        avatar_token,
        created_at: user.created_at,
        refresh_token: user.refresh_token,
        refresh_token_expire_at: user.refresh_token_expire_at.map(|dt| dt.with_timezone(&Utc)),
        access_token: None,
        access_token_expire_at: None,
    })
}

/// 生成邀请码
#[tracing::instrument(
    name = "user_generate_inviter_code",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn generate_inviter_code(
    redis: &Pool,
    user_id: i64
) -> Result<InviterCodeDTO, AppError> {
    metrics_group!("user::generate_inviter_code");

    // 循环生成邀请码, 防止冲突
    // 最大生成次数为3
    let mut conn = redis.get_conn().await.trace_internal_err("redis_conn_error", "生成邀请码时获取Redis连接错误")?;
    for _ in 0..GENERATE_INVITER_CODE_MAX_RETRY {
        let code: String = rand_utils::generate_random_str(INVITER_CODE_LEN);
        let key = RedisKeys::user::inviter_code(&code);

        let success: bool = redis::cmd("SET")
            .arg(&key)
            .arg(user_id)
            .arg("EX")
            .arg(INVITER_CODE_TTL_SECONDS)
            .arg("NX")
            .query_async(&mut conn)
            .timed("user::generate_inviter_code:redis_set")
            .await
            .trace_internal_err("redis_set_error", "生成邀请码时 redis错误")?;

        if success {
            metrics_success!("user::generate_inviter_code");

            info!(status = "success", "生成邀请码成功");

            return Ok(InviterCodeDTO {
                inviter_code: code,
                expire_at: Utc::now() + Duration::seconds(INVITER_CODE_TTL_SECONDS)
            });
        }
    }

    warn!("邀请码生成重试耗尽");
    Err(AppError::InternalServerError)
}

/// 修改昵称
#[tracing::instrument(
    name = "user_change_nickname",
    skip_all,
    fields(user_id = %user_id, new_nickname = %new_nickname)
)]
pub async fn change_nickname(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    new_nickname: String
) -> Result<String, AppError> {
    metrics_group!("user::change_nickname");

    // 更新昵称
    let update_res = user::Entity::update_many()
        .col_expr(user::Column::Nickname, Expr::value(new_nickname.clone()))
        .filter(user::Column::Id.eq(user_id))
        .exec(db)
        .timed("user::change_nickname:db_update")
        .await
        .trace_internal_err("db_update_error", "数据库更新昵称失败")?;

    if update_res.rows_affected == 0 {
        return Err(AppError::bad_request("用户不存在"));
    }

    // 删除用户缓存
    // 错误不返回
    let _ = redis.delete(&RedisKeys::user::user_info_cache(user_id))
        .timed("user::change_nickname:redis_delete").await
        .trace_internal_err("redis_delete_error", "删除用户缓存失败");


    metrics_success!("user::change_nickname");
    info!(status = "success", user_id = %user_id, "修改昵称成功");

    Ok(new_nickname)
}

pub struct UploadAvatarFile {
    pub file_name: String,
    pub file_data: Bytes,
    pub content_type: String
}
/// 上传头像
#[tracing::instrument(
    name = "user_update_avatar",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn update_avatar(
    db: &DatabaseConnection,
    redis: &Pool,
    s3_client: &S3Client,
    token_cipher: &TokenCipher,
    user_id: i64,
    file: UploadAvatarFile,
) -> Result<String, AppError> {
    metrics_group!("user::update_avatar");

    // 效验图片
    let img_metadata = timed!("user::update_avatar:validate_image",
        FileValidator::validate_image(&file.file_data, file.file_name, file.content_type)
            .trace_bad_request_err("invalid_image", "文件验证失败")?
    ) ;

    // 上传图片
    let new_key = format!("avatars/{}/{}.{}", user_id, uuid::Uuid::new_v4(), &img_metadata.format);
    s3_client.upload(&new_key, file.file_data, &img_metadata.mime_type)
        .timed("user::update_avatar:s3_upload").await
        .trace_internal_err("s3_upload_error", "上传头像到S3失败")?;

    // 获取旧头像key并更新数据库
    let new_key_for_db = new_key.clone();
    let old_key = DbUtils::write(db, move |txn| {
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
            }
                .update(txn)
                .await
                .trace_internal_err("db_update_error", "在上传头像时 更新头像url发送错误")?;

            Ok(old_key)
        })
    })
    .timed("user::update_avatar:db_transaction")
    .await
    // 如果更新数据库失败的话, 删除刚才上传的文件
    .inspect_err(|_| {
        let client = s3_client.clone();
        let key = new_key.clone();
        tokio::spawn(async move {
            let _ = client.delete(&key).await
                .trace_internal_err("s3_delete_error", "删除上传的头像失败");
        });
    })?;

    // 删除用户信息缓存
    redis.delete(&RedisKeys::user::user_info_cache(user_id))
        .timed("user::update_avatar:redis_delete").await
        .trace_internal_err("redis_delete_error", "删除用户信息缓存失败")?;

    // 删除旧头像
    // 删除失败, 不返回错误
    if let Some(old_key) = old_key {
        let _ = s3_client.delete(&old_key)
            .timed("user::update_avatar:s3_delete").await
            .trace_internal_err("s3_delete_error", "删除旧头像失败");
    }

    // 生成头像Token
    let avatar_token = timed!("user::update_avatar:encrypt_token",
        token_cipher.encrypt(&ImageToken::thumbnail(new_key.clone()), Some(&new_key))
            .trace_internal_err("encrypt_token_error", "生成头像token失败")?
    );

    metrics_success!("user::update_avatar");

    info!(status = "success", user_id = %user_id, "更新头像成功");

    Ok(avatar_token)
}

/// 修改密码
#[tracing::instrument(
    name = "user_change_password",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn change_password(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    req: ChangePasswordRequest,
    hasher: &HashAlgorithm,
    password_verify_semaphore: &Arc<Semaphore>,
) -> Result<(), AppError> {
    metrics_group!("user::change_password");

    // 新旧密码不可相同
    if req.old_password == req.new_password {
        return Err(AppError::bad_request("新密码不能与旧密码相同"))
    }

    //  获取旧密码
    let old_password: String = user::Entity::find_by_id(user_id)
        .select_only()
        .column(user::Column::Password)
        .into_tuple()
        .one(db)
        .timed("user::change_password:db_query")
        .await
        .trace_internal_err("db_query_error", "更改密码: 数据库查询用户失败")?
        .ok_or_warn("user_not_found", "更改密码", "用户不存在")?;

    // 效验旧密码
    let is_valid = {
        let _permit = password_verify_semaphore
            .acquire()
            .await
            .trace_internal_err("semaphore_error", "获取密码验证信号量失败")?;

        let password_clone = req.old_password.clone();
        let stored_hash = old_password.clone();
        let result: Result<(bool, HashAlgorithm), AppError> = spawn_blocking(move || HashAlgorithm::verify_and_detect(&password_clone, &stored_hash))
            .await
            .trace_internal_err("spawn_blocking_error", "密码验证任务执行失败")?;
        result.trace_internal_err("verify_password_error", "密码验证内部错误")?.0
    };
    if !is_valid {
        return Err(AppError::bad_request("原密码错误"));
    }

    // 加密新密码
    let new_password_hash = {
        let _permit = password_verify_semaphore
            .acquire()
            .await
            .trace_internal_err("semaphore_error", "获取密码验证信号量失败")?;

        let password = req.new_password.clone();
        let hasher_clone = hasher.clone();
        let result: Result<String, AppError> = spawn_blocking(move || hasher_clone.hash(&password))
            .await
            .trace_internal_err("spawn_blocking_error", "密码哈希任务执行失败")?;
        result.trace_internal_err("hash_error", "加密新密码失败")?
    };

    // 更新数据库
    user::ActiveModel {
        id: Set(user_id),
        password: Set(new_password_hash),
        ..Default::default()
    }
    .update(db)
    .timed("user::change_password:db_update")
    .await
    .trace_internal_err("db_update_error", "更改密码: 数据库更新错误")?;

    // 登出. 清除token
    logout(db, redis, user_id).await?;

    metrics_success!("user::change_password");
    info!(status = "success", user_id = %user_id, "修改密码成功");

    Ok(())
}

/// 登出
#[tracing::instrument(
    name = "user_logout",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn logout(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64
) -> Result<(), AppError> {
    metrics_group!("user::logout");

    // 清除refresh_token
    // 清除access_token
    let (refresh_token_result, access_token_result) = tokio::join!(
        user::ActiveModel {
            id: Set(user_id),
            refresh_token: Set(None),
            refresh_token_expire_at: Set(None),
            ..Default::default()
        }
            .update(db)
            .timed("user::logout:db_update"),
        redis.delete(RedisKeys::user::user_access_token(user_id))
            .timed("user::logout:redis_delete")
    );
    refresh_token_result.trace_internal_err("db_update_error", "登出时 清除refresh_token失败")?;
    access_token_result.trace_internal_err("redis_delete_error", "删除访问令牌失败")?;

    metrics_success!("user::logout");
    info!(status = "success", user_id = %user_id, "登出成功");

    Ok(())
}

/// 批量获取其他人的信息
#[tracing::instrument(
    name = "user_get_user_info_batch",
    skip_all,
    fields(user_count = %user_ids.len())
)]
pub async fn get_user_info_batch(
    db: &DatabaseConnection,
    redis: &Pool,
    user_ids: Vec<i64>,
    token_cipher: &TokenCipher,
) -> Result<Vec<Option<UserInfoVO>>, AppError> {
    metrics_group!("user::get_user_info_batch");

    // 空列表直接返回
    if user_ids.is_empty() {
        return Ok(vec![]);
    }

    // 单次获取的最大长度限制为1000
    if user_ids.len() > GET_USER_INFO_BATCH_MAX_LEN {
        return Err(AppError::bad_request("超出了单次获取的最大长度限制"));
    }

    // 带redis缓存的获取用户信息
    let result: Vec<Option<UserInfoDTO>> = redis.get_or_load_batch(
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
                    .all(db)
                    .timed("user::get_user_info_batch:db_query")
                    .await
                    .trace_internal_err("db_query_error", "在批量获取用户信息时, 从数据库获取失败")
            })
        },
        |dto| dto.user_id
    )
    .timed("user::get_user_info_batch:redis_cache")
    .await?;

    metrics_success!("user::get_user_info_batch");
    info!(status = "success", "批量获取用户信息成功");

    Ok(result.into_iter().map(|opt| {
        opt.map(|dto| UserInfoVO::from_dto(dto, token_cipher))
    }).collect())
}
