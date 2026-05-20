/// 常规字符验证器
///
/// 禁止 `< > / \ " ' & @` 等特殊符号，适用于名称、标题等通用文本输入。
use once_cell::sync::Lazy;
use regex::Regex;
use validator::ValidationError;

/// 常规字符验证配置，定义允许的字符模式和错误提示信息
pub struct CommonValidConfig;

impl CommonValidConfig {
    pub const NORMAL_CHAR_PATTERN: &'static str = r#"^[^<>/\\"'&@]+$"#;
    pub const NORMAL_CHAR_MSG: &'static str = "不能包含 < > / \\ \" ' & @等特殊符号";
}

static NORMAL_CHAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(CommonValidConfig::NORMAL_CHAR_PATTERN).expect("Invalid Normal Char Regex")
});

/// 验证字符串是否仅包含常规字符（不允许 `< > / \ " ' & @` 等特殊符号）
///
/// 空字符串或仅包含空白字符的字符串也会被拒绝。
///
/// # 参数
/// - `value`: 待验证的字符串
///
/// # 返回
/// 验证通过返回 `Ok(())`，否则返回包含错误信息的 `ValidationError`
///
/// # 错误
/// - `ValidationError("invalid_characters")`: 字符串为空或包含禁止的特殊符号
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试符合要求的普通字符（不包含禁止的特殊符号）
    #[test]
    fn test_validate_normal_char_valid() {
        // 有效字符测试
        assert!(validate_normal_char("hello").is_ok());
        assert!(validate_normal_char("你好").is_ok());
        assert!(validate_normal_char("test123").is_ok());
        assert!(validate_normal_char("test_name").is_ok());
        assert!(validate_normal_char("test-name").is_ok());
        assert!(validate_normal_char("测试 123").is_ok());
        assert!(validate_normal_char("Hello World").is_ok());
    }

    /// 测试包含禁止字符的情况（< > / \ " ' & @ 等）
    #[test]
    fn test_validate_normal_char_invalid() {
        // 包含禁止字符
        assert!(validate_normal_char("test<value>").is_err()); // < >
        assert!(validate_normal_char("test/value").is_err()); // /
        assert!(validate_normal_char(r"test\value").is_err()); // \
        assert!(validate_normal_char("test\"value\"").is_err()); // "
        assert!(validate_normal_char("test'value'").is_err()); // '
        assert!(validate_normal_char("test&value").is_err()); // &
        assert!(validate_normal_char("test@value").is_err()); // @
    }

    /// 测试包含多个禁止字符的情况
    #[test]
    fn test_validate_normal_char_multiple_invalid_chars() {
        // 包含多个禁止字符
        assert!(validate_normal_char("<>/\\").is_err());
        assert!(validate_normal_char("\"'&@").is_err());
        assert!(validate_normal_char("test<@>&").is_err());
    }

    /// 测试空字符串和全空格字符串的处理
    #[test]
    fn test_validate_normal_char_empty() {
        // 空字符串应该失败（长度为 0）
        assert!(validate_normal_char("").is_err());
        
        // 全为空格也应该失败
        assert!(validate_normal_char(" ").is_err());
        assert!(validate_normal_char("   ").is_err());
        assert!(validate_normal_char("\t").is_err());
        assert!(validate_normal_char("  \t  ").is_err());
    }
}