use once_cell::sync::Lazy;
use fancy_regex::Regex;
use tracing::error;
use validator::ValidationError;

pub struct PasswordValidConfig;
impl PasswordValidConfig {
    pub const MIN: usize = 8;
    pub const MAX: usize = 64;
    pub const PATTERN: &'static str = r"^(?=.*[A-Za-z])(?=.*\d)\S+$";
    pub const LEN_MSG: &'static str = "密码长度需在 8 到 64 位之间";
    pub const PATTERN_MSG: &'static str = "需包含字母和数字(包含特殊字符)";
}

static PASSWORD_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(PasswordValidConfig::PATTERN).unwrap()
});

pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    // 1. 非空检查 (NotBlank)
    if password.trim().is_empty() {
        return Err(ValidationError::new("required").with_message("密码不能为空".into()));
    }

    // 2. 长度检查 (Length)
    let len = password.chars().count();
    if len < PasswordValidConfig::MIN || len > PasswordValidConfig::MAX {
        return Err(ValidationError::new("invalid_length")
            .with_message(PasswordValidConfig::LEN_MSG.into()));
    }

    // 3. 复杂性检查 (Pattern: 字母+数字)
    match PASSWORD_REGEX.is_match(password) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ValidationError::new("invalid_password").with_message(PasswordValidConfig::PATTERN_MSG.into())),
        Err(e) => {
            error!("密码正则解析时出现问题: {:?}", e);
            Err(ValidationError::new("internal_error").with_message("服务器内部校验错误".into()))
        }
    }
}

