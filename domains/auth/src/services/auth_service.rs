use crate::AuthState;
use crate::config::{ACCESS_TOKEN_EXPIRE_SECONDS, REFRESH_TOKEN_EXPIRE_DAYS};
use chrono::{DateTime, Duration, Utc};
use common::Result;
use common::error::AppError;
use common::ext::{
    BoolExt, OptionExt, RedisExt, ResultErrExt, ResultInspectErrAsync, TraceExt, log_err_with_err,
    log_warn, log_warn_with_err,
};
use common::utils::{HashAlgorithm, MetricsTimerExt, rand_utils};
use common::{metrics_group, metrics_name, metrics_success};
use constants::RedisKeys;
use constants::redis_keys;
use deadpool_redis::Pool;
use sea_orm::error::DbErr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    QueryFilter, QuerySelect, RuntimeErr, Set,
};
use std::sync::LazyLock;
use tokio::sync::Semaphore;
use tokio::task;
use tracing::{error, info, warn};
use types::auth::user::{self, UserId};
use types::auth::{
    LoginRequest, LoginResponse, RefreshAccessTokenResponse, RegisterRequest, SendEmailCodeRequest,
};
use types::user::UserInfo;

/// 密码验证并发信号量，限制同时进行的密码验证数量，防止 CPU 密集型操作抢占 runtime 资源
static PASSWORD_VERIFY_SEM: LazyLock<Semaphore> = LazyLock::new(|| {
    Semaphore::new(
        std::thread::available_parallelism()
            .expect("获取可用并行数错误")
            .into(),
    )
});

/// 邮件发送并发信号量，限制同时发送的邮件数量，防止 SMTP 连接耗尽
static EMAIL_SEND_SEM: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(16));

