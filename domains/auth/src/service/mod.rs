use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};
use bcrypt::verify;
use chrono::{Duration, Utc};
use common::constants::{redis_keys, RedisKeys};
use common::error::AppError;
use common::utils::ResultExt;
use common::utils::{rand_utils, MetricsConcurrencyGuard};
use common::utils::{BoolExt, MetricsTimer, OptionExt, RedisExt, ToOkExt};
use deadpool_redis::Pool;
use email::EmailClient;
use entities::user;
use entities::user::UserDTO;
use img_url_generator::{encrypt_image_token, ImageToken};
use metrics::counter;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{error::DbErr, RuntimeErr};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter, QuerySelect, Set};
use tokio::task;
use tracing::{info, Span};

pub async fn login(
    db: &DatabaseConnection,
    redis: &Pool,
    request: LoginRequest,
    encryption_key: &[u8; 32],
) -> Result<UserDTO, AppError> {
    let _total_timer = MetricsTimer::start("total_seconds");
    let _concurrency_guard = MetricsConcurrencyGuard::start("concurrency");
    counter!("total_attempts").increment(1);

    // 查找用户
    let user = {
        let _timer = MetricsTimer::start("db_query_duration_seconds");
        user::Entity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Password)
            .column(user::Column::AvatarFileId)
            .filter(
                Condition::any()
                    .add(user::Column::Username.eq(&request.account))
                    .add(user::Column::Email.eq(&request.account))
            )
            .into_tuple::<(i64, String, Option<String>)>()
            .one(db)
            .await
            .trace_internal_err("db_query_error", "查询用户时数据库错误")?
            .ok_or_warn("user_not_found", "登录失败：账号不存在")?
    };

    // 密码验证
    {
        let _timer = MetricsTimer::start("verify_seconds");
        task::spawn_blocking(move || {
            verify(&request.password, &user.1)
        })
            .await
            .trace_internal_err("tokio_error", "线程调度错误")?
            .trace_internal_err("verify_error", "验证密码时发生错误")?
            .ok_or_warn("invalid_password", "密码错误")?
    };

    let new_access_token = rand_utils::generate_random_str(16);
    // access_token
    {
        let _timer = MetricsTimer::start("redis_seconds");
        redis.set_ex(&RedisKeys::user::user_access_token(user.0), &new_access_token, 2 * 60 * 60)
            .await
            .trace_internal_err("redis_error","向Redis存入access_token失败")?;
    }
    // refresh_token
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expire = Utc::now() + Duration::days(30);
    let updated_user = {
        let _timer = MetricsTimer::start("refresh_token_cost_seconds");
        user::ActiveModel {
            id: Set(user.0),
            refresh_token: Set(Some(new_refresh_token.clone())),
            refresh_token_expire_at: Set(Some(new_refresh_token_expire.into())),
            ..Default::default()
        }
            .update(db)
            .await
            .trace_internal_err("db_error","向数据库更新refresh_token错误")?
    };

    // 加密头像token
    let avatar_token = {
        let _timer = MetricsTimer::start("crypto_avatar_token_seconds");
        user.2.as_ref().and_then(|key| {
            encrypt_image_token(&ImageToken::thumbnail(key.clone()), encryption_key).ok()
        })
    };

    // 完成
    info!(status="success", user_id = %user.0, username = %updated_user.username, "用户登录成功");

    Ok(UserDTO {
        id: updated_user.id.to_string(),
        username: updated_user.username,
        nickname: updated_user.nickname,
        email: updated_user.email,
        avatar_token,
        created_at: updated_user.created_at.into(),
        refresh_token: Some(new_refresh_token),
        refresh_token_expire_at: Some(new_refresh_token_expire),
        access_token: Some(new_access_token),
        access_token_expire_at: Some(Utc::now() + Duration::hours(2)),
    })
}

pub async fn register(
    db: &DatabaseConnection,
    redis: &Pool,
    request: RegisterRequest,
) -> Result<UserDTO, AppError> {
    let email_verified = verify_email_verify_code(redis, &request.email, &request.email_verify_code).await?;
    if !email_verified {
        return Err(AppError::bad_request("邮箱验证码错误"));
    }

    let inviter_id = verify_inviter_code(redis, &request.inviter_code).await?;

    let hashed_pw = bcrypt::hash(&request.password, bcrypt::DEFAULT_COST)
        .map_internal_err("密码加密失败")?;
    
    let new_user = user::ActiveModel {
        username: Set(request.username),
        email: Set(request.email),
        password: Set(hashed_pw),
        nickname: Set(request.nickname),
        inviter: Set(inviter_id.into()),
        ..Default::default()
    };

    // 执行插入并捕获唯一约束冲突错误
    let insert_result = new_user.insert(db).await;

    match insert_result {
        Ok(user_model) => {
            // 注册成功，返回 DTO
            UserDTO {
                id: user_model.id.to_string(),
                username: user_model.username,
                nickname: user_model.nickname,
                email: user_model.email,
                avatar_token: None,
                created_at: user_model.created_at.into(),
                refresh_token: None,
                refresh_token_expire_at: None,
                access_token: None,
                access_token_expire_at: None,
            }.ok_res()
        }
        Err(e) => {
            // 尝试提取底层的数据库错误码
            if let DbErr::Query(RuntimeErr::SqlxError(ref sqlx_err)) = e {
                if let Some(pg_err) = sqlx_err.as_database_error() {
                    // Postgres 的唯一约束冲突错误码是 23505
                    if pg_err.code() == Some("23505".into()) {
                        let detail = pg_err.to_string();
                        return if detail.contains("username") || detail.contains("Username") {
                            Err(AppError::bad_request("该用户名已被占用"))
                        } else if detail.contains("email") || detail.contains("Email") {
                            Err(AppError::bad_request("该邮箱已被注册"))
                        } else {
                            Err(AppError::bad_request("记录已存在"))
                        };
                    }
                }
            }
            // 其他数据库错误（如连接断开、字段超长等）
            tracing::error!(target: "logs", "注册时数据库错误：{:?}", e);
            Err(AppError::InternalServerError)
        }
    }
}

pub async fn send_email_code(
    redis: &Pool,
    email_client: &EmailClient,
    req: SendEmailCodeRequest
) -> Result<(), AppError> {
    let code = rand_utils::generate_random_str(6);
    redis.set_ex(&redis_keys::user::email_verify_code(&req.email), &code, 10 * 60).await.map_internal_err("在发送邮箱验证码时 设置redis值错误")?;

    let client = email_client.clone();
    tokio::spawn(async move {
        let html_body = format!(
            r#"
        <p>您的验证码为: <strong>{}</strong></p>
        <p>该验证码有效期为 10 分钟。</p>
        "#,
            code
        ).trim().to_string();

        if let Err(e) = client.send_html(
            "no-reply@memory-seek.ink",
            "寻忆",
            &req.email,
            "寻忆邮箱验证码",
            html_body
        ).await {
            tracing::error!("后台发送邮件失败: {:?}", e);
        }
    });

    Ok(())
}

pub async fn refresh_access_token(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    refresh_token: String
) -> Result<AccessTokenResponse, AppError> {
    verify_refresh_token(db, user_id, &refresh_token).await?;

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
async fn verify_refresh_token(db: &DatabaseConnection, user_id: i64, refresh_token: &str) -> Result<(), AppError> {
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
    if res.refresh_token.as_deref() != Some(refresh_token) {
        return Err(AppError::Unauthorized);
    }
    if let Some(expire_at) = res.refresh_token_expire_at {
        if Utc::now() > expire_at {
            return Err(AppError::Unauthorized);
        }
    } else {
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
