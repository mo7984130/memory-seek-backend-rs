use std::collections::HashSet;

use common::ext::ToOk;
use common::{
    error::contextual::Result,
    models::CursorPage,
    time::{DateTime, now},
};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use types::cursor::TimeIdCursor;
use types::photo::photo_like::*;
use types::{auth::user::UserId, photo::photo::PhotoId};

pub struct PhotoLikeMapper;

// 创建
impl PhotoLikeMapper {
    /// 插入照片点赞记录; 重复记录由数据库约束处理.
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<bool> {
        let current_time = now();
        let active_model = ActiveModel {
            photo_id: Set(photo_id),
            user_id: Set(user_id),
            created_at: Set(current_time),
            updated_at: Set(current_time),
            ..Default::default()
        };

        let rows_affected = Entity::insert(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([Column::PhotoId, Column::UserId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;

        Ok(rows_affected > 0)
    }
}

// 查询
impl PhotoLikeMapper {
    /// 批量查询用户对一组照片的点赞状态
    /// 查询用户对一批照片的点赞状态.
    pub async fn query_is_like_by_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<HashSet<PhotoId>> {
        Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .into_tuple::<PhotoId>()
            .all(db)
            .await?
            .into_iter()
            .collect::<HashSet<PhotoId>>()
            .to_ok()
    }

    /// 查询用户点赞的照片ID和点赞时间列表（带游标分页）
    ///
    /// 返回 `(PhotoId, DateTime)` 元组，其中 DateTime 为点赞时间。
    /// 分页查询用户点赞过的照片 ID, 并返回点赞时间游标.
    pub async fn query_user_liked_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        cursor: &Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<CursorPage<(PhotoId, DateTime), TimeIdCursor<PhotoId>>> {
        let mut query = Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .column(Column::CreatedAt)
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::PhotoId);

        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::PhotoId));
        }

        let records = query
            .limit(size + 1)
            .into_tuple::<(PhotoId, DateTime)>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size)
            .with_next_cursor(|&(id, time_at)| TimeIdCursor { time_at, id }))
    }
}

// 删除
impl PhotoLikeMapper {
    /// 删除用户对指定照片的点赞记录.
    pub async fn delete(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<bool> {
        let res = Entity::delete_many()
            .filter(Column::PhotoId.eq(photo_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(res.rows_affected > 0)
    }

    /// 删除指定照片的全部点赞记录.
    pub async fn delete_all_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
