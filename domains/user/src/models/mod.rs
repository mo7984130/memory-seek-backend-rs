use common::Result;
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use types::auth::user::UserId;
use types::photo::ImageToken;
use types::user::UserBriefView;

/// 用户信息数据库查询结果（后端内部使用）
#[derive(Serialize, FromQueryResult, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoRow {
    pub user_id: UserId,
    pub nickname: String,
    pub avatar_file_id: Option<String>,
}

impl UserInfoRow {
    /// 转换为 API 响应类型，对头像文件 ID 进行加密（内嵌浏览者身份）
    pub fn into_brief_view(self, viewer: UserId) -> Result<UserBriefView> {
        let avatar_token = self
            .avatar_file_id
            .as_deref()
            .map(|key| ImageToken::encrypt_avatar_token(key, viewer))
            .transpose()?;

        Ok(UserBriefView {
            user_id: self.user_id,
            nickname: self.nickname,
            avatar_token,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::utils::{TokenCipherConfig, init_token_cipher};

    fn init_test_cipher() {
        init_token_cipher(&TokenCipherConfig {
            key: "test-secret-key-32bytes!xxxxxx".to_owned(),
            salt: "test-salt".to_owned(),
        });
    }

    #[test]
    fn test_from_dto_with_avatar() {
        init_test_cipher();
        let dto = UserInfoRow {
            user_id: UserId(42),
            nickname: "Alice".to_string(),
            avatar_file_id: Some("file123".to_string()),
        };
        let vo = dto.into_brief_view(UserId(1)).unwrap();
        assert_eq!(vo.user_id, UserId(42));
        assert_eq!(vo.nickname, "Alice");
        assert!(vo.avatar_token.is_some());
    }

    #[test]
    fn test_from_dto_without_avatar() {
        init_test_cipher();
        let dto = UserInfoRow {
            user_id: UserId(1),
            nickname: "Bob".to_string(),
            avatar_file_id: None,
        };
        let vo = dto.into_brief_view(UserId(2)).unwrap();
        assert_eq!(vo.user_id, UserId(1));
        assert_eq!(vo.nickname, "Bob");
        assert!(vo.avatar_token.is_none());
    }
}
