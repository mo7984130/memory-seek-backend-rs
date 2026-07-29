/// 校验普通字符（无特殊符号）
pub fn validate_normal_char(s: &str) -> Result<(), validator::ValidationError> {
    if s.chars()
        .any(|c| !c.is_alphanumeric() && !c.is_whitespace() && c != '_' && c != '-' && c != '.')
    {
        return Err(validator::ValidationError::new("包含不允许的特殊字符"));
    }
    Ok(())
}

/// 校验密码
pub fn validate_password(password: &str) -> Result<(), validator::ValidationError> {
    if password.len() < 8 || password.len() > 20 {
        return Err(validator::ValidationError::new("密码长度在 8 到 20 个字符"));
    }
    Ok(())
}

/// 校验邮箱
pub fn validate_email(email: &str) -> Result<(), validator::ValidationError> {
    if !email.contains('@') || !email.contains('.') {
        return Err(validator::ValidationError::new("邮箱格式不正确"));
    }
    Ok(())
}
