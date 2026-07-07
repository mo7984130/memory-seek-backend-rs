use std::collections::HashSet;

use common::Result;
use common::ext::{OkExt, ResultErrExt};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use sea_orm::entity::prelude::DateTimeUtc;

use entities::photo::photo::*;

use crate::models::photo::{PageDirection, PhotoCursor};

pub(crate) struct PhotoMapper;

// 创建
impl PhotoMapper {}

// 修改
impl PhotoMapper {
    pub async fn update_comment_count_delta(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(
                Column::CommentCount,
                Expr::col(Column::CommentCount).add(delta),
            )
            .filter(Column::Id.eq(photo_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "更新照片评论总数数数据库错误")?;

        Ok(())
    }

    /// 更新照片点赞数（增量）
    pub async fn update_like_count_delta(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(
                Column::LikeCount,
                Expr::col(Column::LikeCount).add(delta),
            )
            .filter(Column::Id.eq(photo_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "更新照片点赞数错误")?;

        Ok(())
    }
}

// 查询
impl PhotoMapper {
    pub async fn exists(db: &impl ConnectionTrait, photo_id: PhotoId) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(photo_id.0))
            .count(db)
            .await
            .trace_internal_err("db_query_err", "查询照片是否存在失败")?;
        Ok(count > 0)
    }

    pub async fn exists_by_md5_batch(
        db: &impl ConnectionTrait,
        md5s: &[impl AsRef<str>],
    ) -> Result<HashSet<String>> {
        if md5s.is_empty() {
            return Ok(HashSet::new());
        }
        let existing = Entity::find()
            .filter(Column::Md5.is_in(md5s.iter().map(|s| s.as_ref())))
            .select_only()
            .column(Column::Md5)
            .into_tuple::<String>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "批量查询MD5失败")?;
        Ok(existing.into_iter().collect())
    }

    fn build_cursor_query(
        cursor: Option<&PhotoCursor>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTimeUtc>,
    ) -> sea_orm::Select<Entity> {
        let (order_by_desc, filter) = match direction {
            PageDirection::Next => (true, true),   // 倒序，向前翻
            PageDirection::Prev => (false, false), // 正序，向后翻
        };

        let mut query = if order_by_desc {
            Entity::find()
                .order_by_desc(Column::CreatedAt)
                .order_by_desc(Column::Id)
        } else {
            Entity::find()
                .order_by_asc(Column::CreatedAt)
                .order_by_asc(Column::Id)
        };

        query = query.limit(size);

        if let Some(c) = cursor {
            // 有游标时，按游标分页
            if filter {
                // Next: 倒序遍历，找比游标小的
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(Column::CreatedAt.lt(c.created_at))
                        .add(
                            sea_orm::Condition::all()
                                .add(Column::CreatedAt.eq(c.created_at))
                                .add(Column::Id.lt(c.id.0)),
                        ),
                );
            } else {
                // Prev: 正序遍历，找比游标大的
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(Column::CreatedAt.gt(c.created_at))
                        .add(
                            sea_orm::Condition::all()
                                .add(Column::CreatedAt.eq(c.created_at))
                                .add(Column::Id.gt(c.id.0)),
                        ),
                );
            }
        } else if let Some(anchor) = anchor_time {
            // 无游标但有锚点时间时，用锚点时间作为虚拟游标
            if filter {
                // Next (倒序): 找 created_at <= anchor 的照片
                query = query.filter(Column::CreatedAt.lte(anchor));
            } else {
                // Prev (正序): 找 created_at >= anchor 的照片
                query = query.filter(Column::CreatedAt.gte(anchor));
            }
        }

        query
    }

    pub async fn query_cursor_page_ids(
        db: &impl ConnectionTrait,
        cursor: Option<PhotoCursor>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTimeUtc>,
    ) -> Result<Vec<PhotoId>> {
        Self::build_cursor_query(cursor.as_ref(), size, direction, anchor_time)
            .select_only()
            .column(Column::Id)
            .into_tuple::<i64>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询 ID 列表失败")?
            .into_iter()
            .map(PhotoId::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn query_by_ids(
        db: &impl ConnectionTrait,
        ids: &[PhotoId],
    ) -> Result<Vec<PhotoRecord>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().map(|id| id.0)))
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询照片失败")
            .map(|models| models.into_iter().map(PhotoRecord::from).collect())
    }
}

// 删除
impl PhotoMapper {
    pub async fn delete_by_ids(db: &impl ConnectionTrait, ids: &[PhotoId]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        Entity::delete_many()
            .filter(Column::Id.is_in(ids.iter().map(|id| id.0)))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "删除照片失败")?;
        Ok(())
    }
}
