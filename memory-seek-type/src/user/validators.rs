/// 用户模块验证器

use validator::ValidationError;

/// 验证密码强度（长度 8-64，必须同时包含字母和数字）
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.trim().is_empty() {
        return Err(ValidationError::new("required").with_message("密码不能为空".into()));
    }

    let len = password.chars().count();
    if len < 8 || len > 64 {
        return Err(ValidationError::new("invalid_length")
            .with_message("密码长度需在 8 到 64 位之间".into()));
    }

    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Err(ValidationError::new("invalid_password")
            .with_message("需包含字母和数字".into()));
    }

    Ok(())
}

/// 验证常规字符（不允许 < > / \ " ' & @ 等特殊符号）
pub fn validate_normal_char(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() || value.trim().is_empty() {
        return Err(ValidationError::new("invalid_characters")
            .with_message("不能为空".into()));
    }

    for c in value.chars() {
        match c {
            '<' | '>' | '/' | '\\' | '"' | '\'' | '&' | '@' => {
                return Err(ValidationError::new("invalid_characters")
                    .with_message("不能包含 < > / \\ \" \' & @等特殊符号".into()));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("Pass1234").is_ok());
        assert!(validate_password("12345678a").is_ok());
        assert!(validate_password("abcdEFGH1").is_ok());
        assert!(validate_password("Test@123").is_ok());
    }

    #[test]
    fn test_validate_password_empty() {
        assert!(validate_password("").is_err());
        assert!(validate_password("   ").is_err());
    }

    #[test]
    fn test_validate_password_only_letters() {
        assert!(validate_password("abcdefgh").is_err());
    }

    #[test]
    fn test_validate_password_only_numbers() {
        assert!(validate_password("12345678").is_err());
    }

    #[test]
    fn test_validate_password_too_short() {
        assert!(validate_password("Aa1").is_err());
        assert!(validate_password("Pass1").is_err());
    }

    #[test]
    fn test_validate_normal_char_valid() {
        assert!(validate_normal_char("hello").is_ok());
        assert!(validate_normal_char("Hello World").is_ok());
        assert!(validate_normal_char("test123").is_ok());
        assert!(validate_normal_char("test_name").is_ok());
        assert!(validate_normal_char("test-name").is_ok());
    }

    #[test]
    fn test_validate_normal_char_invalid() {
        assert!(validate_normal_char("test<value>").is_err());
        assert!(validate_normal_char("test/value").is_err());
        assert!(validate_normal_char("test\\value").is_err());
        assert!(validate_normal_char("test\"value\"").is_err());
        assert!(validate_normal_char("test\'value\'").is_err());
        assert!(validate_normal_char("test&value").is_err());
        assert!(validate_normal_char("test@value").is_err());
    }

    #[test]
    fn test_validate_normal_char_empty() {
        assert!(validate_normal_char("").is_err());
        assert!(validate_normal_char(" ").is_err());
        assert!(validate_normal_char("   ").is_err());
    }
}
