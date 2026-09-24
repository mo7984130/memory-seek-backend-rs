//! 公共输入校验器
//!
//! 提供统一的账号、用户名、邮箱、密码和常规字符验证功能。
//! 各验证函数返回 `Result<(), ValidationError>`，可与 `validator` crate 的
//! `Validate` trait 配合使用（通过 `#[validate(custom(...))]` 引用）。
//!
//! - `validate_account`: 账号验证（自动识别用户名或邮箱）
//! - `validate_username`: 用户名验证（字母、数字、下划线、短横线，4-20 位）
//! - `validate_email`: 邮箱格式验证
//! - `validate_password`: 密码强度验证（8-64 位，必须包含字母和数字）
//! - `validate_normal_char`: 常规字符验证（禁止 `< > / \ " ' & @` 等特殊符号）

use std::sync::LazyLock;

use validator::ValidationError;

// ==================== 用户名 ====================

/// 用户名验证配置，定义长度范围、允许字符模式和错误提示信息
pub struct UsernameValidConfig;
impl UsernameValidConfig {
    pub const MIN_LENGTH: usize = 4;
    pub const MAX_LENGTH: usize = 20;
    pub const CHAR_ERROR_MSG: &str = "用户名只能包含字母、数字、下划线和短横线";
    pub const LEN_ERROR_MSG: &str = "账号长度需在 4-20 之间";
    pub const PATTERN: &str = r"^[a-zA-Z0-9_-]+$";
}

static USERNAME_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(UsernameValidConfig::PATTERN).unwrap());

/// 验证用户名格式
///
/// 检查用户名长度是否在 4-20 个字符之间，且仅包含字母、数字、下划线和短横线。
/// 校验用户名格式和长度.
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

// ==================== 邮箱 ====================

/// 邮箱正则表达式，匹配 `local@domain.tld` 格式，支持子域名和 `+` 标签
pub static EMAIL_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)*\.[a-zA-Z0-9-]+$").unwrap()
});

/// 验证邮箱地址格式
///
/// 检查邮箱是否为空、是否包含连续的点号，并通过正则表达式验证整体格式。
/// 校验邮箱格式.
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if email.is_empty() {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }

    if email.contains("..") {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }

    if !EMAIL_REGEX.is_match(email) {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }
    Ok(())
}

// ==================== 密码 ====================

/// 密码验证配置，定义长度范围、复杂性模式和错误提示信息
pub struct PasswordValidConfig;
impl PasswordValidConfig {
    pub const MIN: usize = 8;
    pub const MAX: usize = 64;
    pub fn is_valid_password(password: &str) -> bool {
        if password.is_empty() {
            return false;
        }

        let mut has_letter = false;
        let mut has_digit = false;

        for c in password.chars() {
            if c.is_whitespace() {
                return false;
            }

            has_letter |= c.is_ascii_alphabetic();
            has_digit |= c.is_ascii_digit();
        }

        has_letter && has_digit
    }
    pub const LEN_MSG: &'static str = "密码长度需在 8 到 64 位之间";
    pub const PATTERN_MSG: &'static str = "需包含字母和数字 (包含特殊字符)";
}

/// 验证密码强度
///
/// 依次执行非空检查、长度检查（8-64 位）和复杂性检查（必须同时包含字母和数字）。
/// 校验密码长度和字符要求.
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
    match PasswordValidConfig::is_valid_password(password) {
        true => Ok(()),
        false => Err(ValidationError::new("invalid_password")
            .with_message(PasswordValidConfig::PATTERN_MSG.into())),
    }
}

// ==================== 常规字符 ====================

/// 常规字符验证配置，定义允许的字符模式和错误提示信息
pub struct CommonValidConfig;

impl CommonValidConfig {
    pub const NORMAL_CHAR_PATTERN: &'static str = r#"^[^<>/\\"'&@]+$"#;
    pub const NORMAL_CHAR_MSG: &'static str = "不能包含 < > / \\ \" ' & @等特殊符号";
}

static NORMAL_CHAR_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(CommonValidConfig::NORMAL_CHAR_PATTERN).expect("Invalid Normal Char Regex")
});

/// 验证字符串是否仅包含常规字符（不允许 `< > / \ " ' & @` 等特殊符号）
///
/// 空字符串或仅包含空白字符的字符串也会被拒绝。
/// 校验字符串仅包含允许的普通字符.
pub fn validate_normal_char(value: &str) -> Result<(), ValidationError> {
    // 空字符串或只包含空格的字符串直接拒绝
    if value.is_empty() || value.trim().is_empty() {
        return Err(ValidationError::new("invalid_characters")
            .with_message(CommonValidConfig::NORMAL_CHAR_MSG.into()));
    }

    if !NORMAL_CHAR_REGEX.is_match(value) {
        return Err(ValidationError::new("invalid_characters")
            .with_message(CommonValidConfig::NORMAL_CHAR_MSG.into()));
    }
    Ok(())
}

