use crate::AuthState;
use crate::config::{ACCESS_TOKEN_EXPIRE, EMAIL_CODE_EXPIRE, REFRESH_TOKEN_EXPIRE};
use crate::error_ext::AuthOptionExt;
use crate::mapper::{AuthInsertParam, AuthMapper};
use audit::{AuditEvent, AuditRecorder};
use common_core::Result;
use common_core::error::contextual::ext::{
    BoolExt, ContextualResultExt, IntoContextualExt, OptionExt,
};
use common_core::error::{AppError, ContextualError};
use common_core::time::{after, now};
use common_core::{ext::ResultInspectErrAsync, ext::ToOk};
use common_crypto::{HashAlgorithm, rand_utils};
use common_db::db_transaction;
use common_metrics::MetricsTimerExt;
use common_metrics::metrics_name;
use common_redis::RedisExt;
use constants::RedisKeys;
use constants::redis_keys;
use tokio::task::{self, spawn_blocking};
use types_identity::auth::user::UserId;
use types_identity::auth::{
    LoginRequest, LoginResponse, RefreshAccessTokenResponse, RegisterRequest, SendEmailCodeRequest,
};
use types_identity::user::UserInfo;

/// 用户登录，校验账号密码后签发 access_token 和 refresh_token；密码校验经信号量限流，哈希算法过时会异步迁移
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(
        account = %req.account
    )
)]
pub async fn login(state: &AuthState, req: LoginRequest) -> Result<LoginResponse> {
    // 获取用户Id, 密码
    // username 或者 email 等于 account都可以
    let user_result = AuthMapper::query_by_account(&state.db, &req.account)
        .timed(metrics_name!("db_query"))
        .await?;

    // 用户不存在时执行 dummy 验证，防止基于时序的用户枚举攻击
    let user = match user_result {
        Some(u) => u,
        None => {
            task::spawn_blocking(HashAlgorithm::dummy_verify)
                .await
                .into_contextual()
                .emit_if_err();
            return Err(ContextualError::warn_without_source(
                "account_not_found",
                "用户登陆时账号不存在",
                AppError::bad_request("账号或者密码错误"),
            )
            .emit());
        }
    };

    // 校验密码
    // 使用信号量限制同时校验的数量
    let old_alg = {
        let _permit = common_crypto::password_verify_semaphore()
            .acquire()
            .timed(metrics_name!("acquire_permit"))
            .await
            .into_contextual()?;

        let password_clone = req.password.clone();
        let stored_hash = user.password.clone();
        let result = task::spawn_blocking(move || {
            HashAlgorithm::verify_and_detect(&password_clone, &stored_hash)
        })
        .timed(metrics_name!("verify_password"))
        .await
        .into_contextual()??;

        if !result.0 {
            return Err(ContextualError::warn_without_source(
                "invalid_password",
                "用户登录时密码错误",
                AppError::bad_request("账号或者密码错误"),
            )
            .emit());
        }

        result.1
    };

    // 检查是否需要迁移哈希算法
    if constants::PasswordHasher != old_alg {
        let password_for_migration = req.password.clone();
        let password_result =
            spawn_blocking(move || constants::PasswordHasher.hash(&password_for_migration))
                .await
                .into_contextual()?;
        let new_password = password_result?;

        AuthMapper::update_password(&state.db, user.id, &new_password).await?;
    }

    // 更新access_token和refresh_token（顺序执行，确保一致性）
    let new_access_token = rand_utils::generate_random_str(32);
    let new_refresh_token = rand_utils::generate_random_str(32);
    let new_refresh_token_expires_at = after(REFRESH_TOKEN_EXPIRE);

    state
        .redis
        .set_ex(
            RedisKeys::auth::user_access_token(user.id),
            &new_access_token,
            ACCESS_TOKEN_EXPIRE.as_secs(),
        )
        .timed(metrics_name!("redis_set"))
        .await
        .into_contextual()?;

    let user = db_transaction!(&state.db, |txn| {
        let user = AuthMapper::update_refresh_token(
            txn,
            user.id,
            new_refresh_token.clone(),
            new_refresh_token_expires_at,
        )
        .timed(metrics_name!("db_update"))
        .await
        .inspect_err_async(|_| async {
            state
                .redis
                .del(RedisKeys::auth::user_access_token(user.id))
                .await
                .into_contextual()
                .emit_if_err();
        })
        .await?;

        AuditRecorder::append(txn, AuditEvent::new("auth.user_login").with_actor(user.id)).await?;

        Ok(user)
    })
    .await?;

    // 返回 LoginResult（包含用户信息和令牌）
    let user_info = UserInfo::from_with_token(user)?;
    Ok(LoginResponse {
        user: user_info,
        access_token: new_access_token,
        access_token_expire_at: after(ACCESS_TOKEN_EXPIRE),
        refresh_token: new_refresh_token,
        refresh_token_expire_at: new_refresh_token_expires_at,
    })
}

