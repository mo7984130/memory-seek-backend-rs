use oss::S3Client;
use common::constants::RedisKeys;
use entities::user;
use common::error::AppError;
use crate::models::ChangePasswordRequest;
use common::utils::DbUtils;
use common::utils::{CacheExtension, RedisExt};
use common::utils::ResultExt;
use common::utils::{rand_utils, FileValidator};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{DateTime, Duration, Utc};
use deadpool_redis::Pool;
use sea_orm::sea_query::Expr;
use sea_orm::sqlx::types::uuid;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sea_orm::{ColumnTrait, FromQueryResult, QuerySelect};
use serde::{Deserialize, Serialize};
use entities::user::UserDTO;

pub async fn get_user_info(
    db: &DatabaseConnection,
    user_id: i64
) -> Result<UserDTO, AppError> {
    let user = user::Entity::find()
        .filter(user::Column::Id.eq(user_id))
        .one(db)
        .await
        .map_internal_err("在获取用户信息时 查询数据库错误")?
        .ok_or_else(|| AppError::bad_request("用户不存在"))?;
    Ok(UserDTO::from(user))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct  InviterCodeDTO {
    inviter_code: String,
    expires_at: DateTime<Utc>,
}
pub async fn generate_inviter_code(
    redis: &Pool,
    user_id: i64
) -> Result<InviterCodeDTO, AppError> {
    loop {
        let code: String = rand_utils::generate_random_str(6);
        let key = RedisKeys::user::inviter_code(&code);

        let mut conn = redis.get_conn().await?;

        let success: bool = redis::cmd("SET")
            .arg(&key)
            .arg(user_id)
            .arg("EX")
            .arg(600)
            .arg("NX")
            .query_async(&mut conn)
            .await
            .map_internal_err("生成邀请码时 redis错误")?;

        if success {
            return Ok(InviterCodeDTO {
                inviter_code: code,
                expires_at: Utc::now() + Duration::minutes(10)
            });
        }
    }
}

pub async fn change_nickname(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    new_nickname: String
) -> Result<String, AppError> {
    let update_res = user::Entity::update_many()
        .col_expr(user::Column::Nickname, Expr::value(new_nickname.clone()))
        .filter(user::Column::Id.eq(user_id))
        .exec(db)
        .await
        .map_internal_err("数据库更新昵称失败")?;
    if update_res.rows_affected == 0 {
        return Err(AppError::bad_request("用户不存在"));
    }

    redis.delete(&RedisKeys::user::user_info_cache(user_id)).await?;
    Ok(new_nickname)
}

pub async fn update_avatar(
    db: &DatabaseConnection,
    redis: &Pool,
    s3_client: &S3Client,
    user_id: i64,
    file_name: String,
    file_date: Vec<u8>,
    content_type: String
) -> Result<String, AppError> {
    // 验证图片
    let img_metadata = FileValidator::validate_image(&file_date, file_name, content_type)
        .to_bad_request_error()?;

    let new_key = format!("avatars/{}/{}.{}", user_id, uuid::Uuid::new_v4(), &img_metadata.format);

    s3_client.upload(&new_key, file_date, &img_metadata.mime_type).await?;

    let new_key_for_db = new_key.clone();
    let old_key = DbUtils::write(db, move |txn| {
        let new_key_inner = new_key_for_db;

        Box::pin(async move {
            let old_key: Option<String> = user::Entity::find_by_id(user_id)
                .select_only()
                .column(user::Column::AvatarUrl)
                .into_values::<Option<String>, user::Column>() // 关键：直接转为值
                .one(txn)
                .await
                .map_internal_err("在上传头像时 查询头像url发生错误")?
                .ok_or(AppError::bad_request("用户不存在"))?;

            user::ActiveModel {
                id: Set(user_id),
                avatar_url: Set(Some(new_key_inner)),
                ..Default::default()
            }.update(txn).await
                .map_internal_err("在上传头像时 更新头像url发送错误")?;

            Ok(old_key)
        })
    }).await?;

    // 删除redis缓存
    redis.delete(&RedisKeys::user::user_info_cache(user_id)).await?;
    // 删除OSS存储
    if let Some(old_key) = old_key {
        s3_client.delete(&old_key).await?;
    }

    Ok(s3_client.get_url(&new_key))
}

pub async fn change_password(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    req: ChangePasswordRequest
) -> Result<(), AppError> {
    // 查询用户
    let user = user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_internal_err("更改密码: 数据库查询用户失败")?
        .ok_or_else(|| AppError::BadRequest("用户不存在".into()))?;

    // 验证密码
    let is_valid = verify(&req.old_password, &user.password)
        .map_bad_request_err("密码效验错误")?;
    if !is_valid {
        return Err(AppError::BadRequest("原密码错误".into()));
    }

    // 修改密码
    let new_password_hash = hash(req.new_password, DEFAULT_COST).map_bad_request_err("加密新密码失败")?;
    let active_user: user::ActiveModel = user::ActiveModel {
        id: Set(user_id),
        password: Set(new_password_hash),
        ..Default::default()
    };
    active_user.update(db).await
        .map_internal_err("更改密码: 数据库更新错误")?;

    // 删除token
    logout(db, redis, user_id).await?;

    Ok(())
}

pub async fn logout(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64
) -> Result<(), AppError> {
    // 删除数据库的refresh_token
    user::ActiveModel {
        id: Set(user_id),
        refresh_token: Set(None),
        refresh_token_expire_at: Set(None),
        ..Default::default()
    }.update(db).await
        .map_internal_err("登出时 清除refresh_token失败")?;

    // 删除redis里的access_token
    redis.delete(&RedisKeys::user::user_access_token(user_id)).await?;

    Ok(())
}

#[derive(Serialize, FromQueryResult, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoDTO {
    pub user_id: i64,
    pub nickname: String,
    pub avatar_url: Option<String>,
}
pub async fn get_user_info_batch(
    db: &DatabaseConnection,
    redis: &Pool,
    user_ids: Vec<i64>
) -> Result<Vec<Option<UserInfoDTO>>, AppError> {
    redis.get_or_load_batch(
        user_ids,
        |id| RedisKeys::user::user_info_cache(*id),
        1 * 24 * 60 * 60,
        |miss_ids| {
            Box::pin(async move {
                user::Entity::find()
                    .filter(user::Column::Id.is_in(miss_ids))
                    .select_only()
                    .column_as(user::Column::Id, "user_id")
                    .column(user::Column::Nickname)
                    .column(user::Column::AvatarUrl)
                    .into_model::<UserInfoDTO>()
                    .all(db)
                    .await
                    .map_internal_err("在批量获取用户信息时, 从数据库获取失败")
            })
        },
        |dto| dto.user_id
    ).await
}