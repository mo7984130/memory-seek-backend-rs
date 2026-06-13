/// 邮箱格式验证器
use once_cell::sync::Lazy;
use regex::Regex;
use validator::ValidationError;

/// 邮箱正则表达式，匹配 `local@domain.tld` 格式，支持子域名和 `+` 标签
pub static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)*\.[a-zA-Z0-9-]+$").unwrap()
});

/// 验证邮箱地址格式
///
/// 检查邮箱是否为空、是否包含连续的点号，并通过正则表达式验证整体格式。
///
/// # 参数
/// - `email`: 待验证的邮箱地址字符串
///
/// # 返回
/// 验证通过返回 `Ok(())`，否则返回包含错误信息的 `ValidationError`
///
/// # 错误
/// - `ValidationError("invalid_email")`: 邮箱为空、包含连续点号或格式不合法
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    // 空字符串直接拒绝
    if email.is_empty() {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }

    // 检查是否有连续的点
    if email.contains("..") {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }

    if !EMAIL_REGEX.is_match(email) {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试有效邮箱地址的各种格式
    #[test]
    fn test_validate_email_valid() {
        // 有效邮箱测试
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("user.name@gmail.com").is_ok());
        assert!(validate_email("user+label@gmail.com").is_ok());
        assert!(validate_email("user_name@example.com").is_ok());
        assert!(validate_email("user-name@example.com").is_ok());
        assert!(validate_email("123456@qq.com").is_ok());
        assert!(validate_email("test@sub.example.com").is_ok());
        assert!(validate_email("test@example.co.uk").is_ok());
    }

    /// 测试无效邮箱地址的各种情况（格式错误、缺少部分等）
    #[test]
    fn test_validate_email_invalid_format() {
        // 无效邮箱测试
        assert!(validate_email("").is_err());
        assert!(validate_email("test").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("test@").is_err());
        assert!(validate_email("test@example").is_err());
        assert!(validate_email("test@@example.com").is_err());
        assert!(validate_email("test@example..com").is_err());
        assert!(validate_email("test example.com").is_err());
        assert!(validate_email("test@exam ple.com").is_err());
    }

    /// 测试邮箱地址中特殊字符的处理（+、_、.、- 允许，其他不允许）
    #[test]
    fn test_validate_email_special_chars() {
        // 特殊字符测试
        assert!(validate_email("test+label@example.com").is_ok());
        assert!(validate_email("test_user@example.com").is_ok());
        assert!(validate_email("test.user@example.com").is_ok());
        assert!(validate_email("test-user@example.com").is_ok());
        assert!(validate_email("test#user@example.com").is_err());
        assert!(validate_email("test$user@example.com").is_err());
    }
}
