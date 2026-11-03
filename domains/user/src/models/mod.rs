use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use types_identity::auth::user::UserId;
use types_identity::user::UserBriefView;
use types_visual_token::{VisualToken, VisualTokenStr};

#[derive(Serialize, FromQueryResult, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBriefRow {
    pub user_id: UserId,
    pub nickname: String,
    pub avatar_file_id: Option<String>,
}

/// 管理员用户列表只选取可展示字段，不读取密码等凭据。
#[derive(FromQueryResult, Debug, Clone)]
pub(crate) struct AdminUserRow {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub nickname: String,
    pub avatar_file_id: Option<String>,
    pub created_at: common_core::time::DateTime,
}

impl AdminUserRow {
    /// 转为用户视图；头像令牌始终以用户自身 ID 为主体签发。
    pub(crate) fn into_user_info(
        self,
    ) -> common_core::ContextualResult<types_identity::user::UserInfo> {
        use types_identity::user::UserInfo;

        let avatar_token = self
            .avatar_file_id
            .map(|file_id| {
                types_visual_token::VisualTokenStr::encrypt(
                    &types_visual_token::VisualToken::image_thumbnail(self.id, file_id),
                )
            })
            .transpose()?;

        Ok(UserInfo {
            id: self.id,
            username: self.username,
            nickname: self.nickname,
            email: self.email,
            avatar_token,
            created_at: self.created_at,
        })
    }
}

impl UserBriefRow {
    /// 转换为用户摘要视图，头像 token 按浏览者签发
    pub fn into_brief_view(self, viewer: UserId) -> common_core::ContextualResult<UserBriefView> {
        let avatar_token = self
            .avatar_file_id
            .map(|file_id| VisualTokenStr::encrypt(&VisualToken::image_thumbnail(viewer, file_id)))
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
    use common_crypto::{TokenCipherConfig, init_token_cipher};

    fn init_test_cipher() {
        init_token_cipher(&TokenCipherConfig {
            key: "test-secret-key-32bytes!xxxxxx".to_owned(),
            salt: "test-salt".to_owned(),
        });
    }

    #[test]
    fn test_from_dto_with_avatar() {
        init_test_cipher();
        let dto = UserBriefRow {
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
        let dto = UserBriefRow {
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
