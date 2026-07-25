//! 认证相关类型定义

use super::validators::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use validator::Validate;

/// 登录请求
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    #[validate(custom(function = "validate_account"))]
    pub account: String,

    #[validate(custom(function = "validate_password"))]
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub user: crate::user::UserInfo,
    pub access_token: String,
    pub access_token_expire_at: chrono::DateTime<chrono::Utc>,
    pub refresh_token: String,
    pub refresh_token_expire_at: chrono::DateTime<chrono::Utc>,
}


/// Token 刷新响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RefreshAccessTokenResponse {
    /// 访问令牌
    pub access_token: String,
    /// 访问令牌过期时间
    pub access_token_expire_at: DateTime<Utc>,
}

/// 注册请求
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    /// 用户名
    #[validate(custom(function = "validate_username"))]
    pub username: String,

    /// 邮箱
    #[validate(custom(function = "validate_email"))]
    pub email: String,

    /// 密码
    #[validate(custom(function = "validate_password"))]
    pub password: String,

    /// 昵称
    #[validate(length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"))]
    pub nickname: String,

    /// 邀请码
    #[validate(length(min = 6, max = 6, message = "邀请码长度为6个字符"))]
    pub inviter_code: String,

    /// 邮箱验证码
    #[validate(length(min = 6, max = 6, message = "邮箱验证码长度为6个字符"))]
    pub email_verify_code: String,
}

/// 注册响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResponse {
    /// 用户ID
    pub user_id: String,

    /// 访问令牌
    pub access_token: String,

    /// 访问令牌过期时间
    pub access_token_expire_at: DateTime<Utc>,
}

/// 发送邮箱验证码请求
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailCodeRequest {
    /// 邮箱
    #[validate(custom(function = "validate_email"))]
    pub email: String,
}

/// 发送邮箱验证码响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailCodeResponse {
    /// 是否成功
    pub success: bool,

    /// 消息
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== LoginRequest validation ====================

    #[test]
    fn test_login_request_valid() {
        let request = LoginRequest {
            account: "testuser1".to_string(),
            password: "pass1234".to_string(),
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_login_request_invalid_empty_account() {
        let request = LoginRequest {
            account: "".to_string(),
            password: "pass1234".to_string(),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_login_request_invalid_short_password() {
        let request = LoginRequest {
            account: "testuser1".to_string(),
            password: "pass1".to_string(),
        };
        assert!(request.validate().is_err());
    }

    // ==================== RegisterRequest validation ====================

    fn valid_register_request() -> RegisterRequest {
        RegisterRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "Pass1234".to_string(),
            nickname: "Test Nick".to_string(),
            inviter_code: "ABC123".to_string(),
            email_verify_code: "654321".to_string(),
        }
    }

    #[test]
    fn test_register_request_valid() {
        assert!(valid_register_request().validate().is_ok());
    }

    #[test]
    fn test_register_request_invalid_short_username() {
        let mut request = valid_register_request();
        request.username = "abc".to_string();
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_register_request_invalid_email() {
        let mut request = valid_register_request();
        request.email = "not-an-email".to_string();
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_register_request_invalid_inviter_code_wrong_length() {
        let mut request = valid_register_request();
        request.inviter_code = "ABC".to_string();
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_register_request_invalid_email_verify_code_wrong_length() {
        let mut request = valid_register_request();
        request.email_verify_code = "12345".to_string();
        assert!(request.validate().is_err());
    }

    // ==================== SendEmailCodeRequest validation ====================

    #[test]
    fn test_send_email_code_request_valid() {
        let request = SendEmailCodeRequest {
            email: "user@example.com".to_string(),
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_send_email_code_request_invalid_email() {
        let request = SendEmailCodeRequest {
            email: "invalid-email".to_string(),
        };
        assert!(request.validate().is_err());
    }

    // ==================== LoginResponse serialization ====================

    #[test]
    fn test_login_response_serializes_to_camel_case() {
        use chrono::TimeZone;

        let response = LoginResponse {
            user: crate::user::UserInfo {
                id: "user1".to_string(),
                username: "alice".to_string(),
                nickname: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                avatar_token: None,
                created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            },
            access_token: "tok123".to_string(),
            access_token_expire_at: Utc.with_ymd_and_hms(2026, 6, 13, 12, 0, 0).unwrap(),
            refresh_token: "ref456".to_string(),
            refresh_token_expire_at: Utc.with_ymd_and_hms(2026, 7, 13, 12, 0, 0).unwrap(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"accessToken\""));
        assert!(json.contains("\"accessTokenExpireAt\""));
        assert!(!json.contains("access_token"));
    }
}
