/// 用户名验证器
///
/// 要求用户名仅包含字母、数字、下划线和短横线，长度 4-20 个字符。
use once_cell::sync::Lazy;
use regex::Regex;
use validator::ValidationError;

/// 用户名验证配置，定义长度范围、允许字符模式和错误提示信息
pub struct UsernameValidConfig;
impl UsernameValidConfig {
    pub const MIN_LENGTH: usize = 4;
    pub const MAX_LENGTH: usize = 20;
    pub const CHAR_ERROR_MSG: &str = "用户名只能包含字母、数字、下划线和短横线";
    pub const LEN_ERROR_MSG: &str = "账号长度需在 4-20 之间";
    pub const PATTERN: &str = r"^[a-zA-Z0-9_-]+$";
}
pub static USERNAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(UsernameValidConfig::PATTERN).unwrap());

/// 验证用户名格式
///
/// 检查用户名长度是否在 4-20 个字符之间，且仅包含字母、数字、下划线和短横线。
///
/// # 参数
/// - `username`: 待验证的用户名字符串
///
/// # 返回
/// 验证通过返回 `Ok(())`，否则返回包含错误信息的 `ValidationError`
///
/// # 错误
/// - `ValidationError("invalid_length")`: 用户名长度不在 4-20 范围内
/// - `ValidationError("invalid_username")`: 用户名包含非法字符
pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    let len = username.chars().count();
    if !(UsernameValidConfig::MIN_LENGTH..=UsernameValidConfig::MAX_LENGTH).contains(&len) {
        return Err(ValidationError::new("invalid_length")
            .with_message(UsernameValidConfig::LEN_ERROR_MSG.into()));
    }
    if !USERNAME_REGEX.is_match(username) {
        return Err(ValidationError::new("invalid_username")
            .with_message(UsernameValidConfig::CHAR_ERROR_MSG.into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试有效用户名的各种情况
    #[test]
    fn test_validate_username_valid() {
        // 有效用户名测试
        assert!(validate_username("user").is_ok());
        assert!(validate_username("User123").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("user-name").is_ok());
        assert!(validate_username("a1_b-c").is_ok());
        assert!(validate_username("1234").is_ok());
        assert!(validate_username("12345678901234567890").is_ok()); // 20 字符
    }

    /// 测试用户名长度不符合要求的情况（过短或过长）
    #[test]
    fn test_validate_username_invalid_length() {
        // 长度过短
        assert!(validate_username("abc").is_err());
        assert!(validate_username("").is_err());
        assert!(validate_username("a").is_err());

        // 长度过长
        assert!(validate_username("123456789012345678901").is_err()); // 21 字符
        assert!(validate_username("123456789012345678901234567890").is_err()); // 30 字符
    }

    /// 测试用户名包含非法字符的情况
    #[test]
    fn test_validate_username_invalid_chars() {
        // 包含非法字符
        assert!(validate_username("user name").is_err()); // 空格
        assert!(validate_username("user@name").is_err()); // @
        assert!(validate_username("user#name").is_err()); // #
        assert!(validate_username("user$name").is_err()); // $
        assert!(validate_username("user 名字").is_err()); // 中文
        assert!(validate_username("user!name").is_err()); // !
    }

    /// 测试用户名长度的边界值（最小长度 4，最大长度 20）
    #[test]
    fn test_validate_username_boundary() {
        // 边界值测试
        assert!(validate_username("1234").is_ok()); // 最小长度 4
        assert!(validate_username("12345678901234567890").is_ok()); // 最大长度 20
        assert!(validate_username("123").is_err()); // 长度 3
        assert!(validate_username("123456789012345678901").is_err()); // 长度 21
    }
}
