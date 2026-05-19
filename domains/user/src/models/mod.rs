use chrono::{DateTime, Utc};
use common::models::ImageToken;
use common::utils::validators::validate_normal_char;
use common::utils::validators::validate_password;
use common::utils::TokenCipher;
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// 修改密码请求体
#[derive(Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    #[validate(custom(function = "validate_password"))]
    pub old_password: String,
    #[validate(custom(function = "validate_password"))]
    pub new_password: String
}

/// 修改昵称请求体
#[derive(Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ChangeNicknameRequest {
    #[validate(
        length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"),
        custom(function = "validate_normal_char")
    )]
    pub new_nickname: String
}

/// 批量获取用户信息请求体
#[derive(Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct GetUserInfoBatchRequest {
    pub user_ids: Vec<String>
}

/// 邀请码数据传输对象，包含邀请码值和过期时间
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviterCodeDTO {
    pub inviter_code: String,
    pub expire_at: DateTime<Utc>,
}

/// 用户信息数据库查询结果，直接映射数据库字段
#[derive(Serialize, FromQueryResult, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoDTO {
    pub user_id: i64,
    pub nickname: String,
    pub avatar_file_id: Option<String>,
}

/// 用户信息视图对象，用于 API 响应，头像字段已加密为 token
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoVO {
    pub user_id: String,
    pub nickname: String,
    pub avatar_token: Option<String>,
}

impl UserInfoVO {
    /// 将数据库 DTO 转换为视图对象，对头像文件 ID 进行加密
    ///
    /// # 参数
    /// - `dto`: 用户信息数据库查询结果
    /// - `token_cipher`: 用于加密头像文件 ID 的加密器
    ///
    /// # 返回
    /// 转换后的用户信息视图对象，`user_id` 转为字符串，头像字段加密为 token
    pub fn from_dto(dto: UserInfoDTO, token_cipher: &TokenCipher) -> Self {
        let avatar_token = dto.avatar_file_id
            .as_ref()
            .and_then(|key| token_cipher.encrypt(&ImageToken::thumbnail(key.clone()), Some(key)).ok());
        
        Self {
            user_id: dto.user_id.to_string(),
            nickname: dto.nickname,
            avatar_token,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::utils::TokenCipher;
    use validator::Validate;

    // 创建用于测试的 TokenCipher 实例
    fn create_test_cipher() -> TokenCipher {
        TokenCipher::new("test-secret-key-32bytes!xxxxxx", "test-salt")
    }

    #[test]
    fn test_user_info_vo_from_dto_with_avatar() {
        let cipher = create_test_cipher();
        let dto = UserInfoDTO {
            user_id: 42,
            nickname: "Alice".to_string(),
            avatar_file_id: Some("file123".to_string()),
        };
        let vo = UserInfoVO::from_dto(dto, &cipher);
        assert_eq!(vo.user_id, "42");
        assert_eq!(vo.nickname, "Alice");
        assert!(vo.avatar_token.is_some());
    }

    #[test]
    fn test_user_info_vo_from_dto_without_avatar() {
        let cipher = create_test_cipher();
        let dto = UserInfoDTO {
            user_id: 1,
            nickname: "Bob".to_string(),
            avatar_file_id: None,
        };
        let vo = UserInfoVO::from_dto(dto, &cipher);
        assert_eq!(vo.user_id, "1");
        assert_eq!(vo.nickname, "Bob");
        assert!(vo.avatar_token.is_none());
    }

    #[test]
    fn test_change_nickname_request_valid() {
        let req = ChangeNicknameRequest {
            new_nickname: "Alice".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_nickname_request_empty() {
        let req = ChangeNicknameRequest {
            new_nickname: "".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_nickname_request_too_long() {
        let req = ChangeNicknameRequest {
            new_nickname: "a".repeat(21),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_nickname_request_special_chars() {
        let req = ChangeNicknameRequest {
            new_nickname: "test<script>".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_password_request_valid() {
        let req = ChangePasswordRequest {
            old_password: "oldPass123".to_string(),
            new_password: "newPass456".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_password_request_no_number() {
        let req = ChangePasswordRequest {
            old_password: "oldPassword".to_string(),
            new_password: "onlyLetters".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_password_request_too_short() {
        let req = ChangePasswordRequest {
            old_password: "oldPass123".to_string(),
            new_password: "a1".to_string(),
        };
        assert!(req.validate().is_err());
    }
}