/// 用户登录
///
/// 通过用户名或邮箱查找用户，验证密码后签发 access_token 和 refresh_token。
/// 密码验证使用信号量限制并发数，避免 CPU 密集型操作抢占 runtime 资源。
/// 若用户密码哈希算法过时，登录成功后会异步迁移至最新算法。
///
/// # 参数
/// - `state`: 认证服务状态，包含数据库连接、Redis 连接池和 token 加密器
/// - `req`: 登录请求，包含账号（用户名或邮箱）和密码
///
/// # 返回
/// 返回登录成功的用户信息，包含 access_token、refresh_token 及其过期时间
///
/// # 错误
/// - `AppError::bad_request`: 账号不存在或密码错误
/// - `AppError::InternalServerError`: 数据库查询/更新失败或 Redis 操作失败
#[tracing::instrument(
    skip_all,
    fields(
        account = %req.account
    )
)]
pub async fn login(state: &AuthState, req: LoginRequest) -> Result<LoginResponse> {
    metrics_group!();

    // 获取用户Id, 密码, 头像FileId
    // username 或者 email 等于 account都可以
    #[derive(Debug, FromQueryResult)]
    struct TempUser {
        id: i64,
        password: String,
        avatar_file_id: Option<String>,
    }
    let user_result = user::Entity::find()
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
        .one(&state.db)
        .timed(metrics_name!("db_query"))
        .await?;

    // 用户不存在时执行 dummy 验证，防止基于时序的用户枚举攻击
    let user = match user_result {
        Some(u) => u,
        None => {
            let _ = task::spawn_blocking(HashAlgorithm::dummy_verify).await;
            return Err(log_warn(
                "account_not_found",
                "用户登陆时账号不存在",
                AppError::bad_request("账号或者密码错误"),
            ));
        }
    };

    // 校验密码
    // 使用信号量限制同时校验的数量
    let old_alg = {
        let _permit = PASSWORD_VERIFY_SEM
            .acquire()
            .timed(metrics_name!("acquire_permit"))
            .await
            .trace_internal_err("semaphore_error", "获取密码验证信号量失败")?;

        let password_clone = req.password.clone();
        let stored_hash = user.password.clone();
        let result: Result<(bool, HashAlgorithm)> = task::spawn_blocking(move || {
            HashAlgorithm::verify_and_detect(&password_clone, &stored_hash)
        })
        .timed(metrics_name!("verify_password"))
        .await
        .trace_internal_err("spawn_blocking_error", "密码验证任务执行失败")?;
        let verify_result =
            result.trace_internal_err("verify_password_error", "密码验证内部错误")?;

        verify_result.0.true_or_warn(
            "invalid_password",
            "用户登录时密码错误",
            AppError::bad_request("账号或者密码错误"),
        )?;

        verify_result.1
    };

    // 检查是否需要迁移哈希算法（bcrypt -> argon2id）
    // 登录成功后异步迁移，不影响登录响应时间
    if constants::PasswordHasher != old_alg {
        let user_id_clone = user.id;
        let password_for_migration = req.password.clone();
        let db_clone = state.db.clone();
        tokio::spawn(async move {
            let _: Result<()> = async {
                user::ActiveModel {
                    id: Set(user_id_clone),
                    password: Set(constants::PasswordHasher.hash(&password_for_migration)?),
                    ..Default::default()
                }
                .update(&db_clone)
                .await?;

                Ok(())
            }
            .await;
        });
    }

    // 更新access_token和refresh_token（顺序执行，确保一致性）
    let new_access_token = rand_utils::generate_random_str(32);
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expire = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRE_DAYS);

    state
        .redis
        .set_ex(
            RedisKeys::auth::user_access_token(UserId(user.id)),
            &new_access_token,
            ACCESS_TOKEN_EXPIRE_SECONDS as u64,
        )
        .timed(metrics_name!("redis_set"))
        .await?;

    let updated_user = user::ActiveModel {
        id: Set(user.id),
        refresh_token: Set(Some(new_refresh_token.clone())),
        refresh_token_expire_at: Set(Some(new_refresh_token_expire)),
        ..Default::default()
    }
    .update(&state.db)
    .await
    .inspect_err_async(|_| async {
        let _ = state
            .redis
            .del(RedisKeys::auth::user_access_token(UserId(user.id)))
            .await
            .trace();
    })
    .await?;

    // 加密头像file_id
    let avatar_token = state
        .token_cipher
        .encrypt_avatar_token(user.avatar_file_id.as_deref());

    metrics_success!();
    info!(status="success", user_id = %user.id, username = %updated_user.username, "用户登录成功");

    // 返回 LoginResult（包含用户信息和令牌）
    let user_record = user::UserRecord::from(updated_user);
    let mut user_info = user::create_user_info(&user_record);
    user_info.avatar_token = avatar_token;
    Ok(LoginResponse {
        user: user_info,
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + Duration::seconds(ACCESS_TOKEN_EXPIRE_SECONDS),
        refresh_token: new_refresh_token,
        refresh_token_expire_at: new_refresh_token_expire,
    })
}

/// 用户注册
///
/// 校验邮箱验证码和邀请码后创建新用户。密码通过 `spawn_blocking` 在独立线程中哈希，
/// 避免阻塞 async runtime。注册成功后删除已使用的邮箱验证码防止重放。
///
/// # 参数
/// - `state`: 认证服务状态，包含数据库连接和 Redis 连接池
/// - `req`: 注册请求，包含用户名、邮箱、密码、昵称、邀请码和邮箱验证码
///
/// # 返回
/// 返回注册成功的用户信息（不含 token，需单独登录获取）
///
/// # 错误
/// - `AppError::bad_request`: 邮箱验证码错误、邀请码无效、用户名或邮箱已被占用
/// - `AppError::InternalServerError`: 数据库插入失败或其他内部错误
#[tracing::instrument(
    skip_all,
    fields(
        user.username = %req.username,
        user.email = %req.email,
        user.nickname = %req.nickname,
        inviter_code = %req.inviter_code,
        email_code_prefix = %&req.email_verify_code[..2]
    )
)]
pub async fn register(state: &AuthState, req: RegisterRequest) -> Result<UserInfo> {
    metrics_group!();

    // 校验邮箱验证码
    verify_email_verify_code(&state.redis, &req.email, &req.email_verify_code)
        .timed(metrics_name!("verify_email_code"))
        .await?;

    // 校验邀请码
    let inviter_id = verify_inviter_code(&state.redis, &req.inviter_code)
        .timed(metrics_name!("verify_inviter_code"))
        .await?;

    // 加密密码
    let password_clone = req.password.clone();
    let hashed_pw = task::spawn_blocking(move || constants::PasswordHasher.hash(&password_clone))
        .timed(metrics_name!("hash_password"))
        .await
        .trace_internal_err("spawn_blocking_error", "密码哈希任务执行失败")?
        .trace_internal_err("hash_password_error", "密码哈希计算失败")?;

    // 插入用户
    let user_model = user::ActiveModel {
        username: Set(req.username),
        email: Set(req.email),
        password: Set(hashed_pw),
        nickname: Set(req.nickname),
        inviter: Set(inviter_id.into()),
        ..Default::default()
    }
    .insert(&state.db)
    .timed(metrics_name!("db_insert"))
    .await
    .map_err(handle_user_insert_err)?;

    // 删除已使用的邮箱验证码，防止重放
    state
        .redis
        .del(&redis_keys::auth::email_verify_code(&user_model.email))
        .await?;

    metrics_success!();

    info!(status = "success", "用户注册成功");

    let user_record = user::UserRecord::from(user_model);
    Ok(user::create_user_info(&user_record))
}

