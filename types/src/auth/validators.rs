/// 校验账号（用户名或邮箱）
pub fn validate_account(account: &str) -> Result<(), validator::ValidationError> {
    if account.is_empty() || account.len() < 4 || account.len() > 50 {
        return Err(validator::ValidationError::new("invalid_length")
            .with_message("账号长度在 4 到 50 个字符".into()));
    }
    Ok(())
}

/// 校验密码
pub fn validate_password(password: &str) -> Result<(), validator::ValidationError> {
    if password.len() < 8 || password.len() > 20 {
        return Err(validator::ValidationError::new("invalid_length")
            .with_message("密码长度在 8 到 20 个字符".into()));
    }
    Ok(())
}

/// 校验邮箱
pub fn validate_email(email: &str) -> Result<(), validator::ValidationError> {
    if !email.contains('@') || !email.contains('.') {
        return Err(validator::ValidationError::new("invalid_email")
            .with_message("邮箱格式不正确".into()));
    }
    Ok(())
}

/// 校验用户名
pub fn validate_username(username: &str) -> Result<(), validator::ValidationError> {
    if username.len() < 4 || username.len() > 20 {
        return Err(validator::ValidationError::new("invalid_length").with_message(
            "用户名长度在 4 到 20 个字符".into(),
        ));
    }
    Ok(())
}
