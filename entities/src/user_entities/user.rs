use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

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
    pub refresh_token_expire_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct UserDTO {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub email: String,
    pub avatar_token: Option<String>,
    pub created_at: DateTime<Utc>,
    pub refresh_token: Option<String>,
    pub refresh_token_expire_at: Option<DateTime<Utc>>,
    pub access_token: Option<String>,
    pub access_token_expire_at: Option<DateTime<Utc>>,
}

impl UserDTO {
    pub fn with_access_token(mut self, token: String, expire: DateTime<Utc>) -> Self {
        self.access_token = Some(token);
        self.access_token_expire_at = Some(expire);
        self
    }

    pub fn with_refresh_token(mut self, token: String, expire: DateTime<Utc>) -> Self {
        self.refresh_token = Some(token);
        self.refresh_token_expire_at = Some(expire);
        self
    }
}
