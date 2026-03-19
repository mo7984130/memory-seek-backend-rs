use chrono::{DateTime, Utc};
use common::utils::validators::validate_password;
use common::utils::validators::validate_normal_char;
use img_url_generator::{encrypt_image_token, ImageToken};
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    #[validate(custom(function = "validate_password"))]
    pub old_password: String,
    #[validate(custom(function = "validate_password"))]
    pub new_password: String
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ChangeNicknameRequest {
    #[validate(
        length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"),
        custom(function = "validate_normal_char")
    )]
    pub new_nickname: String
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUserInfoBatchRequest {
    pub user_ids: Vec<String>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviterCodeDTO {
    pub inviter_code: String,
    pub expire_at: DateTime<Utc>,
}

#[derive(Serialize, FromQueryResult, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoDTO {
    pub user_id: i64,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoVO {
    pub user_id: String,
    pub nickname: String,
    pub avatar_token: Option<String>,
}

impl UserInfoVO {
    pub fn from_dto(dto: UserInfoDTO, encryption_key: &[u8; 32]) -> Self {
        let avatar_token = dto.avatar_url
            .as_ref()
            .and_then(|key| encrypt_image_token(&ImageToken::thumbnail(key.clone()), encryption_key).ok());
        
        Self {
            user_id: dto.user_id.to_string(),
            nickname: dto.nickname,
            avatar_token,
        }
    }
}
