use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct UserId(pub i64);

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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDTO {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub email: String,
    pub avatar_token: Option<String>,
    pub created_at: DateTimeUtc,
    pub refresh_token: Option<String>,
    pub refresh_token_expire_at: Option<DateTimeUtc>,
    pub access_token: Option<String>,
    pub access_token_expire_at: Option<DateTimeUtc>,
}

impl UserDTO {
    pub fn from_user(avatar_token: Option<String>, user: Model) -> Self {
        Self {
            id: user.id.to_string(),
            username: user.username,
            nickname: user.nickname,
            email: user.email,
            avatar_token,
            created_at: user.created_at,
            refresh_token: None,
            refresh_token_expire_at: None,
            access_token: None,
            access_token_expire_at: None,
        }
    }

    /// 设置访问令牌信息并返回自身（Builder 模式）
    ///
    /// # 参数
    /// - `token`: JWT 访问令牌
    /// - `expire`: 令牌过期时间
    ///
    /// # 返回
    /// 附加了访问令牌信息的 `UserDTO`
    pub fn with_access_token(mut self, token: String, expire: DateTimeUtc) -> Self {
        self.access_token = Some(token);
        self.access_token_expire_at = Some(expire);
        self
    }

    /// 设置刷新令牌信息并返回自身（Builder 模式）
    ///
    /// # 参数
    /// - `token`: JWT 刷新令牌
    /// - `expire`: 令牌过期时间
    ///
    /// # 返回
    /// 附加了刷新令牌信息的 `UserDTO`
    pub fn with_refresh_token(mut self, token: String, expire: DateTimeUtc) -> Self {
        self.refresh_token = Some(token);
        self.refresh_token_expire_at = Some(expire);
        self
    }
}
