use std::fmt;

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct UserId(pub i64);

impl From<i64> for UserId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "auth_user")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub username: String,
    pub email: String,
    pub password: String,
    pub nickname: String,
    pub avatar_file_id: Option<String>,
    pub inviter: i64,
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
    pub inviter: i64,
    pub refresh_token: Option<String>,
    pub refresh_token_expire_at: Option<DateTimeUtc>,
    pub updated_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
}

impl From<Model> for UserRecord {
    fn from(model: Model) -> Self {
        Self {
            id: UserId(model.id),
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
pub fn create_user_info(user: &UserRecord) -> memory_seek_type::user::UserInfo {
    memory_seek_type::user::UserInfo {
        id: user.id.to_string(),
        username: user.username.clone(),
        nickname: user.nickname.clone(),
        email: user.email.clone(),
        avatar_token: user.avatar_file_id.clone(),
        created_at: user.created_at,
    }
}
