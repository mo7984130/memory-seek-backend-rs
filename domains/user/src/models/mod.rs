use common::utils::validators::validate_password;
use common::utils::validators::validate_normal_char;
use serde::{Deserialize};
use validator::Validate;

/**
*/
#[derive(Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(custom(function = "validate_password"))]
    pub old_password: String,
    #[validate(custom(function = "validate_password"))]
    pub new_password: String
}

#[derive(Deserialize, Validate)]
pub struct ChangeNicknameRequest {
    #[validate(
        length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"),
        custom(function = "validate_normal_char")
    )]
    pub new_nickname: String
}

#[derive(Deserialize)]
pub struct GetUserInfoBatchRequest {
    pub user_ids: Vec<u32>
}

