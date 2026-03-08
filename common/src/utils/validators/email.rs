use once_cell::sync::Lazy;
use regex::Regex;
use validator::ValidationError;

pub static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$").unwrap() );

pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if !EMAIL_REGEX.is_match(email) {
        return Err(ValidationError::new("invalid_email").with_message("邮箱格式不正确".into()));
    }
    Ok(())
}