// ============================================================
// UserId
// ============================================================

pub use types_core::UserId;

/// 管理员身份标识定义在共享内核 `types-core`(身份、审计、备份、视觉上下文共用),
/// 此处重导出以保持 `types_identity::auth::user::AdminId` 路径不变。
pub use types_core::AdminId;

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::user::models::UserInfo;
    use types_token::VisualToken;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "auth_user")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: UserId,

        /// 用户名
        #[sea_orm(column_type = "String(StringLen::N(255))", unique)]
        pub username: String,

        /// 邮箱地址
        #[sea_orm(column_type = "String(StringLen::N(255))", unique)]
        pub email: String,

        /// 加密后的密码
        #[sea_orm(column_type = "String(StringLen::N(255))")]
        pub password: String,

        /// 用户昵称
        #[sea_orm(column_type = "String(StringLen::N(255))")]
        pub nickname: String,

        /// 头像 文件ID
        #[sea_orm(column_type = "String(StringLen::N(1023))", nullable)]
        pub avatar_file_id: Option<String>,

        /// 邀请人ID
        pub inviter: UserId,

        /// 刷新令牌
        #[sea_orm(column_type = "String(StringLen::N(32))", nullable)]
        pub refresh_token: Option<String>,
        /// 刷新令牌过期时间
        pub refresh_token_expire_at: Option<DateTime>,

        /// 更新时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub updated_at: DateTime,
        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
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
        pub refresh_token_expire_at: Option<DateTime>,
        pub updated_at: DateTime,
        pub created_at: DateTime,
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
                avatar_token: None,
                created_at: user.created_at,
            }
        }
    }

    impl UserInfo {
        /// 为头像文件 ID 生成加密访问令牌.
        pub fn with_avatar_token(&mut self, file_id: String) {
            self.avatar_token = Some(VisualToken::image_thumbnail(self.id, file_id).into());
        }

        /// 将用户记录转换为包含头像访问令牌的用户信息.
        pub fn from_with_token(user: UserRecord) -> Self {
            let file_id = user.avatar_file_id.clone();
            let mut this = UserInfo::from(user);
            if let Some(file_id) = file_id {
                this.with_avatar_token(file_id);
            }
            this
        }
    }
}

#[cfg(feature = "orm")]
pub use entity::*;
