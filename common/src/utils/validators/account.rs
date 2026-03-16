use crate::utils::validators::email::EMAIL_REGEX;
use crate::utils::validators::username::{UsernameValidConfig, USERNAME_REGEX};
use validator::ValidationError;

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
        if len < UsernameValidConfig::MIN_LENGTH || len > UsernameValidConfig::MAX_LENGTH {
            return Err(ValidationError::new("invalid_length").with_message(UsernameValidConfig::LEN_ERROR_MSG.into()));
        }
    }
    Ok(())
}