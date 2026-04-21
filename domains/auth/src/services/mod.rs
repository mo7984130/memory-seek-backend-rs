use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};
use common::utils::HashAlgorithm;
use chrono::{DateTime, Duration, Utc};
use common::constants::{redis_keys, RedisKeys};
use common::error::AppError;
use common::{metrics_group, metrics_success, timed};
use common::utils::{BoolExt, MetricsTimerExt, OptionExt, rand_utils, RedisExt, ResultExt, ToOkExt};
use deadpool_redis::Pool;
use email::EmailClient;
use entities::user;
use entities::user::UserDTO;
use common::models::ImageToken;
use common::utils::TokenCipher;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    QueryFilter, QuerySelect, Set,
};
use sea_orm::{error::DbErr, RuntimeErr};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;
use tracing::{error, info, warn};
use crate::config::{ACCESS_TOKEN_EXPIRE_SECONDS, REFRESH_TOKEN_EXPIRE_DAYS};

/// 登陆
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
    hasher: &HashAlgorithm,
    req: LoginRequest,
    token_cipher: &TokenCipher,
    password_verify_semaphore: &Arc<Semaphore>,
) -> Result<UserDTO, AppError> {
    metrics_group!("login");

    // 获取用户Id, 密码, 头像FileId
    // username 或者 email 等于 account都可以
    #[derive(Debug, FromQueryResult)]
    struct TempUser {
        id: i64,
        password: String,
        avatar_file_id: Option<String>,
    }
    let user = user::Entity::find()
        .select_only()
        .column(user::Column::Id)
        .column(user::Column::Password)
        .column(user::Column::AvatarFileId)
        .filter(
            Condition::any()
                .add(user::Column::Username.eq(&req.account))
                .add(user::Column::Email.eq(&req.account)),
        )
        .into_model::<TempUser>()
        .one(db)
        .timed("auth::login:db_query")
        .await
        .trace_internal_err("db_query_error", "查询用户时数据库错误")?
        .ok_or_warn("user_not_found", "登录失败：账号不存在", "账号或者密码错误")?;

    // 即使账号不存在, 也进行一次dummy密码效验, 保证两者时长差不多
    // TODO
    // let user = match qury_result {
    //     Ok(u) => u,
    //     Err(e) => {
    //         let _ = task::spawn_blocking(|| {
    //             let _ = verify_password("dummy", "$2b$12$QIgiYYcKC7dCwqhEmAX.duD4QA1t5Hgr9HAsmiawNdkXCdxZ8Dvea");
    //         })
    //         .await;
    //         return Err(e);
    //     }
    // };

    // TODO 尝试次数限制
    let old_alg = {
        // 效验密码
        // 使用信号量限制同时效验的数量
        let _permit = password_verify_semaphore
            .acquire()
            .await
            .trace_internal_err("semaphore_error", "获取密码验证信号量失败")?;

        // 在 spawn_blocking 中验证密码，避免阻塞 async runtime
        let password_clone = req.password.clone();
        let stored_hash = user.password.clone();
        let result: Result<(bool, HashAlgorithm), AppError> = task::spawn_blocking(move || HashAlgorithm::verify_and_detect(&password_clone, &stored_hash))
            .await
            .trace_internal_err("spawn_blocking_error", "密码验证任务执行失败")?;
        let verify_result = result.trace_internal_err("verify_password_error", "密码验证内部错误")?;

        if !verify_result.0 {
            return Err(AppError::bad_request("账号或者密码错误"));
        }

        verify_result.1
    };

    // 检查是否需要迁移哈希算法（bcrypt -> argon2id）
    // 登录成功后异步迁移，不影响登录响应时间
    if *hasher != old_alg {
        info!("更新用户密码哈希算法");
        let user_id_clone = user.id;
        let password_for_migration = req.password.clone();
        let db_clone = db.clone();
        let hasher_clone = hasher.clone();
        tokio::spawn(async move {
            let _: Result<(), AppError> = async {
                user::ActiveModel {
                    id: Set(user_id_clone),
                    password: Set(hasher_clone.hash(&password_for_migration)?),
                    ..Default::default()
                }
                .update(&db_clone)
                .await
                .trace_internal_err("db_update_error", "数据库更新密码哈希失败")?;

                Ok(())
            }.await;
        });
    }

    // 更新access_token和refresh_token
    let new_access_token = rand_utils::generate_random_str(16);
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expire = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRE_DAYS);
    let (access_token_result, refresh_token_result) = tokio::join!(
        redis
            .set_ex(
                RedisKeys::user::user_access_token(user.id),
                &new_access_token,
                ACCESS_TOKEN_EXPIRE_SECONDS as u64,
            )
            .timed("auth::login:redis_set"),
        user::ActiveModel {
                id: Set(user.id),
                refresh_token: Set(Some(new_refresh_token.clone())),
                refresh_token_expire_at: Set(Some(new_refresh_token_expire.into())),
                ..Default::default()
            }
            .update(db)
            .timed("auth::login:update_refresh_token")
    );
    access_token_result.trace_internal_err("redis_error", "向Redis存入access_token失败")?;
    let updated_user =
        refresh_token_result.trace_internal_err("db_error", "向数据库更新refresh_token错误")?;

    // 加密头像file_id
    let avatar_token = timed!("auth::login:encrypt_avatar",
        user.avatar_file_id.as_ref().and_then(|key: &String| {
            token_cipher.encrypt(&ImageToken::thumbnail(key.clone()), Some(key)).ok()
        })
    );

    metrics_success!("login");
    info!(status="success", user_id = %user.id, username = %updated_user.username, "用户登录成功");

    // 返回UserDTO
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
        access_token_expire_at: Some(Utc::now() + Duration::seconds(ACCESS_TOKEN_EXPIRE_SECONDS)),
    })
}

