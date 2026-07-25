use entities::auth::user::UserId;

/// 生成用户访问令牌的 Redis 缓存键
///
/// # 参数
/// - `user_id`: 用户 ID
///
/// # 返回
/// 格式为 `a:u:at:{user_id}` 的缓存键
#[inline]
pub fn user_access_token(user_id: UserId) -> String {
    //auth:user:accessToken
    format!("a:u:at:{}", user_id)
}

/// 生成邮箱验证码的 Redis 缓存键
///
/// # 参数
/// - `email`: 邮箱地址
///
/// # 返回
/// 格式为 `a:v:e:{email}` 的缓存键
#[inline]
pub fn email_verify_code(email: &str) -> String {
    //auth:verify:email
    format!("a:v:e:{}", email)
}

/// 生成邀请码的 Redis 缓存键
///
/// # 参数
/// - `code`: 邀请码
///
/// # 返回
/// 格式为 `a:i:c:{code}` 的缓存键
#[inline]
pub fn inviter_code(code: &str) -> String {
    //auth:inviter:code
    format!("a:i:c:{}", code)
}

/// 生成用户信息的 Redis 缓存键
///
/// # 参数
/// - `user_id`: 用户 ID
///
/// # 返回
/// 格式为 `a:u:i:{user_id}` 的缓存键
#[inline]
pub fn user_info_cache(user_id: UserId) -> String {
    //auth:user:info
    format!("a:u:i:{}", user_id)
}
