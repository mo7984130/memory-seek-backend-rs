use once_cell::sync::Lazy;
use regex::Regex;
use validator::ValidationError;

pub struct UsernameValidConfig;
impl UsernameValidConfig {
    pub const MIN_LENGTH: usize = 4;
    pub const MAX_LENGTH: usize = 20;
    pub const CHAR_ERROR_MSG: &str = "用户名只能包含字母、数字、下划线和短横线";
    pub const LEN_ERROR_MSG: &str = "账号长度需在 4-20 之间";
    pub const PATTERN: &str = r"^[a-zA-Z0-9_-]+$";
}
pub static USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(UsernameValidConfig::PATTERN).unwrap());

pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    let len = username.chars().count();
    if len < UsernameValidConfig::MIN_LENGTH || len > UsernameValidConfig::MAX_LENGTH {
        return Err(ValidationError::new("invalid_length").with_message(UsernameValidConfig::LEN_ERROR_MSG.into()))
    }
    if !USERNAME_REGEX.is_match(username) {
        return Err(ValidationError::new("invalid_username").with_message(UsernameValidConfig::CHAR_ERROR_MSG.into()))
    }
    Ok(())
}