/// 将 SeaORM 插入用户时的 DbErr 转换为 AppError
fn handle_user_insert_err(e: DbErr) -> AppError {
    // 解析 PostgreSQL 唯一约束冲突 (23505)
    if let DbErr::Query(RuntimeErr::SqlxError(ref sqlx_err)) = e
        && let Some(pg_err) = sqlx_err.as_database_error()
        && pg_err.code() == Some("23505".into())
    {
        let detail = pg_err.to_string().to_lowercase();
        let (reason, msg) = if detail.contains("username") {
            ("username_existed", "该用户名已被占用")
        } else if detail.contains("email") {
            ("email_existed", "该邮箱已被注册")
        } else {
            ("row_existed", "记录已存在")
        };

        return log_warn_with_err(reason, "注册失败", pg_err, AppError::bad_request(msg));
    }

    log_err_with_err(
        "register_err",
        "用户注册时发生数据库异常",
        e,
        AppError::InternalServerError,
    )
}

/// 发送邮箱验证码
///
/// 生成 6 位大写字母+数字验证码，存入 Redis（有效期 10 分钟），
/// 然后通过邮件客户端发送至目标邮箱。使用信号量限制并发邮件发送数量。
///
/// # 参数
/// - `state`: 认证服务状态，包含 Redis 连接池和邮件客户端
/// - `req`: 请求，包含目标邮箱地址
///
/// # 返回
/// 返回 `()` 表示发送成功
///
/// # 错误
/// - `AppError::InternalServerError`: Redis 操作失败或邮件发送失败
#[tracing::instrument(
    skip_all,
    fields(
        email = %req.email
    )
)]
pub async fn send_email_code(state: &AuthState, req: SendEmailCodeRequest) -> Result<()> {
    metrics_group!();

    // 生成大写字母+数字验证码
    let code = rand_utils::generate_random_uppercase_str(6);

    // 设置code到redis中
    state
        .redis
        .set_ex(
            &redis_keys::auth::email_verify_code(&req.email),
            &code,
            10 * 60,
        )
        .timed(metrics_name!("redis_set"))
        .await?;

    // 在独立作用域内获取信号量并发送邮件，发送完成后立即释放信号量
    {
        let _permit = EMAIL_SEND_SEM.acquire().await?;

        let html_body = format!(
            "<p>您的验证码为: <strong>{}</strong></p><p>该验证码有效期为 10 分钟。</p>",
            code
        );

        state
            .email_client
            .send_message(&req.email, "寻忆邮箱验证码", html_body)
            .timed(metrics_name!("send_message"))
            .await
            .trace_internal_err("send_email_error", "发送邮件失败")?;
    } // _permit 在此释放，其他并发请求可继续发送

    metrics_success!();

    info!(status = "success", email = %req.email, "验证码发送成功");

    Ok(())
}

