use chrono::{DateTime, Utc};
use common::utils::validators::validate_account;
use common::utils::validators::validate_email;
use common::utils::validators::validate_normal_char;
use common::utils::validators::validate_password;
use common::utils::validators::validate_username;
use serde::{Deserialize, Serialize};
use validator::Validate;

// 重新导出共享类型
pub use memory_seek_type::auth::*;

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginParam {
    #[validate(custom(function = "validate_account"))]
    pub account: String,
    #[validate(custom(function = "validate_password"))]
    pub password: String
}

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterParam {
    #[validate(custom(function = "validate_username"))]
    pub username: String,
    #[validate(custom(function = "validate_email"))]
    pub email: String,
    #[validate(custom(function = "validate_password"))]
    pub password: String,
    #[validate(
        length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"),
        custom(function = "validate_normal_char")
    )]
    pub nickname: String,
    #[validate(length(min = 6, max = 6, message = "邀请码长度为6个字符"))]
    pub inviter_code: String,
    #[validate(length(min = 6, max = 6, message = "邮箱验证码长度为6个字符"))]
    pub email_verify_code: String
}

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailCodeParam {
    #[validate(custom(function = "validate_email"))]
    pub email: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenResult {
    pub access_token: String,
    pub access_token_expire_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // ==================== LoginParam validation ====================

    #[test]
    fn test_login_param_valid() {
        let param = LoginParam {
            account: "testuser1".to_string(),
            password: "pass1234".to_string(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_login_param_invalid_empty_account() {
        let param = LoginParam {
            account: "".to_string(),
            password: "pass1234".to_string(),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_login_param_invalid_short_password() {
        let param = LoginParam {
            account: "testuser1".to_string(),
            password: "pass1".to_string(),
        };
        assert!(param.validate().is_err());
    }

    // ==================== RegisterParam validation ====================

    fn valid_register_param() -> RegisterParam {
        RegisterParam {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "Pass1234".to_string(),
            nickname: "Test Nick".to_string(),
            inviter_code: "ABC123".to_string(),
            email_verify_code: "654321".to_string(),
        }
    }

    #[test]
    fn test_register_param_valid() {
        assert!(valid_register_param().validate().is_ok());
    }

    #[test]
    fn test_register_param_invalid_short_username() {
        let mut param = valid_register_param();
        param.username = "abc".to_string();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_register_param_invalid_email() {
        let mut param = valid_register_param();
        param.email = "not-an-email".to_string();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_register_param_invalid_password_no_digits() {
        let mut param = valid_register_param();
        param.password = "AbCdEfGh".to_string();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_register_param_invalid_nickname_angle_bracket() {
        let mut param = valid_register_param();
        param.nickname = "name<script>".to_string();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_register_param_invalid_inviter_code_wrong_length() {
        let mut param = valid_register_param();
        param.inviter_code = "ABC".to_string();
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_register_param_invalid_email_verify_code_wrong_length() {
        let mut param = valid_register_param();
        param.email_verify_code = "12345".to_string();
        assert!(param.validate().is_err());
    }

    // ==================== SendEmailCodeParam validation ====================

    #[test]
    fn test_send_email_code_param_valid() {
        let param = SendEmailCodeParam {
            email: "user@example.com".to_string(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_send_email_code_param_invalid_email() {
        let param = SendEmailCodeParam {
            email: "invalid-email".to_string(),
        };
        assert!(param.validate().is_err());
    }

    // ==================== AccessTokenResult serialization ====================

    #[test]
    fn test_access_token_result_serializes_to_camel_case() {
        let result = AccessTokenResult {
            access_token: "tok123".to_string(),
            access_token_expire_at: Utc.with_ymd_and_hms(2026, 6, 13, 12, 0, 0).unwrap(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"accessToken\""));
        assert!(json.contains("\"accessTokenExpireAt\""));
        assert!(!json.contains("access_token"));
    }
}
