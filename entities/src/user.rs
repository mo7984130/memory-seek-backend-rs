use sea_orm::entity::prelude::*;
use serde::Serialize;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "auth_user")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u32,
    #[sea_orm(unique)]
    pub username: String,
    pub email: String,
    pub password: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
    pub inviter: i64,
    pub refresh_token: Option<String>,
    pub refresh_token_expire_at: Option<DateTimeWithTimeZone>,
    pub updated_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct UserDTO {
    pub id: String,          // 数据库 ID
    pub username: String,    // 登录名
    pub nickname: String, // 显示昵称
    pub email: String,       // 邮箱
    pub avatar_url: Option<String>,     // 头像Url
    pub created_at: DateTime<Utc>, // 创建时间
    pub refresh_token: Option<String>, // 刷新令牌
    pub refresh_token_expire_at: Option<DateTime<Utc>>, // 刷新令牌过期时间
    pub access_token: Option<String>, // 访问令牌
    pub access_token_expire_at: Option<DateTime<Utc>>, // 访问令牌过期时间
}
impl From<Model> for UserDTO {
    fn from(user: Model) -> Self {
        Self {
            id: user.id.to_string(),
            username: user.username,
            nickname: user.nickname,
            email: user.email,
            avatar_url: user.avatar_url,
            created_at: user.created_at.into(),
            refresh_token: user.refresh_token,
            refresh_token_expire_at: user.refresh_token_expire_at.map(|dt| dt.with_timezone(&Utc)),
            access_token: None,
            access_token_expire_at: None,
        }
    }
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