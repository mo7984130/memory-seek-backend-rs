use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};
use bcrypt::verify;
use chrono::{Duration, Utc};
use common::constants::{redis_keys, RedisKeys};
use common::error::AppError;
use common::utils::{MetricsTimerExt, ResultExt};
use common::utils::rand_utils;
#[cfg(feature = "metrics")]
use common::utils::{MetricsConcurrencyGuard, MetricsTimer};
use common::utils::{BoolExt, OptionExt, RedisExt, ToOkExt};
use deadpool_redis::Pool;
use email::EmailClient;
use entities::user;
use entities::user::UserDTO;
use img_url_generator::{encrypt_image_token, ImageToken};
#[cfg(feature = "metrics")]
use metrics::counter;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{error::DbErr, RuntimeErr};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter, QuerySelect, Set};
use tokio::task;
use tracing::{error, info, warn};

#[tracing::instrument(
    name = "auth_login",
    skip_all,
    fields(
            account = %req.account
    )
)]
pub async fn login(
    db: &DatabaseConnection,
    redis: &Pool,
    req: LoginRequest,
    encryption_key: &[u8; 32],
) -> Result<UserDTO, AppError> {
    #[cfg(feature = "metrics")]
    let (_timer, _guard, _) = (
        MetricsTimer::start("login_total_seconds"),
        MetricsConcurrencyGuard::start("login_concurrency"),
        counter!("login_attempts").increment(1)
    );

    let user = {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("login_db_query_duration_seconds");
        
        user::Entity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Password)
            .column(user::Column::AvatarFileId)
            .filter(
                Condition::any()
                    .add(user::Column::Username.eq(&req.account))
                    .add(user::Column::Email.eq(&req.account))
            )
            .into_tuple::<(i64, String, Option<String>)>()
            .one(db)
            .await
            .trace_internal_err("db_query_error", "查询用户时数据库错误")?
            .ok_or_warn("user_not_found","登录失败：账号不存在", "账号不存在")?
    };

    {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("login_verify_seconds");
        
        task::spawn_blocking(move || {
            verify(&req.password, &user.1)
        })
            .await
            .trace_internal_err("tokio_error", "线程调度错误")?
            .trace_internal_err("verify_error", "验证密码时发生错误")?
            .ok_or_warn("invalid_password", "密码错误")?
    };

    let new_access_token = rand_utils::generate_random_str(16);
    {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("login_redis_seconds");
        
        redis.set_ex(&RedisKeys::user::user_access_token(user.0), &new_access_token, 2 * 60 * 60)
            .await
            .trace_internal_err("redis_error","向Redis存入access_token失败")?;
    }
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expire = Utc::now() + Duration::days(30);
    let updated_user = {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("login_refresh_token_cost_seconds");
        
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

    let avatar_token = {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("login_crypto_avatar_token_seconds");
        
        user.2.as_ref().and_then(|key| {
            encrypt_image_token(&ImageToken::thumbnail(key.clone()), encryption_key).ok()
        })
    };

    #[cfg(feature = "metrics")]
    counter!("login_success").increment(1);
    
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

#[tracing::instrument(
    name = "auth_register",
    skip_all,
    fields(
        user.username = %req.username,
        user.email = %req.email,
        user.nickname = %req.nickname,
        inviter_code = %req.inviter_code,
        email_code_prefix = %&req.email_verify_code[..2]
    )
)]
pub async fn register(
    db: &DatabaseConnection,
    redis: &Pool,
    req: RegisterRequest,
) -> Result<UserDTO, AppError> {
    #[cfg(feature = "metrics")]
    let (_timer, _guard, _) = (
        MetricsTimer::start("register_total_seconds"),
        MetricsConcurrencyGuard::start("register_concurrency"),
        counter!("register_attempts").increment(1)
    );

    {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("verify_email_code_seconds");
        
        verify_email_verify_code(redis, &req.email, &req.email_verify_code).await?
            .ok_or_warn("invalid_email_code","邮箱验证码错误")?
    };

    let inviter_id = {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("verify_inviter_code_seconds");
        
        verify_inviter_code(redis, &req.inviter_code).await?
    };

    let hashed_pw = {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("hash_password_seconds");
        
        bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)
            .trace_internal_err("hash_error", "密码加密失败")?
    };
    
    let new_user = user::ActiveModel {
        username: Set(req.username),
        email: Set(req.email),
        password: Set(hashed_pw),
        nickname: Set(req.nickname),
        inviter: Set(inviter_id.into()),
        ..Default::default()
    };

    let insert_result = {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("db_insert_seconds");
        
        new_user.insert(db).await
    };

    match insert_result {
        Ok(user_model) => {
            #[cfg(feature = "metrics")]
            counter!("register_success").increment(1);
            
            info!(status = "success", "用户注册成功");
            
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
            if let DbErr::Query(RuntimeErr::SqlxError(ref sqlx_err)) = e {
                if let Some(pg_err) = sqlx_err.as_database_error() {
                    if pg_err.code() == Some("23505".into()) {
                        let detail = pg_err.to_string().to_lowercase();

                        let (reason, msg) = if detail.contains("username") {
                            ("username_existed", "该用户名已被占用")
                        } else if detail.contains("email") {
                            ("email_existed", "该邮箱已被注册")
                        } else {
                            ("row_existed", "记录已存在")
                        };

                        warn!(reason = %reason, status = "failed", "用户注册冲突");
                        return Err(AppError::bad_request(msg));
                    }
                }
            }

            error!(error = ?e, status = "error", "用户注册时发生数据库异常");
            Err(AppError::InternalServerError)
        }
    }
}