/// 注册
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
    hasher: &HashAlgorithm,
    req: RegisterRequest,
) -> Result<UserDTO, AppError> {
    metrics_group!("register");

    // 效验邮箱验证码
    verify_email_verify_code(redis, &req.email, &req.email_verify_code)
        .timed("auth::register:verify_email_code")
        .await?
        .ok_or_warn("invalid_email_code", "邮箱验证码错误")?;

    // 效验邀请码
    let inviter_id = verify_inviter_code(redis, &req.inviter_code)
        .timed("auth::register:verify_inviter_code")
        .await?;

    // 加密密码
    let hashed_pw = timed!("auth::register:hash_password",
        hasher.hash(&req.password)?
    );

    // 插入用户
    let insert_result = user::ActiveModel {
        username: Set(req.username),
        email: Set(req.email),
        password: Set(hashed_pw),
        nickname: Set(req.nickname),
        inviter: Set(inviter_id.into()),
        ..Default::default()
    }
    .insert(db)
    .timed("auth::register:db_insert")
    .await;

    // 结果匹配
    match insert_result {
        Ok(user_model) => {
            metrics_success!("register");

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
            }
            .ok_res()
        }
        Err(e) => {
            // 根据postgres的错误码, 来分辨错误的原因.
            // 不使用先查询后插入, 而是使用这种方式的话
            // 可以节约一次数据库查询和冲突的风险
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

/// 发送邮箱验证码
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
    req: SendEmailCodeRequest,
) -> Result<(), AppError> {
    metrics_group!("send_email_code");

    let code = rand_utils::generate_random_str(6);

    // 设置code到redis中
    redis
        .set_ex(
            &redis_keys::user::email_verify_code(&req.email),
            &code,
            10 * 60,
        )
        .timed("auth::send_email_code:redis_set")
        .await
        .trace_internal_err("redis_error", "在发送邮箱验证码时设置redis值错误")?;

    // 发送邮件
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
        )
        .trim()
        .to_string();

        if let Err(e) = client
            .send_html(
                "no-reply@memory-seek.ink",
                "寻忆",
                &email,
                "寻忆邮箱验证码",
                html_body,
            )
            .await
        {
            error!(status = "error", "后台发送邮件失败: {:?}", e);
        } else {
            let duration = start.elapsed();
            metrics_success!("send_email_code");

            info!(status = "success", email = %email, duration_ms = %duration.as_millis(), "验证码发送成功");
        }
    });

    Ok(())
}

/// 刷新access_token
#[tracing::instrument(name = "auth_refresh_access_token", skip_all, fields(user_id = %user_id))]
pub async fn refresh_access_token(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    refresh_token: String,
) -> Result<AccessTokenResponse, AppError> {
    metrics_group!("refresh_access_token");

    // 效验refresh_token
    verify_refresh_token(db, user_id, &refresh_token)
        .timed("auth::refresh_access_token:verify_token")
        .await?;

    // 设置新的access_token到redis
    let new_access_token = rand_utils::generate_random_str(16);
    redis
        .set_ex(
            &RedisKeys::user::user_access_token(user_id),
            &new_access_token,
            2 * 60 * 60,
        )
        .timed("auth::refresh_access_token:set_token")
        .await?;

    metrics_success!("refresh_token");

    info!(status = "success", "AccessToken刷新成功");

    Ok(AccessTokenResponse {
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + Duration::hours(2),
    })
}

/// 效验邮箱验证吗
async fn verify_email_verify_code(
    redis: &Pool,
    email: &str,
    code: &str,
) -> Result<bool, AppError> {
    let stored_code: Option<String> = redis
        .get_as(&RedisKeys::user::email_verify_code(email))
        .await
        .trace_internal_err("redis_error", "验证邮箱验证码时 获取redis值错误")?;
    Ok(stored_code.map_or(false, |v| v == code))
}

/// 效验邀请码
async fn verify_inviter_code(redis: &Pool, inviter_code: &str) -> Result<u32, AppError> {
    redis
        .get_as(&RedisKeys::user::inviter_code(inviter_code))
        .await
        .trace_internal_err("redis_error", "验证邀请码时 获取redis值错误")?
        .ok_or_warn(
            "invalid_inviter_code",
            "邀请码无效",
            "邀请码无效. 不存在或已过期",
        )
}

/// 效验refresh_token
#[derive(FromQueryResult)]
struct RefreshTokenValidation {
    refresh_token: Option<String>,
    refresh_token_expire_at: Option<DateTime<Utc>>,
}
async fn verify_refresh_token(
    db: &DatabaseConnection,
    user_id: i64,
    refresh_token: &str,
) -> Result<(), AppError> {
    // 从数据库中获取RefreshToken 和 RefreshTokenExpireAt
    let res = user::Entity::find()
        .select_only()
        .column(user::Column::RefreshToken)
        .column(user::Column::RefreshTokenExpireAt)
        .filter(user::Column::Id.eq(user_id))
        .into_model::<RefreshTokenValidation>()
        .one(db)
        .await
        .trace_internal_err(
            "db_error",
            "刷新access_token时 查询 数据库RefreshToken 失败",
        )?
        .ok_or_warn("user_not_found", "用户不存在", "用户不存在")?;
    if res.refresh_token.as_deref() != Some(refresh_token) {
        warn!("refresh_token不匹配");
        return Err(AppError::Unauthorized);
    }
    if let Some(expire_at) = res.refresh_token_expire_at {
        if Utc::now() > expire_at {
            warn!("refresh_token已过期");
            return Err(AppError::Unauthorized);
        }
    } else {
        error!("refresh_token过期时间不存在");
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
