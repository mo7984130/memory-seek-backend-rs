use common_core::{DbConn, error::contextual::Result, ext::ToOk};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, Set,
};
use types_identity::auth::user::{ActiveModel, Column, Entity, UserId, UserRecord};

use crate::error_ext::ContextualErrorExt;
use crate::models::UserBriefRow;

/// 用户表数据访问，向 repo 层提供统一的数据库访问入口
pub struct UserMapper;

// 修改
impl UserMapper {
    /// 修改用户昵称
    pub async fn update_nickname(
        db: &impl DbConn,
        user_id: UserId,
        new_nickname: &str,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::Nickname, Expr::value(new_nickname))
            .filter(Column::Id.eq(user_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// 在事务中锁定用户行并更新头像，返回旧头像 key（由调用方决定是否删除旧文件）
    pub async fn update_avatar(
        txn: &DatabaseTransaction,
        user_id: UserId,
        new_key: String,
    ) -> Result<Option<String>> {
        // 注意: 不能用 `into_values::<Option<String>, Column>()` —— 它按 `Column`
        // 全量枚举的列名取 `cols[0]`(即 "id"), 而本查询只 select 了
        // avatar_file_id; 且 `Option` 的 `TryGetable` 对“列不存在”会返回
        // `Ok(None)`, 导致旧头像 key 恒为 None、旧对象永不删除。
        // 这里改用按列序号取值的 `into_tuple`。
        let old_key: Option<String> = Entity::find_by_id(user_id)
            .select_only()
            .column(Column::AvatarFileId)
            .lock_exclusive()
            .into_tuple::<Option<String>>()
            .one(txn)
            .await?
            .user_not_found()?;

        ActiveModel {
            id: Set(user_id),
            avatar_file_id: Set(Some(new_key)),
            ..Default::default()
        }
        .update(txn)
        .await?;

        Ok(old_key)
    }

    /// 更新用户密码哈希
    pub async fn update_password(
        db: &impl DbConn,
        user_id: UserId,
        new_password_hash: String,
    ) -> Result<()> {
        ActiveModel {
            id: Set(user_id),
            password: Set(new_password_hash),
            ..Default::default()
        }
        .update(db)
        .await?;

        Ok(())
    }

    /// 登出时清除 refresh_token 与过期时间
    pub async fn clear_refresh_token(db: &impl DbConn, user_id: UserId) -> Result<()> {
        ActiveModel {
            id: Set(user_id),
            refresh_token: Set(None),
            refresh_token_expire_at: Set(None),
            ..Default::default()
        }
        .update(db)
        .await?;

        Ok(())
    }
}

// 查询
impl UserMapper {
    /// 按 ID 查询完整用户记录
    pub async fn query_by_id(db: &impl DbConn, user_id: UserId) -> Result<Option<UserRecord>> {
        Entity::find_by_id(user_id)
            .one(db)
            .await?
            .map(UserRecord::from)
            .to_ok()
    }

    /// 批量查询用户基本信息（未找到的用户不包含在结果中）
    pub async fn query_info_rows(
        db: &impl DbConn,
        user_ids: &[UserId],
    ) -> Result<Vec<UserBriefRow>> {
        Entity::find()
            .filter(Column::Id.is_in(user_ids.iter().copied()))
            .select_only()
            .column_as(Column::Id, "user_id")
            .column(Column::Nickname)
            .column(Column::AvatarFileId)
            .into_model::<UserBriefRow>()
            .all(db)
            .await?
            .to_ok()
    }

    /// 查询用户密码哈希
    pub async fn query_password_hash(db: &impl DbConn, user_id: UserId) -> Result<String> {
        Entity::find_by_id(user_id)
            .select_only()
            .column(Column::Password)
            .into_tuple()
            .one(db)
            .await?
            .user_not_found()
    }
}

// 创建
impl UserMapper {}

// 删除
impl UserMapper {}
