/// 账号验证器
///
/// 支持用户名和邮箱两种账号格式的自动识别与验证。
use crate::utils::validators::email::EMAIL_REGEX;
use crate::utils::validators::username::{UsernameValidConfig, USERNAME_REGEX};
use validator::ValidationError;

/// 验证账号格式，支持用户名或邮箱两种形式
///
/// 根据输入中是否包含 `@` 自动判断验证策略：包含 `@` 时按邮箱格式验证，
/// 否则按用户名格式验证（字符规则 + 长度约束）。
///
/// # 参数
/// - `value`: 待验证的账号字符串，可以是用户名或邮箱
///
/// # 返回
/// 验证通过返回 `Ok(())`，否则返回包含错误信息的 `ValidationError`
///
/// # 错误
/// - `ValidationError("invalid_email")`: 输入包含 `@` 但不符合邮箱格式
/// - `ValidationError("invalid_username")`: 输入不含 `@` 且包含非法字符
/// - `ValidationError("invalid_length")`: 输入不含 `@` 且长度不在 4-20 范围内
pub fn validate_account(value: &str) -> Result<(), ValidationError> {
    if value.contains('@') {
        if !EMAIL_REGEX.is_match(value) {
            return Err(ValidationError::new("invalid_email").with_message("请输入正确的邮箱地址".into()));
        }
    } else {
        if !USERNAME_REGEX.is_match(value) {
            return Err(ValidationError::new("invalid_username").with_message(UsernameValidConfig::CHAR_ERROR_MSG.into()));
        }
        let len = value.chars().count();
        if !(UsernameValidConfig::MIN_LENGTH..=UsernameValidConfig::MAX_LENGTH).contains(&len) {
            return Err(ValidationError::new("invalid_length").with_message(UsernameValidConfig::LEN_ERROR_MSG.into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试有效用户名作为账号的情况
    #[test]
    fn test_validate_account_valid_username() {
        // 有效用户名作为账号
        assert!(validate_account("user123").is_ok());
        assert!(validate_account("test_user").is_ok());
        assert!(validate_account("user-name").is_ok());
        assert!(validate_account("12345678").is_ok());
    }

    /// 测试有效邮箱作为账号的情况
    #[test]
    fn test_validate_account_valid_email() {
        // 有效邮箱作为账号
        assert!(validate_account("test@example.com").is_ok());
        assert!(validate_account("user@gmail.com").is_ok());
        assert!(validate_account("test+label@qq.com").is_ok());
    }

    /// 测试无效用户名作为账号的情况（长度、字符等）
    #[test]
    fn test_validate_account_invalid_username() {
        // 无效用户名
        assert!(validate_account("abc").is_err()); // 长度过短
        assert!(validate_account("user name").is_err()); // 包含空格
        assert!(validate_account("user@name").is_err()); // 包含@但不是邮箱格式
        assert!(validate_account("用户名字").is_err()); // 中文
    }

    /// 测试无效邮箱作为账号的情况（格式错误）
    #[test]
    fn test_validate_account_invalid_email() {
        // 无效邮箱
        assert!(validate_account("test@").is_err());
        assert!(validate_account("@example.com").is_err());
        assert!(validate_account("test@example").is_err());
        assert!(validate_account("test@@example.com").is_err());
        assert!(validate_account("example@example..com").is_err());
    }

    /// 测试账号长度的边界值（用户名最小 4 位，最大 20 位）
    #[test]
    fn test_validate_account_boundary() {
        // 边界值测试
        assert!(validate_account("1234").is_ok()); // 用户名最小长度
        assert!(validate_account("12345678901234567890").is_ok()); // 用户名最大长度
        assert!(validate_account("123").is_err()); // 用户名长度不足
    }
}