#[tracing::instrument(
    name = "auth_send_email_code",
    skip_all,
    fields(
            email = %req.email
    )
)]
pub async fn send_email_code(
    redis: &Pool,
    email_client: &EmailClient,
    req: SendEmailCodeRequest
) -> Result<(), AppError> {
    #[cfg(feature = "metrics")]
    let (_timer, _guard, _) = (
        MetricsTimer::start("send_email_code_total_seconds"),
        MetricsConcurrencyGuard::start("send_email_code_concurrency"),
        counter!("send_email_code_attempts").increment(1)
    );

    let code = rand_utils::generate_random_str(6);
    
    {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("redis_set_seconds");
        
        redis.set_ex(&redis_keys::user::email_verify_code(&req.email), &code, 10 * 60).await
            .trace_internal_err("redis_error", "在发送邮箱验证码时设置redis值错误")?;
    }

    let client = email_client.clone();
    let email = req.email.clone();
    tokio::spawn(async move {
        let start = std::time::Instant::now();
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
            &email,
            "寻忆邮箱验证码",
            html_body
        ).await {
            error!(status="error", "后台发送邮件失败: {:?}", e);
        } else {
            let duration = start.elapsed();
            #[cfg(feature = "metrics")]
            counter!("send_email_code_success").increment(1);
            
            info!(status = "success", email = %email, duration_ms = %duration.as_millis(), "验证码发送成功");
        }
    });

    Ok(())
}

#[tracing::instrument(
    name = "auth_refresh_access_token",
    skip_all
)]
pub async fn refresh_access_token(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    refresh_token: String
) -> Result<AccessTokenResponse, AppError> {
    #[cfg(feature = "metrics")]
    let (_timer, _guard, _) = (
        MetricsTimer::start("refresh_access_token_total_seconds"),
        MetricsConcurrencyGuard::start("refresh_access_token_concurrency"),
        counter!("refresh_access_token_attempts").increment(1)
    );
    
    {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("verify_refresh_token_seconds");
        
        verify_refresh_token(db, user_id, &refresh_token).await?;
    }

    let new_access_token = rand_utils::generate_random_str(16);
    {
        #[cfg(feature = "metrics")]
        let _timer = MetricsTimer::start("set_access_token_seconds");
        
        redis.set_ex(&RedisKeys::user::user_access_token(user_id), &new_access_token, 2 * 60 * 60).await?;
    }

    #[cfg(feature = "metrics")]
    counter!("refresh_token_success").increment(1);
    
    info!(status = "success", user_id = %user_id, "AccessToken刷新成功");

    Ok(AccessTokenResponse {
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + Duration::hours(2)
    })
}

async fn verify_email_verify_code(redis: &Pool, email: &str, code: &str) -> Result<bool, AppError> {
    let stored_code: Option<String> = redis.get_as(&RedisKeys::user::email_verify_code(email))
        .await
        .trace_internal_err("redis_error", "验证邮箱验证码时 获取redis值错误")?;
    Ok(stored_code.map_or(false, |v| v == code))
}

async fn verify_inviter_code(redis: &Pool, inviter_code: &str) -> Result<u32, AppError> {
    redis.get_as(&RedisKeys::user::inviter_code(inviter_code))
        .await
        .trace_internal_err("redis_error", "验证邀请码时 获取redis值错误")?
        .ok_or_warn("invalid_inviter_code","邀请码无效" ,"邀请码无效. 不存在或已过期")
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
        .trace_internal_err("db_error", "刷新access_token时 查询 数据库RefreshToken 失败")?
        .ok_or_warn("user_not_found", "用户不存在", "用户不存在")?;
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
