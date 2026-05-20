/// 密码强度验证器
///
/// 要求密码长度 8-64 位，且必须同时包含字母和数字。
use fancy_regex::Regex;
use once_cell::sync::Lazy;
use tracing::error;
use validator::ValidationError;

/// 密码验证配置，定义长度范围、复杂性模式和错误提示信息
pub struct PasswordValidConfig;
impl PasswordValidConfig {
    pub const MIN: usize = 8;
    pub const MAX: usize = 64;
    pub const PATTERN: &'static str = r"^(?=.*[A-Za-z])(?=.*\d)\S+$";
    pub const LEN_MSG: &'static str = "密码长度需在 8 到 64 位之间";
    pub const PATTERN_MSG: &'static str = "需包含字母和数字 (包含特殊字符)";
}

static PASSWORD_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(PasswordValidConfig::PATTERN).unwrap()
});

/// 验证密码强度
///
/// 依次执行非空检查、长度检查（8-64 位）和复杂性检查（必须同时包含字母和数字）。
///
/// # 参数
/// - `password`: 待验证的密码字符串
///
/// # 返回
/// 验证通过返回 `Ok(())`，否则返回包含错误信息的 `ValidationError`
///
/// # 错误
/// - `ValidationError("required")`: 密码为空或仅包含空白字符
/// - `ValidationError("invalid_length")`: 密码长度不在 8-64 范围内
/// - `ValidationError("invalid_password")`: 密码未同时包含字母和数字
/// - `ValidationError("internal_error")`: 正则表达式匹配异常（内部错误）
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    // 1. 非空检查 (NotBlank)
    if password.trim().is_empty() {
        return Err(ValidationError::new("required").with_message("密码不能为空".into()));
    }

    // 2. 长度检查 (Length)
    let len = password.chars().count();
    if !(PasswordValidConfig::MIN..=PasswordValidConfig::MAX).contains(&len) {
        return Err(ValidationError::new("invalid_length")
            .with_message(PasswordValidConfig::LEN_MSG.into()));
    }

    // 3. 复杂性检查 (Pattern: 字母 + 数字)
    match PASSWORD_REGEX.is_match(password) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ValidationError::new("invalid_password").with_message(PasswordValidConfig::PATTERN_MSG.into())),
        Err(e) => {
            error!("密码正则解析时出现问题：{:?}", e);
            Err(ValidationError::new("internal_error").with_message("服务器内部校验错误".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试有效密码的各种情况（包含字母和数字）
    #[test]
    fn test_validate_password_valid() {
        // 有效密码测试
        assert!(validate_password("Pass1234").is_ok());
        assert!(validate_password("12345678a").is_ok());
        assert!(validate_password("abcdEFGH1").is_ok());
        assert!(validate_password("Test@123").is_ok());
        assert!(validate_password("MyP@ss2024").is_ok());
        assert!(validate_password("a1!@#$%^&*()").is_ok());
    }

    /// 测试空密码或只包含空格的密码
    #[test]
    fn test_validate_password_empty() {
        // 空密码测试
        assert!(validate_password("").is_err());
        assert!(validate_password("   ").is_err());
        assert!(validate_password("\t").is_err());
    }

    /// 测试密码长度不符合要求的情况（小于 8 位或大于 64 位）
    #[test]
    fn test_validate_password_invalid_length() {
        // 长度过短
        assert!(validate_password("Aa1").is_err());
        assert!(validate_password("Pass1").is_err());
        assert!(validate_password("1234567").is_err()); // 7 位
        
        // 长度过长
        let long_password = "a".repeat(65);
        assert!(validate_password(&long_password).is_err());
    }

    /// 测试密码只包含字母的情况（必须同时包含字母和数字）
    #[test]
    fn test_validate_password_only_letters() {
        // 只有字母
        assert!(validate_password("abcdefgh").is_err());
        assert!(validate_password("ABCDEFGH").is_err());
        assert!(validate_password("AbCdEfGh").is_err());
    }

    /// 测试密码只包含数字的情况（必须同时包含字母和数字）
    #[test]
    fn test_validate_password_only_numbers() {
        // 只有数字
        assert!(validate_password("12345678").is_err());
        assert!(validate_password("00000000").is_err());
        assert!(validate_password("99999999").is_err());
    }

    /// 测试密码长度的边界值（最小 8 位，最大 64 位）
    #[test]
    fn test_validate_password_boundary() {
        // 边界值测试
        assert!(validate_password("Aa123456").is_ok()); // 最小长度 8
        let valid_64 = "Aa123456".repeat(8); // 64 位
        assert!(validate_password(&valid_64).is_ok());
        
        let invalid_65 = "Aa1234567".repeat(8); // 72 位
        assert!(validate_password(&invalid_65).is_err());
    }

    /// 测试包含特殊字符的有效密码
    #[test]
    fn test_validate_password_with_special_chars() {
        // 包含特殊字符的有效密码
        assert!(validate_password("P@ss1234").is_ok());
        assert!(validate_password("Test#2024").is_ok());
        assert!(validate_password("My$Pass1").is_ok());
        assert!(validate_password("Abc!@#123").is_ok());
    }
}

