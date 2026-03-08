use email::EmailClient;
use common::constants::{redis_keys, RedisKeys};
use entities::user;
use entities::user::UserDTO;
use common::error::AppError;
use crate::models::{LoginRequest, RegisterRequest, SendEmailCodeRequest};
use common::utils::RedisExt;
use common::utils::ResultExt;
use common::utils::rand_utils;
use bcrypt::verify;
use chrono::{Duration, Utc};
use deadpool_redis::Pool;
use sea_orm::prelude::{DateTimeUtc, DateTimeWithTimeZone};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QuerySelect, Set};
use serde::Serialize;

#[derive(FromQueryResult)]
struct LoginUser {
    id: u32,
    password: String,
}
pub async fn login(
    db: &DatabaseConnection,
    redis: &Pool,
    request: LoginRequest
) -> Result<UserDTO, AppError> {
    // 验证邮箱或用户名是否存在
    let user = user::Entity::find()
        .select_only()
        .column(user::Column::Id)
        .column(user::Column::Password)
        .filter(
            Condition::any()
                .add(user::Column::Username.eq(&request.account))
                .add(user::Column::Email.eq(&request.account))
        )
        .into_model::<LoginUser>()
        .one(db)
        .await
        .map_internal_err("登录时 数据库查询失败")?
        .ok_or_else(|| {
            AppError::bad_request("账号或密码错误")
        })?;

    // 验证密码
    let valid = verify(&request.password, &user.password)
        .map_internal_err("登陆时 验证密码错误")?;
    if !valid {
        return Err(AppError::bad_request("账号或密码错误"));
    };

    // 生成Token
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expire = Utc::now() + Duration::days(30);
    let new_access_token = rand_utils::generate_random_str(16);
    // 存储access_token
    redis.set_ex(&RedisKeys::user::user_access_token(user.id), &new_access_token, 2 * 60 * 60)
        .await
        .map_internal_err("登录时 向Redis存入access_token失败")?;
    // 存储refresh_token
    let new_user = user::ActiveModel {
        id: Set(user.id),
        refresh_token: Set(Some(new_refresh_token.clone())),
        refresh_token_expire_at: Set(Some(new_refresh_token_expire.into())),
        ..Default::default()
    }
        .update(db).await
        .map_internal_err("登陆时 向数据库更新refresh_token错误")?;

    let dto = UserDTO::from(new_user)
        .with_access_token(new_access_token, Utc::now() + Duration::hours(2))
        .with_refresh_token(new_refresh_token, new_refresh_token_expire);
    Ok(dto)
}

pub async fn register(
    db: &DatabaseConnection,
    redis: &Pool,
    request: RegisterRequest,
) -> Result<UserDTO, AppError> {
    // 效验邮箱验证码
    let email_verified = verify_email_verify_code(redis, &request.email, &request.email_verify_code).await?;
    if !email_verified {
        return Err(AppError::bad_request("邮箱验证码错误"));
    }

    // 效验邀请码
    let inviter_id = verify_inviter_code(redis, &request.inviter_code).await?;

    // 检查用户名/邮箱是否重复
    let exists = user::Entity::find()
        .filter(
            Condition::any()
                .add(user::Column::Username.eq(&request.username))
                .add(user::Column::Email.eq(&request.email))
        )
        .count(db)
        .await
        .map_internal_err("在注册时 数据库查询失败")?;

    if exists > 0 {
        return Err(AppError::bad_request("用户名或邮箱已存在"));
    }

    let hashed_pw = bcrypt::hash(&request.password, bcrypt::DEFAULT_COST)
        .map_internal_err("密码加密失败")?;
    // 插入用户
    let new_user = user::ActiveModel {
        username: Set(request.username),
        email: Set(request.email),
        password: Set(hashed_pw),
        nickname: Set(request.nickname),
        inviter: Set(inviter_id.into()),
        ..Default::default()
    }
        .update(db)
        .await
        .map_internal_err("在注册时 插入用户失败")?;

    Ok(UserDTO::from(new_user))
}

pub async fn send_email_code(
    redis: &Pool,
    email_client: &EmailClient,
    req: SendEmailCodeRequest
) -> Result<(), AppError> {
    let code = rand_utils::generate_random_str(6);

    let html_body = format!(
        r#"
        <p>您的验证码为: <strong>{}</strong></p>
        <p>该验证码有效期为 10 分钟。</p>
        "#,
        code
    ).trim().to_string();
    email_client.send_html(
        "no-reply@memory-seek.driftcloud.ink",
        "寻忆",
        &req.email,
        "寻忆邮箱验证码",
        html_body
    ).await?;

    redis.set_ex(&redis_keys::user::email_verify_code(&req.email), code, 10 * 60).await.map_internal_err("在发送邮箱验证码时 设置redis值错误")?;
    Ok(())
}

#[derive(Serialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub access_token_expire_at: DateTimeUtc
}
pub async fn refresh_access_token(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: u32,
    refresh_token: String
) -> Result<AccessTokenResponse, AppError> {
    // 验证refresh_token
    verify_refresh_token(db, user_id, &refresh_token).await?;

    // 存储新的access_token
    let new_access_token = rand_utils::generate_random_str(16);
    redis.set_ex(&RedisKeys::user::user_access_token(user_id), &new_access_token, 2 * 60 * 60).await?;

    Ok(AccessTokenResponse {
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + Duration::hours(2)
    })
}

async fn verify_email_verify_code(redis: &Pool, email: &str, code: &str) -> Result<bool, AppError> {
    let stored_code: Option<String> = redis.get_as(&RedisKeys::user::email_verify_code(email))
        .await
        .map_internal_err("验证邮箱验证码时 获取redis值错误")?;
    Ok(stored_code.map_or(false, |v| v == code))
}

async fn verify_inviter_code(redis: &Pool, inviter_code: &str) -> Result<u32, AppError> {
    redis.get_as(&RedisKeys::user::inviter_code(inviter_code))
        .await
        .map_internal_err("验证邀请码时 获取redis值错误")?
        .ok_or_else(|| AppError::bad_request("邀请码无效. 不存在或已过期"))
}

#[derive(FromQueryResult)]
struct RefreshTokenValidation {
    refresh_token: Option<String>,
    refresh_token_expire_at: Option<DateTimeWithTimeZone>
}
async fn verify_refresh_token(db: &DatabaseConnection, user_id: u32, refresh_token: &str) -> Result<(), AppError> {
    // 获取数据库中的refresh_token
    let res = user::Entity::find()
        .select_only()
        .column(user::Column::RefreshToken)
        .column(user::Column::RefreshTokenExpireAt)
        .filter(user::Column::Id.eq(user_id))
        .into_model::<RefreshTokenValidation>()
        .one(db)
        .await
        .map_internal_err("刷新access_token时 查询 数据库RefreshToken 失败")?
        .ok_or_else(|| AppError::bad_request("用户不存在"))?;
    // 判断是否一致
    if res.refresh_token.as_deref() != Some(refresh_token) {
        return Err(AppError::Unauthorized);
    }
    // 判断是否过期
    if let Some(expire_at) = res.refresh_token_expire_at {
        if Utc::now() > expire_at {
            return Err(AppError::Unauthorized);
        }
    } else {
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
