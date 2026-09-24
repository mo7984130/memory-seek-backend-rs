use std::collections::HashSet;

use common_core::ext::ToOk;
use common_core::{
    DbConn as ConnectionTrait,
    error::contextual::Result,
    time::{DateTime, now},
    types::CursorPage,
};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use types::{auth::user::UserId, visual::visual::VisualId};
use types_core::cursor::TimeIdCursor;
use types_visual::visual_like::*;

pub struct VisualLikeMapper;

// 创建
impl VisualLikeMapper {
    /// 插入影像点赞记录; 重复记录由数据库约束处理.
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<bool> {
        let current_time = now();
        let active_model = ActiveModel {
            visual_id: Set(visual_id),
            user_id: Set(user_id),
            created_at: Set(current_time),
            ..Default::default()
        };

        let rows_affected = Entity::insert(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([Column::VisualId, Column::UserId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;

        Ok(rows_affected > 0)
    }
}

// 查询
impl VisualLikeMapper {
    /// 批量查询用户对一组影像的点赞状态
    /// 查询用户对一批影像的点赞状态.
    pub async fn query_is_like_by_visual_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        visual_ids: &[VisualId],
    ) -> Result<HashSet<VisualId>> {
        Entity::find()
            .select_only()
            .column(Column::VisualId)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::VisualId.is_in(visual_ids.iter().copied()))
            .into_tuple::<VisualId>()
            .all(db)
            .await?
            .into_iter()
            .collect::<HashSet<VisualId>>()
            .to_ok()
    }

    /// 查询用户点赞的影像ID和点赞时间列表（带游标分页）
    ///
    /// 返回 `(VisualId, DateTime)` 元组，其中 DateTime 为点赞时间。
    /// 分页查询用户点赞过的影像 ID, 并返回点赞时间游标.
    pub async fn query_user_liked_visual_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        cursor: &Option<TimeIdCursor<VisualId>>,
        size: u64,
    ) -> Result<CursorPage<(VisualId, DateTime), TimeIdCursor<VisualId>>> {
        let mut query = Entity::find()
            .select_only()
            .column(Column::VisualId)
            .column(Column::CreatedAt)
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::VisualId);

        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::VisualId));
        }

        let records = query
            .limit(size + 1)
            .into_tuple::<(VisualId, DateTime)>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size)
            .with_next_cursor(|&(id, time_at)| TimeIdCursor { time_at, id }))
    }
}

// 删除
impl VisualLikeMapper {
    /// 删除用户对指定影像的点赞记录.
    pub async fn delete(
        db: &impl ConnectionTrait,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<bool> {
        let res = Entity::delete_many()
            .filter(Column::VisualId.eq(visual_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(res.rows_affected > 0)
    }

    /// 删除指定影像的全部点赞记录.
    pub async fn delete_all_by_visual_ids(
        db: &impl ConnectionTrait,
        visual_ids: &[VisualId],
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::VisualId.is_in(visual_ids.iter().copied()))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
