use validator::ValidationError;

/// 验证账号（用户名或邮箱）
pub fn validate_account(account: &str) -> Result<(), ValidationError> {
    if account.is_empty() {
        return Err(ValidationError::new("账号不能为空"));
    }
    if account.contains('@') {
        validate_email(account)?;
    } else {
        validate_username(account)?;
    }
    Ok(())
}

/// 验证用户名
pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    if username.len() < 4 || username.len() > 20 {
        return Err(ValidationError::new("用户名长度在 4 到 20 个字符"));
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(ValidationError::new("用户名只能包含字母、数字和下划线"));
    }
    Ok(())
}

/// 验证邮箱
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if !email.contains('@') || !email.contains('.') {
        return Err(ValidationError::new("邮箱格式不正确"));
    }
    Ok(())
}

/// 验证密码
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.len() < 6 {
        return Err(ValidationError::new("密码长度至少为6个字符"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_account_valid_username() {
        assert!(validate_account("testuser").is_ok());
    }

    #[test]
    fn test_validate_account_valid_email() {
        assert!(validate_account("test@example.com").is_ok());
    }

    #[test]
    fn test_validate_account_empty() {
        assert!(validate_account("").is_err());
    }

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("test_user").is_ok());
    }

    #[test]
    fn test_validate_username_too_short() {
        assert!(validate_username("abc").is_err());
    }

    #[test]
    fn test_validate_username_too_long() {
        assert!(validate_username(&"a".repeat(21)).is_err());
    }

    #[test]
    fn test_validate_username_invalid_chars() {
        assert!(validate_username("test@user").is_err());
    }

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("test@example.com").is_ok());
    }

    #[test]
    fn test_validate_email_invalid() {
        assert!(validate_email("invalid-email").is_err());
    }

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("password123").is_ok());
    }

    #[test]
    fn test_validate_password_too_short() {
        assert!(validate_password("12345").is_err());
    }
}
