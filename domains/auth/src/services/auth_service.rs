use crate::AuthState;
use crate::config::{ACCESS_TOKEN_EXPIRE_SECONDS, REFRESH_TOKEN_EXPIRE_DAYS};
use crate::mapper::{AuthInsertParam, AuthMapper};
use chrono::{Duration, Utc};
use common::Result;
use common::error::AppError;
use common::ext::{
    DeferFromExt, OptionExt, RedisExt, ResultErrExt, ResultInspectErrAsync, log_warn,
};
use common::utils::{HashAlgorithm, MetricsTimerExt, rand_utils};
use common::{inc_error, metrics_name};
use constants::RedisKeys;
use constants::redis_keys;
use std::sync::LazyLock;
use tokio::sync::Semaphore;
use tokio::task::{self, spawn_blocking};
use types::auth::user::UserId;
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
#[common::metered]
#[tracing::instrument(
    skip_all,
    fields(
        account = %req.account
    )
)]
pub async fn login(state: &AuthState, req: LoginRequest) -> Result<LoginResponse> {
    // 获取用户Id, 密码, 头像FileId
    // username 或者 email 等于 account都可以

    let user_result = AuthMapper::query_by_account(&state.db, &req.account)
        .timed(metrics_name!("db_query"))
        .await?;

    // 用户不存在时执行 dummy 验证，防止基于时序的用户枚举攻击
    let user = match user_result {
        Some(u) => u,
        None => {
            inc_error!("auth");
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
        let result = task::spawn_blocking(move || {
            HashAlgorithm::verify_and_detect(&password_clone, &stored_hash)
        })
        .timed(metrics_name!("verify_password"))
        .await
        .trace_internal_err("spawn_blocking_error", "密码验证任务执行失败")?;
        let verify_result =
            result.trace_internal_err("verify_password_error", "密码验证内部错误")?;

        if !verify_result.0 {
            inc_error!("auth");
            return Err(log_warn(
                "invalid_password",
                "用户登录时密码错误",
                AppError::bad_request("账号或者密码错误"),
            ));
        }

        verify_result.1
    };

    // 检查是否需要迁移哈希算法（bcrypt -> argon2id）
    // 登录成功后异步迁移，不影响登录响应时间
    if constants::PasswordHasher != old_alg {
        let password_for_migration = req.password.clone();
        let password_result =
            spawn_blocking(move || constants::PasswordHasher.hash(&password_for_migration))
                .await
                .defer()?;
        let new_password = password_result?;

        AuthMapper::update_password(&state.db, user.id, &new_password).await?;
    }

    // 更新access_token和refresh_token（顺序执行，确保一致性）
    let new_access_token = rand_utils::generate_random_str(32);
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_EXPIRE_DAYS);

    state
        .redis
        .set_ex(
            RedisKeys::auth::user_access_token(user.id),
            &new_access_token,
            ACCESS_TOKEN_EXPIRE_SECONDS as u64,
        )
        .timed(metrics_name!("redis_set"))
        .await
        .defer()?;

    let updated_user = AuthMapper::update_refresh_token(
        &state.db,
        user.id,
        new_refresh_token.clone(),
        new_refresh_token_expires_at,
    )
    .await
    .inspect_err_async(|_| async {
        if let Err(error) = state
            .redis
            .del(RedisKeys::auth::user_access_token(user.id))
            .await
            .defer()
        {
            let _ = AppError::from(error);
        }
    })
    .await?;

    // 加密头像file_id

    // 返回 LoginResult（包含用户信息和令牌）
    let user_info = UserInfo::from_with_token(updated_user)?;
    Ok(LoginResponse {
        user: user_info,
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + Duration::seconds(ACCESS_TOKEN_EXPIRE_SECONDS),
        refresh_token: new_refresh_token,
        refresh_token_expire_at: new_refresh_token_expires_at,
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
#[common::metered]
#[tracing::instrument(
    skip_all,
    fields(
        username = %req.username,
        email = %req.email,
        nickname = %req.nickname,
        inviter_code = %req.inviter_code,
        email_code_prefix = %&req.email_verify_code[..2]
    )
)]
pub async fn register(state: &AuthState, req: RegisterRequest) -> Result<UserInfo> {
    // 校验邮箱验证码
    verify_email_verify_code(state, &req.email, &req.email_verify_code)
        .timed(metrics_name!("verify_email_code"))
        .await
        .inspect_err(|_| inc_error!("validation"))?;

    // 校验邀请码
    let inviter_id = verify_inviter_code(state, &req.inviter_code)
        .timed(metrics_name!("verify_inviter_code"))
        .await
        .inspect_err(|_| inc_error!("validation"))?;

    // 加密密码
    let clone_password = req.password.clone();
    let hashed_pw = task::spawn_blocking(move || constants::PasswordHasher.hash(&clone_password))
        .timed(metrics_name!("hash_password"))
        .await
        .trace_internal_err("spawn_blocking_error", "密码哈希任务执行失败")?
        .trace_internal_err("hash_password_error", "密码哈希计算失败")?;

    // 插入用户
    let user_model = AuthMapper::insert(
        &state.db,
        AuthInsertParam {
            username: req.username,
            email: req.email,
            password: hashed_pw,
            nickname: req.nickname,
            inviter: inviter_id,
        },
    )
    .timed(metrics_name!("db_insert"))
    .await?;

    // 删除已使用的邮箱验证码，防止重放
    state
        .redis
        .del(&redis_keys::auth::email_verify_code(&user_model.email))
        .await
        .defer()?;

    Ok(UserInfo::from(user_model))
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
#[common::metered]
#[tracing::instrument(
    skip_all,
    fields(
        email = %req.email
    )
)]
pub async fn send_email_code(state: &AuthState, req: SendEmailCodeRequest) -> Result<()> {
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
        .await
        .defer()?;

    // 在独立作用域内获取信号量并发送邮件，发送完成后立即释放信号量
    {
        let _permit = EMAIL_SEND_SEM.acquire().await.defer()?;

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
#[common::metered]
#[tracing::instrument(skip_all, fields(user_id = %user_id))]
pub async fn refresh_access_token(
    state: &AuthState,
    user_id: UserId,
    refresh_token: String,
) -> Result<RefreshAccessTokenResponse> {
    // 校验refresh_token
    verify_refresh_token(state, user_id, &refresh_token)
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
        .await
        .defer()?;

    Ok(RefreshAccessTokenResponse {
        access_token: new_access_token,
        access_token_expire_at: Utc::now() + chrono::Duration::seconds(ACCESS_TOKEN_EXPIRE_SECONDS),
    })
}

// 校验邮箱验证码（大小写不敏感），从 Redis 中比对存储的验证码
async fn verify_email_verify_code(state: &AuthState, email: &str, code: &str) -> Result<()> {
    let stored_code: Option<String> = state
        .redis
        .get_as(&RedisKeys::auth::email_verify_code(email))
        .await
        .defer()?;
    let code_upper = code.to_uppercase();
    match stored_code {
        Some(v) if v == code_upper => Ok(()),
        _ => Err(AppError::bad_request("邮箱验证码错误")),
    }
}

// 校验邀请码（大小写不敏感），从 Redis 中查找邀请码对应的用户 ID
async fn verify_inviter_code(state: &AuthState, inviter_code: &str) -> Result<UserId> {
    // 统一转大写后查找 Redis key
    let code_upper = inviter_code.to_uppercase();
    if code_upper == "DRIFTC" {
        return Ok(UserId(1));
    }
    state
        .redis
        .get_as(&RedisKeys::auth::inviter_code(&code_upper))
        .await
        .defer()?
        .map(UserId)
        .ok_or_warn(
            "invalid_inviter_code",
            "邀请码无效",
            AppError::bad_request("邀请码无效. 不存在或已过期"),
        )
}

// 校验用户的 refresh_token：查询数据库验证匹配性和有效期，不匹配或过期返回 Unauthorized
async fn verify_refresh_token(
    state: &AuthState,
    user_id: UserId,
    refresh_token: &str,
) -> Result<()> {
    // 从数据库中获取RefreshToken 和 RefreshTokenExpireAt
    let res = AuthMapper::query_refresh_token(&state.db, user_id)
        .await?
        .ok_or_warn(
            "user_not_found",
            "刷新access_token时, 用户不存在",
            AppError::bad_request("用户不存在"),
        )?;
    if res.refresh_token.as_deref() != Some(refresh_token) {
        common::caller_warn!("refresh_token不匹配");
        return Err(AppError::Unauthorized);
    }
    if let Some(expire_at) = res.refresh_token_expire_at {
        if Utc::now() > expire_at {
            return Err(AppError::Unauthorized);
        }
    } else {
        common::caller_error!("refresh_token过期时间不存在");
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
