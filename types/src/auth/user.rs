// ============================================================
// UserId
// ============================================================

crate::id_type!(UserId, "user/");

impl UserId {
    /// 管理员用户 ID
    pub const ADMIN_ID: i64 = 1;

    /// 是否为管理员
    pub fn is_admin(&self) -> bool {
        self.0 == Self::ADMIN_ID
    }
}

/// 已通过管理员校验的用户身份
///
/// 由 [`AdminId::new`] 构造，只有管理员才能取得。
/// 作为 service 层参数，内部通过 [`AdminId::into_inner`] 展开为 [`UserId`] 使用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display)]
#[display("{}", _0)]
pub struct AdminId(UserId);

impl AdminId {
    /// 展开为内部 [`UserId`]
    pub fn into_inner(self) -> UserId {
        self.0
    }

    /// 校验管理员权限，非管理员返回 403
    #[cfg(feature = "orm")]
    pub fn new(user_id: UserId) -> Result<Self, common::error::AppError> {
        if user_id.is_admin() {
            Ok(Self(user_id))
        } else {
            Err(common::error::AppError::forbidden("仅管理员可访问"))
        }
    }
}

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::user::models::UserInfo;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "auth_user")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: UserId,
        #[sea_orm(unique)]
        pub username: String,
        pub email: String,
        pub password: String,
        pub nickname: String,
        pub avatar_file_id: Option<String>,
        pub inviter: UserId,
        pub refresh_token: Option<String>,
        pub refresh_token_expire_at: Option<DateTimeUtc>,
        pub updated_at: DateTimeUtc,
        pub created_at: DateTimeUtc,
    }

    /// 用户记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct UserRecord {
        pub id: UserId,
        pub username: String,
        pub email: String,
        pub password: String,
        pub nickname: String,
        pub avatar_file_id: Option<String>,
        pub inviter: UserId,
        pub refresh_token: Option<String>,
        pub refresh_token_expire_at: Option<DateTimeUtc>,
        pub updated_at: DateTimeUtc,
        pub created_at: DateTimeUtc,
    }

    impl From<Model> for UserRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                username: model.username,
                email: model.email,
                password: model.password,
                nickname: model.nickname,
                avatar_file_id: model.avatar_file_id,
                inviter: model.inviter,
                refresh_token: model.refresh_token,
                refresh_token_expire_at: model.refresh_token_expire_at,
                updated_at: model.updated_at,
                created_at: model.created_at,
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    /// 从 UserRecord 创建 UserInfo
    pub fn create_user_info(user: &UserRecord) -> UserInfo {
        UserInfo {
            id: user.id,
            username: user.username.clone(),
            nickname: user.nickname.clone(),
            email: user.email.clone(),
            avatar_token: user.avatar_file_id.clone(),
            created_at: user.created_at,
        }
    }
}

#[cfg(feature = "orm")]
pub use entity::*;