// ==================== 账号 ====================

/// 验证账号格式，支持用户名或邮箱两种形式
///
/// 根据输入中是否包含 `@` 自动判断验证策略：包含 `@` 时按邮箱格式验证，
/// 否则按用户名格式验证（字符规则 + 长度约束）。
/// 校验登录账号符合用户名或邮箱格式.
pub fn validate_account(value: &str) -> Result<(), ValidationError> {
    if value.contains('@') {
        if !EMAIL_REGEX.is_match(value) {
            return Err(
                ValidationError::new("invalid_email").with_message("请输入正确的邮箱地址".into())
            );
        }
    } else {
        if !USERNAME_REGEX.is_match(value) {
            return Err(ValidationError::new("invalid_username")
                .with_message(UsernameValidConfig::CHAR_ERROR_MSG.into()));
        }
        let len = value.chars().count();
        if !(UsernameValidConfig::MIN_LENGTH..=UsernameValidConfig::MAX_LENGTH).contains(&len) {
            return Err(ValidationError::new("invalid_length")
                .with_message(UsernameValidConfig::LEN_ERROR_MSG.into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== validate_username ====================

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("user").is_ok());
        assert!(validate_username("User123").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("user-name").is_ok());
        assert!(validate_username("a1_b-c").is_ok());
        assert!(validate_username("1234").is_ok());
        assert!(validate_username("12345678901234567890").is_ok()); // 20 字符
    }

    #[test]
    fn test_validate_username_invalid_length() {
        assert!(validate_username("abc").is_err());
        assert!(validate_username("").is_err());
        assert!(validate_username("a").is_err());
        assert!(validate_username("123456789012345678901").is_err()); // 21 字符
        assert!(validate_username("123456789012345678901234567890").is_err()); // 30 字符
    }

    #[test]
    fn test_validate_username_invalid_chars() {
        assert!(validate_username("user name").is_err()); // 空格
        assert!(validate_username("user@name").is_err()); // @
        assert!(validate_username("user#name").is_err()); // #
        assert!(validate_username("user$name").is_err()); // $
        assert!(validate_username("user 名字").is_err()); // 中文
        assert!(validate_username("user!name").is_err()); // !
    }

    #[test]
    fn test_validate_username_boundary() {
        assert!(validate_username("1234").is_ok()); // 最小长度 4
        assert!(validate_username("12345678901234567890").is_ok()); // 最大长度 20
        assert!(validate_username("123").is_err()); // 长度 3
        assert!(validate_username("123456789012345678901").is_err()); // 长度 21
    }

    // ==================== validate_email ====================

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("user.name@gmail.com").is_ok());
        assert!(validate_email("user+label@gmail.com").is_ok());
        assert!(validate_email("user_name@example.com").is_ok());
        assert!(validate_email("user-name@example.com").is_ok());
        assert!(validate_email("123456@qq.com").is_ok());
        assert!(validate_email("test@sub.example.com").is_ok());
        assert!(validate_email("test@example.co.uk").is_ok());
    }

    #[test]
    fn test_validate_email_invalid_format() {
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

    #[test]
    fn test_validate_email_special_chars() {
        assert!(validate_email("test+label@example.com").is_ok());
        assert!(validate_email("test_user@example.com").is_ok());
        assert!(validate_email("test.user@example.com").is_ok());
        assert!(validate_email("test-user@example.com").is_ok());
        assert!(validate_email("test#user@example.com").is_err());
        assert!(validate_email("test$user@example.com").is_err());
    }

    // ==================== validate_password ====================

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("Pass1234").is_ok());
        assert!(validate_password("12345678a").is_ok());
        assert!(validate_password("abcdEFGH1").is_ok());
        assert!(validate_password("Test@123").is_ok());
        assert!(validate_password("MyP@ss2024").is_ok());
        assert!(validate_password("a1!@#$%^&*()").is_ok());
    }

    #[test]
    fn test_validate_password_empty() {
        assert!(validate_password("").is_err());
        assert!(validate_password("   ").is_err());
        assert!(validate_password("\t").is_err());
    }

    #[test]
    fn test_validate_password_invalid_length() {
        assert!(validate_password("Aa1").is_err());
        assert!(validate_password("Pass1").is_err());
        assert!(validate_password("1234567").is_err()); // 7 位
        let long_password = "a".repeat(65);
        assert!(validate_password(&long_password).is_err());
    }

    #[test]
    fn test_validate_password_only_letters() {
        assert!(validate_password("abcdefgh").is_err());
        assert!(validate_password("ABCDEFGH").is_err());
        assert!(validate_password("AbCdEfGh").is_err());
    }

    #[test]
    fn test_validate_password_only_numbers() {
        assert!(validate_password("12345678").is_err());
        assert!(validate_password("00000000").is_err());
        assert!(validate_password("99999999").is_err());
    }

    #[test]
    fn test_validate_password_boundary() {
        assert!(validate_password("Aa123456").is_ok()); // 最小长度 8
        let valid_64 = "Aa123456".repeat(8); // 64 位
        assert!(validate_password(&valid_64).is_ok());
        let invalid_65 = "Aa1234567".repeat(8); // 72 位
        assert!(validate_password(&invalid_65).is_err());
    }

    #[test]
    fn test_validate_password_with_special_chars() {
        assert!(validate_password("P@ss1234").is_ok());
        assert!(validate_password("Test#2024").is_ok());
        assert!(validate_password("My$Pass1").is_ok());
        assert!(validate_password("Abc!@#123").is_ok());
    }

    // ==================== validate_normal_char ====================

    #[test]
    fn test_validate_normal_char_valid() {
        assert!(validate_normal_char("hello").is_ok());
        assert!(validate_normal_char("你好").is_ok());
        assert!(validate_normal_char("test123").is_ok());
        assert!(validate_normal_char("test_name").is_ok());
        assert!(validate_normal_char("test-name").is_ok());
        assert!(validate_normal_char("测试 123").is_ok());
        assert!(validate_normal_char("Hello World").is_ok());
    }

    #[test]
    fn test_validate_normal_char_invalid() {
        assert!(validate_normal_char("test<value>").is_err()); // < >
        assert!(validate_normal_char("test/value").is_err()); // /
        assert!(validate_normal_char(r"test\value").is_err()); // \
        assert!(validate_normal_char("test\"value\"").is_err()); // "
        assert!(validate_normal_char("test'value'").is_err()); // '
        assert!(validate_normal_char("test&value").is_err()); // &
        assert!(validate_normal_char("test@value").is_err()); // @
    }

    #[test]
    fn test_validate_normal_char_multiple_invalid_chars() {
        assert!(validate_normal_char("<>/\\").is_err());
        assert!(validate_normal_char("\"'&@").is_err());
        assert!(validate_normal_char("test<@>&").is_err());
    }

    #[test]
    fn test_validate_normal_char_empty() {
        assert!(validate_normal_char("").is_err());
        assert!(validate_normal_char(" ").is_err());
        assert!(validate_normal_char("   ").is_err());
        assert!(validate_normal_char("\t").is_err());
        assert!(validate_normal_char("  \t  ").is_err());
    }

    // ==================== validate_account ====================

    #[test]
    fn test_validate_account_valid_username() {
        assert!(validate_account("user123").is_ok());
        assert!(validate_account("test_user").is_ok());
        assert!(validate_account("user-name").is_ok());
        assert!(validate_account("12345678").is_ok());
    }

    #[test]
    fn test_validate_account_valid_email() {
        assert!(validate_account("test@example.com").is_ok());
        assert!(validate_account("user@gmail.com").is_ok());
        assert!(validate_account("test+label@qq.com").is_ok());
    }

    #[test]
    fn test_validate_account_invalid_username() {
        assert!(validate_account("abc").is_err()); // 长度过短
        assert!(validate_account("user name").is_err()); // 包含空格
        assert!(validate_account("user@name").is_err()); // 包含@但不是邮箱格式
        assert!(validate_account("用户名字").is_err()); // 中文
    }

    #[test]
    fn test_validate_account_invalid_email() {
        assert!(validate_account("test@").is_err());
        assert!(validate_account("@example.com").is_err());
        assert!(validate_account("test@example").is_err());
        assert!(validate_account("test@@example.com").is_err());
        assert!(validate_account("example@example..com").is_err());
    }

    #[test]
    fn test_validate_account_boundary() {
        assert!(validate_account("1234").is_ok()); // 用户名最小长度
        assert!(validate_account("12345678901234567890").is_ok()); // 用户名最大长度
        assert!(validate_account("123").is_err()); // 用户名长度不足
    }
}
