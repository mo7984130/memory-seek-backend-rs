// ============================================================
// UserId
// ============================================================

crate::id_type!(UserId, "user/");

/// 已通过管理员校验的用户身份
///
/// 由 [`AdminId::new`] 构造，只有管理员才能取得。
/// 作为 service 层参数，内部通过 [`AdminId::into_inner`] 展开为 [`UserId`] 使用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display)]
#[display("{}", _0)]
pub struct AdminId(UserId);

impl AdminId {
    pub const ADMIN_ID: AdminId = AdminId(UserId(1));

    /// 判断该身份是否为系统管理员.
    pub fn is_admin(&self) -> bool {
        *self == Self::ADMIN_ID
    }

    /// 展开为内部 [`UserId`]
    pub fn into_inner(self) -> UserId {
        self.0
    }

    /// 校验管理员权限，非管理员返回 403
    #[cfg(feature = "orm")]
    pub fn new(user_id: UserId) -> common::Result<Self> {
        let this = Self(user_id);
        if this.is_admin() {
            Ok(this)
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
    use crate::{photo::ImageToken, user::models::UserInfo};

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

    impl From<UserRecord> for UserInfo {
        fn from(user: UserRecord) -> Self {
            UserInfo {
                id: user.id,
                username: user.username,
                nickname: user.nickname,
                email: user.email,
                avatar_token: user.avatar_file_id,
                created_at: user.created_at,
            }
        }
    }

    impl UserInfo {
        /// 为头像文件 ID 生成加密访问令牌.
        pub fn with_avatar_token(mut self) -> common::error::contextual::Result<Self> {
            self.avatar_token = self
                .avatar_token
                .as_deref()
                .map(|key| ImageToken::encrypt_avatar_token(key, self.id))
                .transpose()?;
            Ok(self)
        }

        /// 将用户记录转换为包含头像访问令牌的用户信息.
        pub fn from_with_token(user: UserRecord) -> common::error::contextual::Result<Self> {
            UserInfo::from(user).with_avatar_token()
        }
    }
}

#[cfg(feature = "orm")]
pub use entity::*;
