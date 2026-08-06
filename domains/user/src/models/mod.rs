use common::utils::TokenCipher;
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

/// 将数据库查询结果转换为 API 响应类型，对头像文件 ID 进行加密
pub fn user_brief_view_from_dto(dto: UserInfoRow, token_cipher: &TokenCipher) -> UserBriefView {
    let avatar_token = dto.avatar_file_id.as_ref().and_then(|key| {
        token_cipher
            .encrypt(&ImageToken::thumbnail(key.clone()), Some(key))
            .ok()
    });

    UserBriefView {
        user_id: dto.user_id,
        nickname: dto.nickname,
        avatar_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::utils::TokenCipher;

    fn create_test_cipher() -> TokenCipher {
        TokenCipher::new("test-secret-key-32bytes!xxxxxx", "test-salt")
    }

    #[test]
    fn test_from_dto_with_avatar() {
        let cipher = create_test_cipher();
        let dto = UserInfoRow {
            user_id: UserId(42),
            nickname: "Alice".to_string(),
            avatar_file_id: Some("file123".to_string()),
        };
        let vo = user_brief_view_from_dto(dto, &cipher);
        assert_eq!(vo.user_id, UserId(42));
        assert_eq!(vo.nickname, "Alice");
        assert!(vo.avatar_token.is_some());
    }

    #[test]
    fn test_from_dto_without_avatar() {
        let cipher = create_test_cipher();
        let dto = UserInfoRow {
            user_id: UserId(1),
            nickname: "Bob".to_string(),
            avatar_file_id: None,
        };
        let vo = user_brief_view_from_dto(dto, &cipher);
        assert_eq!(vo.user_id, UserId(1));
        assert_eq!(vo.nickname, "Bob");
        assert!(vo.avatar_token.is_none());
    }
}
