use common_core::{DbConn, error::contextual::Result, ext::ToOk, time::now};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use types_identity::auth::user::{ActiveModel, Column, Entity, UserId, UserRecord};

use crate::error_ext::UserOptionExt;
use crate::models::{AdminUserRow, UserBriefRow};

/// 用户表数据访问，向 repo 层提供统一的数据库访问入口
pub struct UserMapper;

// 修改
impl UserMapper {
    /// 按 ID 倒序分页查询用户。
    pub(crate) async fn query_admin_users(
        db: &impl DbConn,
        cursor: Option<UserId>,
        limit: u64,
    ) -> Result<Vec<AdminUserRow>> {
        let mut query = Entity::find()
            .select_only()
            .columns([
                Column::Id,
                Column::Username,
                Column::Email,
                Column::Nickname,
                Column::AvatarFileId,
                Column::CreatedAt,
            ])
            .order_by_desc(Column::Id);

        if let Some(cursor) = cursor {
            query = query.filter(Column::Id.lt(cursor));
        }

        query
            .limit(limit)
            .into_model::<AdminUserRow>()
            .all(db)
            .await
            .map_err(Into::into)
    }

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

    /// 仅当密码仍等于 `expected_hash` 时才更新。
    ///
    /// 用旧密码哈希做 compare-and-set: 并发的两个改密请求都通过「旧密码校验」时,
    /// 只有第一个能真正写入, 第二个 `rows_affected == 0`。
    pub async fn update_password_if_unchanged(
        db: &impl DbConn,
        user_id: UserId,
        expected_hash: &str,
        new_password_hash: String,
    ) -> Result<bool> {
        let result = Entity::update_many()
            .col_expr(Column::Password, Expr::value(new_password_hash))
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .filter(Column::Id.eq(user_id))
            .filter(Column::Password.eq(expected_hash))
            .exec(db)
            .await?;

        Ok(result.rows_affected == 1)
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
