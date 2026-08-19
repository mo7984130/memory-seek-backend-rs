//! 用户相关类型定义

use common::DateTime;
use validator::Validate;

use super::validators::*;
use crate::auth::user::UserId;

// ============================================================
// UserIds — 校验型用户 ID 批量列表
// ============================================================

crate::validated_newtype!(
    UserIds,
    Vec<UserId>,
    1024,
    "user/",
    "用户ID列表不能为空",
    "用户数量不能超过1024"
);

crate::out_dto!(UserInfo, "user/", Debug; {
    /// 用户ID
    pub id: UserId,

    /// 用户名
    pub username: String,

    /// 昵称
    pub nickname: String,

    /// 邮箱
    pub email: String,

    /// 头像令牌
    pub avatar_token: Option<String>,

    /// 创建时间
    pub created_at: DateTime,
});

crate::in_dto!(ChangePasswordParam, "user/", serialize; {
    #[validate(custom(function = "validate_password"))]
    pub old_password: String,

    #[validate(
        custom(function = "validate_password"),
        must_match(other = "confirm_password")
    )]
    pub new_password: String,

    pub confirm_password: String,
});

crate::in_dto!(ChangeNicknameParam, "user/", serialize; {
    #[validate(
        length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"),
        custom(function = "validate_normal_char")
    )]
    pub new_nickname: String,
});

crate::in_dto!(GetUserInfoBatchParam, "user/", serialize; {
    #[validate(nested)]
    pub user_ids: UserIds,
});

crate::out_dto!(InviterCodeView, "user/", rename = "InviterCode"; {
    pub inviter_code: String,
    pub expire_at: DateTime,
});

crate::in_dto!(UpdateAvatarParam, "user/", serialize, docs = "更新头像请求参数（文件二进制数据由 multipart 单独传递）"; {
    /// 文件名
    #[validate(length(min = 1, max = 255, message = "文件名不能为空"))]
    pub file_name: String,

    /// 文件 MIME 类型
    #[validate(length(min = 1, max = 100, message = "文件类型不能为空"))]
    pub content_type: String,
});

crate::out_dto!(UserBriefView, "user/", rename = "UserBrief"; {
    pub user_id: UserId,
    pub nickname: String,
    pub avatar_token: Option<String>,
});

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_user_ids_new_valid() {
        let ids = UserIds::new(vec![UserId(1), UserId(2)]);
        assert!(ids.is_ok());
    }

    #[test]
    fn test_user_ids_new_empty() {
        let ids = UserIds::new(vec![]);
        assert!(ids.is_err());
    }

    #[test]
    fn test_user_ids_new_too_many() {
        let ids = UserIds::new((0..1025).map(UserId).collect());
        assert!(ids.is_err());
    }

    #[test]
    fn test_user_ids_new_exact_max() {
        let ids = UserIds::new((0..1024).map(UserId).collect());
        assert!(ids.is_ok());
    }

    #[test]
    fn test_user_info_serializes_to_camel_case() {
        let user = UserInfo {
            id: UserId(123),
            username: "testuser".to_string(),
            nickname: "Test User".to_string(),
            email: "test@example.com".to_string(),
            avatar_token: None,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("\"avatarToken\""));
        assert!(json.contains("\"createdAt\""));
    }

    #[test]
    fn test_user_info_clone() {
        let user = UserInfo {
            id: UserId(123),
            username: "testuser".to_string(),
            nickname: "Test User".to_string(),
            email: "test@example.com".to_string(),
            avatar_token: Some("token123".to_string()),
            created_at: Utc::now(),
        };
        let cloned = user.clone();
        assert_eq!(user.id, cloned.id);
        assert_eq!(user.username, cloned.username);
    }

    #[test]
    fn test_change_password_param_valid() {
        let req = ChangePasswordParam {
            old_password: "oldPass123".to_string(),
            new_password: "newPass456".to_string(),
            confirm_password: "newPass456".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_password_param_short() {
        let req = ChangePasswordParam {
            old_password: "oldPass123".to_string(),
            new_password: "a1".to_string(),
            confirm_password: "a1".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_password_param_mismatch() {
        let req = ChangePasswordParam {
            old_password: "oldPass123".to_string(),
            new_password: "newPass456".to_string(),
            confirm_password: "newPass789".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_nickname_param_valid() {
        let req = ChangeNicknameParam {
            new_nickname: "Alice".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_nickname_param_empty() {
        let req = ChangeNicknameParam {
            new_nickname: "".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_nickname_param_special_chars() {
        let req = ChangeNicknameParam {
            new_nickname: "test<script>".to_string(),
        };
        assert!(req.validate().is_err());
    }
}