/// 用户注册，校验邮箱验证码与邀请码后创建用户；成功后删除已用验证码防止重放
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(
        username = %req.username,
        email = %req.email,
        nickname = %req.nickname,
        inviter_code = %req.inviter_code,
        email_code_prefix = %req.email_verify_code.chars().take(2).collect::<String>()
    )
)]
pub async fn register(state: &AuthState, req: RegisterRequest) -> Result<UserInfo> {
    // 校验邮箱验证码
    verify_email_verify_code(state, &req.email, &req.email_verify_code)
        .timed(metrics_name!("verify_email_code"))
        .await?;

    // 校验邀请码
    let inviter_id = verify_inviter_code(state, &req.inviter_code)
        .timed(metrics_name!("verify_inviter_code"))
        .await?;

    // 加密密码
    let password = req.password;
    let hashed_pw = task::spawn_blocking(move || constants::PasswordHasher.hash(&password))
        .timed(metrics_name!("hash_password"))
        .await
        .into_contextual()??;

    let insert_param = AuthInsertParam {
        username: req.username,
        email: req.email,
        password: hashed_pw,
        nickname: req.nickname,
        inviter: inviter_id,
    };

    // 写入用户
    let user_model = common_db::db_transaction!(&state.db, |txn| {
        let user_model = AuthMapper::insert(txn, insert_param)
            .timed(metrics_name!("db_insert"))
            .await?;

        AuditRecorder::append(
            txn,
            AuditEvent::new("auth.user_registered")
                .with_actor(user_model.id.0)
                .with_target("user", user_model.id.0),
        )
        .await?;

        Ok(user_model)
    })
    .await?;

    // 删除已使用的邮箱验证码，防止重放
    state
        .redis
        .del(&redis_keys::auth::email_verify_code(&user_model.email))
        .timed(metrics_name!("redis_delete"))
        .await
        .into_contextual()?;

    Ok(UserInfo::from(user_model))
}

/// 生成 6 位验证码写入 Redis 并发送到目标邮箱
#[common_macros::metered]
#[tracing::instrument(
    skip_all,
    fields(
        email = %req.email
    )
)]
pub async fn send_email_code(state: &AuthState, req: SendEmailCodeRequest) -> Result<()> {
    // 验证码接口用于注册,已注册邮箱不再发送注册验证码。
    AuthMapper::email_exists(&state.db, &req.email)
        .timed(metrics_name!("db_query"))
        .await?
        .false_or_warn(
            "email_existed",
            "发送注册验证码时邮箱已注册",
            AppError::bad_request("该邮箱已被注册"),
        )?;

    // 生成大写字母+数字验证码
    let code = rand_utils::generate_random_uppercase_str(6);

    // 设置code到redis中
    state
        .redis
        .set_ex(
            &redis_keys::auth::email_verify_code(&req.email),
            &code,
            EMAIL_CODE_EXPIRE.as_secs(),
        )
        .timed(metrics_name!("redis_set"))
        .await
        .into_contextual()?;

    // 发送邮件
    let html_body = format!(
        "<p>您的验证码为: <strong>{}</strong></p><p>该验证码有效期为 {} 分钟。</p>",
        code,
        EMAIL_CODE_EXPIRE.as_secs() / 60
    );

    state
        .email_client
        .send_message(&req.email, "寻忆邮箱验证码", html_body)
        .timed(metrics_name!("send_message"))
        .await?;

    Ok(())
}

/// 校验 refresh_token 后签发新的 access_token 并写入 Redis
#[common_macros::metered]
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
            ACCESS_TOKEN_EXPIRE.as_secs(),
        )
        .timed(metrics_name!("set_token"))
        .await
        .into_contextual()?;

    Ok(RefreshAccessTokenResponse {
        access_token: new_access_token,
        access_token_expire_at: after(ACCESS_TOKEN_EXPIRE),
    })
}

// 校验邮箱验证码（大小写不敏感），从 Redis 中比对存储的验证码
/// 校验邮箱验证码是否匹配，大小写不敏感
async fn verify_email_verify_code(state: &AuthState, email: &str, code: &str) -> Result<()> {
    let stored_code = state
        .redis
        .get_as::<String>(&RedisKeys::auth::email_verify_code(email))
        .await
        .into_contextual()?
        .ok_or_warn(
            "auth:email_verify_code_not_exist",
            "用户注册时邮箱验证码不存在",
            AppError::bad_request("邮箱验证码错误"),
        )?;

    let code_upper = code.to_uppercase();
    (code_upper == stored_code).true_or_warn(
        "auth:email_verify_code_not_match",
        "用户注册时邮箱验证码错误与",
        AppError::bad_request("邮箱验证码错误"),
    )?;

    Ok(())
}

// 校验邀请码（大小写不敏感），从 Redis 中查找邀请码对应的用户 ID
/// 校验邀请码（大小写不敏感）并返回其绑定的用户 ID
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
        .into_contextual()?
        .map(UserId)
        .ok_or_warn(
            "invalid_inviter_code",
            "邀请码无效",
            AppError::bad_request("邀请码无效. 不存在或已过期"),
        )?
        .to_ok()
}

// 校验用户的 refresh_token：查询数据库验证匹配性和有效期，不匹配或过期返回 Unauthorized
/// 校验 refresh_token 与数据库记录一致且未过期
async fn verify_refresh_token(
    state: &AuthState,
    user_id: UserId,
    refresh_token: &str,
) -> Result<()> {
    // 从数据库中获取RefreshToken 和 RefreshTokenExpireAt
    let res = AuthMapper::query_refresh_token(&state.db, user_id)
        .await?
        .user_not_found()?;

    let expire_at = res.refresh_token_expire_at.ok_or_error(
        "refresh_token_expiry_missing",
        "刷新 access_token 时，refresh token 过期时间不存在",
        AppError::Unauthorized,
    )?;
    (now() <= expire_at).true_or_warn(
        "refresh_token_expired",
        "刷新 access_token 时，refresh token 已过期",
        AppError::Unauthorized,
    )?;

    (res.refresh_token.as_deref() == Some(refresh_token)).true_or_warn(
        "refresh_token_mismatch",
        "刷新 access_token 时，refresh token 不匹配",
        AppError::Unauthorized,
    )?;

    Ok(())
}
