/// 生成用户访问令牌的 Redis 缓存键
///
/// # 参数
/// - `user_id`: 用户 ID
///
/// # 返回
/// 格式为 `a:u:at:{user_id}` 的缓存键
#[inline]
pub fn user_access_token(user_id: i64) -> String {
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
/// 格式为 `u:i:{user_id}` 的缓存键
#[inline]
pub fn user_info_cache(user_id: i64) -> String {
    //auth:user:info
    format!("a:u:i:{}", user_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_and_user_keys() {
        // 1. 测试 user_access_token (i64)
        assert_eq!(user_access_token(10086), "a:u:at:10086");
        assert_eq!(user_access_token(0), "a:u:at:0");

        // 2. 测试 email_verify_code (&str)
        // 注意验证特殊字符是否被原样保留（Redis Key 是支持的）
        assert_eq!(
            email_verify_code("test@example.com"),
            "a:v:e:test@example.com"
        );
        assert_eq!(
            email_verify_code("user.name+label@gmail.com"),
            "a:v:e:user.name+label@gmail.com"
        );

        // 3. 测试 inviter_code (&str)
        assert_eq!(inviter_code("RUST666"), "a:i:c:RUST666");
        assert_eq!(inviter_code(""), "a:i:c:"); // 边界测试：空字符串

        // 4. 测试 user_info_cache (i64)
        assert_eq!(user_info_cache(123456789), "u:i:123456789");
    }
}
