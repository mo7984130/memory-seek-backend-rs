use chrono::{DateTime, Utc};
use common::utils::validators::validate_account;
use common::utils::validators::validate_email;
use common::utils::validators::validate_normal_char;
use common::utils::validators::validate_password;
use common::utils::validators::validate_username;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    #[validate(custom(function = "validate_account"))]
    pub account: String,
    #[validate(custom(function = "validate_password"))]
    pub password: String
}

#[derive(Debug, Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
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

#[derive(Debug, Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailCodeRequest {
    #[validate(custom(function = "validate_email"))]
    pub email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub access_token_expire_at: DateTime<Utc>,
}
