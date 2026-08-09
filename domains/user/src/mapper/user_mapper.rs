use common::{Result, ext::ToOk};
use sea_orm::{ConnectionTrait, EntityTrait};
use types::auth::user::{Entity, UserId, UserRecord};

pub struct UserMapper;

// 创建
impl UserMapper {}

// 修改
impl UserMapper {}

// 查询
impl UserMapper {
    pub async fn query(db: &impl ConnectionTrait, user_id: UserId) -> Result<Option<UserRecord>> {
        Entity::find_by_id(user_id)
            .one(db)
            .await?
            .map(UserRecord::from)
            .to_ok()
    }
}

// 删除
impl UserMapper {}