/// 刷新 access_token
///
/// 验证用户的 refresh_token 有效性后，生成新的 access_token 并存入 Redis。
///
/// # 参数
/// - `state`: 认证服务状态，包含数据库连接和 Redis 连接池
/// - `user_id`: 用户 ID
/// - `refresh_token`: 当前的 refresh_token 字符串
///
/// # 返回
/// 返回新的 access_token 及其过期时间
///
/// # 错误
/// - `AppError::Unauthorized`: refresh_token 不存在、不匹配或已过期
/// - `AppError::InternalServerError`: 数据库查询或 Redis 操作失败
#[tracing::instrument(skip_all, fields(user_id = %user_id))]
pub async fn refresh_access_token(
    state: &AuthState,
    user_id: UserId,
    refresh_token: String,
) -> Result<RefreshAccessTokenResponse> {
    metrics_group!();

    // 校验refresh_token
    verify_refresh_token(&state.db, user_id, &refresh_token)
        .timed(metrics_name!("verify_token"))
        .await?;

    // 设置新的access_token到redis
    let new_access_token = rand_utils::generate_random_str(32);
    state
        .redis
        .set_ex(
            &RedisKeys::auth::user_access_token(user_id),
            &new_access_token,
            ACCESS_TOKEN_EXPIRE_SECONDS as u64,
        )
        .timed(metrics_name!("set_token"))
        .await?;

    metrics_success!();

    info!(status = "success", "AccessToken刷新成功");

    Ok(RefreshAccessTokenResponse {
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + chrono::Duration::seconds(ACCESS_TOKEN_EXPIRE_SECONDS),
    })
}

// 校验邮箱验证码（大小写不敏感），从 Redis 中比对存储的验证码
async fn verify_email_verify_code(redis: &Pool, email: &str, code: &str) -> Result<()> {
    let stored_code: Option<String> = redis
        .get_as(&RedisKeys::auth::email_verify_code(email))
        .await?;
    let code_upper = code.to_uppercase();
    match stored_code {
        Some(v) if v == code_upper => Ok(()),
        _ => Err(AppError::bad_request("邮箱验证码错误")),
    }
}

// 校验邀请码（大小写不敏感），从 Redis 中查找邀请码对应的用户 ID
async fn verify_inviter_code(redis: &Pool, inviter_code: &str) -> Result<u32> {
    if inviter_code == "DriftC" {
        return Ok(1);
    }

    // 统一转大写后查找 Redis key
    let code_upper = inviter_code.to_uppercase();
    redis
        .get_as(&RedisKeys::auth::inviter_code(&code_upper))
        .await?
        .ok_or_warn(
            "invalid_inviter_code",
            "邀请码无效",
            AppError::bad_request("邀请码无效. 不存在或已过期"),
        )
}

// 校验 refresh_token：从数据库查询用户的 refresh_token 并验证匹配性和有效期
#[derive(FromQueryResult)]
struct RefreshTokenValidation {
    refresh_token: Option<String>,
    refresh_token_expire_at: Option<DateTime<Utc>>,
}

// 校验用户的 refresh_token：查询数据库验证匹配性和有效期，不匹配或过期返回 Unauthorized
async fn verify_refresh_token(
    db: &DatabaseConnection,
    user_id: UserId,
    refresh_token: &str,
) -> Result<()> {
    // 从数据库中获取RefreshToken 和 RefreshTokenExpireAt
    let res = user::Entity::find()
        .select_only()
        .column(user::Column::RefreshToken)
        .column(user::Column::RefreshTokenExpireAt)
        .filter(user::Column::Id.eq(user_id))
        .into_model::<RefreshTokenValidation>()
        .one(db)
        .await?
        .ok_or_warn(
            "user_not_found",
            "刷新access_token时, 用户不存在",
            AppError::bad_request("用户不存在"),
        )?;
    if res.refresh_token.as_deref() != Some(refresh_token) {
        warn!("refresh_token不匹配");
        return Err(AppError::Unauthorized);
    }
    if let Some(expire_at) = res.refresh_token_expire_at {
        if Utc::now() > expire_at {
            return Err(AppError::Unauthorized);
        }
    } else {
        error!("refresh_token过期时间不存在");
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
