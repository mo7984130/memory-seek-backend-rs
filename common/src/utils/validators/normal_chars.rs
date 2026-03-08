use once_cell::sync::Lazy;
use regex::Regex;
use validator::ValidationError;

pub struct CommonValidConfig;

impl CommonValidConfig {
    pub const NORMAL_CHAR_PATTERN: &'static str = r#"^[^<>/\\"\'&@]+$"#;
    pub const NORMAL_CHAR_MSG: &'static str = "不能包含 < > / \\ \" ' & @等特殊符号";
}

static NORMAL_CHAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(CommonValidConfig::NORMAL_CHAR_PATTERN).expect("Invalid Normal Char Regex")
});

pub fn validate_normal_char(value: &str) -> Result<(), ValidationError> {
    if !NORMAL_CHAR_REGEX.is_match(value) {
        return Err(ValidationError::new("invalid_characters")
            .with_message(CommonValidConfig::NORMAL_CHAR_MSG.into()));
    }
    Ok(())
}