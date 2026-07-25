//! 用户相关类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use validator::Validate;

use super::validators::*;

/// 用户信息（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    /// 用户ID
    pub id: String,

    /// 用户名
    pub username: String,

    /// 昵称
    pub nickname: String,

    /// 邮箱
    pub email: String,

    /// 头像令牌
    pub avatar_token: Option<String>,

    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 用户详情响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GetUserResponse {
    /// 用户信息
    pub user: UserInfo,
}

/// 更新用户资料请求
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    /// 昵称
    pub nickname: Option<String>,

    /// 头像令牌
    pub avatar_token: Option<String>,
}

/// 更新用户资料响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserResponse {
    /// 用户信息
    pub user: UserInfo,
}

/// 修改密码请求
#[derive(Debug, Serialize, Deserialize, Validate, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordParam {
    #[validate(custom(function = "validate_password"))]
    pub old_password: String,

    #[validate(custom(function = "validate_password"))]
    pub new_password: String,
}

/// 修改昵称请求
#[derive(Debug, Serialize, Deserialize, Validate, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ChangeNicknameParam {
    #[validate(
        length(min = 1, max = 20, message = "昵称长度在 1 到 20 个字符"),
        custom(function = "validate_normal_char")
    )]
    pub new_nickname: String,
}

/// 批量获取用户信息请求
#[derive(Debug, Serialize, Deserialize, Validate, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GetUserInfoBatchParam {
    pub user_ids: Vec<String>,
}

/// 邀请码响应
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct InviterCodeResult {
    pub inviter_code: String,
    pub expire_at: DateTime<Utc>,
}

/// 更新头像请求参数（文件二进制数据由 multipart 单独传递）
#[derive(Debug, Validate, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvatarParam {
    /// 文件名
    #[validate(length(min = 1, max = 255, message = "文件名不能为空"))]
    pub file_name: String,

    /// 文件 MIME 类型
    #[validate(length(min = 1, max = 100, message = "文件类型不能为空"))]
    pub content_type: String,
}

/// 用户信息响应（批量查询返回）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoResult {
    pub user_id: String,
    pub nickname: String,
    pub avatar_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_info_serializes_to_camel_case() {
        let user = UserInfo {
            id: "123".to_string(),
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
            id: "123".to_string(),
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
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_password_param_short() {
        let req = ChangePasswordParam {
            old_password: "oldPass123".to_string(),
            new_password: "a1".to_string(),
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
