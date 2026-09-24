#[cfg(feature = "face")]
use std::collections::HashMap;
use std::collections::HashSet;

use common::error::contextual::ext::OptionExt;
use common::ext::ToOk;
use common::{
    DbConn as ConnectionTrait,
    error::{AppError, ContextualError, contextual::Result},
    time::DateTime,
    types::CursorPage,
};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
use types::visual::{dto::visual::PageDirection, visual::*};

pub(crate) struct VisualMapper;

// 创建
impl VisualMapper {}

// 修改
impl VisualMapper {
    /// 增量更新影像的评论数量.
    pub async fn update_comment_count_delta(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(
                Column::CommentCount,
                Expr::col(Column::CommentCount).add(delta),
            )
            .filter(Column::Id.eq(visual_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// 增量更新影像的点赞数量.
    pub async fn update_like_count_delta(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::LikeCount, Expr::col(Column::LikeCount).add(delta))
            .filter(Column::Id.eq(visual_id))
            .exec(db)
            .await?;

        Ok(())
    }
}

// 查询
impl VisualMapper {
    /// 检查影像是否存在.
    pub async fn exists(db: &impl ConnectionTrait, visual_id: VisualId) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(visual_id))
            .count(db)
            .await?;
        Ok(count > 0)
    }

    /// 确保影像存在
    pub async fn ensure_exist(db: &impl ConnectionTrait, visual_id: VisualId) -> Result<()> {
        if !Self::exists(db, visual_id).await? {
            return Err(ContextualError::warn_without_source(
                "visual_not_exist",
                "影像不存在",
                AppError::not_found("影像不存在"),
            ));
        }
        Ok(())
    }

    /// 批量检查哈希值是否存在.
    pub async fn exists_by_hash_batch(
        db: &impl ConnectionTrait,
        hashes: &[impl AsRef<str>],
    ) -> Result<HashSet<String>> {
        Entity::find()
            .filter(Column::Hash.is_in(hashes.iter().map(|s| s.as_ref())))
            .select_only()
            .column(Column::Hash)
            .into_tuple::<String>()
            .all(db)
            .await?
            .into_iter()
            .collect::<HashSet<_>>()
            .to_ok()
    }

    /// 检查哈希值是否存在.
    pub async fn exists_by_hash(db: &impl ConnectionTrait, hash: impl AsRef<str>) -> Result<bool> {
        let results = Self::exists_by_hash_batch(db, &[hash.as_ref()]).await?;
        Ok(!results.is_empty())
    }

    /// 构建影像游标查询, 并统一处理时间与 ID 的排序边界.
    fn build_cursor_query(
        cursor: Option<&TimeIdCursor<VisualId>>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTime>,
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

        // 分页契约: 查询 size+1 条, 多出的 1 条用于 has_more 判定并截断。
        query = query.limit(size + 1);

        if let Some(c) = cursor {
            // 有游标时，按游标分页
            if filter {
                // Next: 倒序遍历，找比游标小的
                query = query.filter(c.before(Column::CreatedAt, Column::Id));
            } else {
                // Prev: 正序遍历，找比游标大的
                query = query.filter(c.after(Column::CreatedAt, Column::Id));
            }
        } else if let Some(anchor) = anchor_time {
            // 无游标但有锚点时间时，用锚点时间作为虚拟游标
            if filter {
                // Next (倒序): 找 created_at <= anchor 的影像
                query = query.filter(Column::CreatedAt.lte(anchor));
            } else {
                // Prev (正序): 找 created_at >= anchor 的影像
                query = query.filter(Column::CreatedAt.gte(anchor));
            }
        }

        query
    }

    /// 游标查询影像id.
    pub async fn query_cursor_page_ids(
        db: &impl ConnectionTrait,
        cursor: Option<TimeIdCursor<VisualId>>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTime>,
    ) -> Result<CursorPage<VisualId, ()>> {
        let records = Self::build_cursor_query(cursor.as_ref(), size, direction, anchor_time)
            .select_only()
            .column(Column::Id)
            .into_tuple::<VisualId>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size))
    }

    /// 按影像id查询记录.
    pub async fn query_by_ids(
        db: &impl ConnectionTrait,
        ids: &[VisualId],
    ) -> Result<Vec<VisualRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .all(db)
            .await?
            .into_iter()
            .map(VisualRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    #[cfg(feature = "face")]
    /// 批量查询影像 ID 和 file_id.
    pub async fn query_id_and_file_id_by_ids(
        db: &impl ConnectionTrait,
        ids: impl IntoIterator<Item = &VisualId>,
    ) -> Result<HashMap<VisualId, String>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.into_iter().copied()))
            .select_only()
            .column(Column::Id)
            .column(Column::FileId)
            .into_tuple::<(VisualId, String)>()
            .all(db)
            .await?
            .into_iter()
            .collect::<HashMap<VisualId, String>>()
            .to_ok()
    }

    /// 根据影像id查询属于用户的影像
    pub async fn query_by_user_id_and_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        ids: &[VisualId],
    ) -> Result<Vec<VisualRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .filter(Column::UserId.eq(user_id))
            .all(db)
            .await?
            .into_iter()
            .map(VisualRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 根据 file_id 查询影像尺寸.
    pub async fn query_dimensions_by_file_id(
        db: &impl ConnectionTrait,
        file_id: &str,
    ) -> Result<Option<(u32, u32)>> {
        Entity::find()
            .select_only()
            .column(Column::Width)
            .column(Column::Height)
            .filter(Column::FileId.eq(file_id))
            .into_tuple::<(u32, u32)>()
            .one(db)
            .await?
            .to_ok()
    }

    /// 根据影像 id 查询 file_id.
    pub async fn query_file_id_by_id(db: &impl ConnectionTrait, id: VisualId) -> Result<String> {
        Entity::find()
            .select_only()
            .column(Column::FileId)
            .filter(Column::Id.eq(id))
            .into_tuple::<String>()
            .one(db)
            .await?
            .ok_or_error(
                "visual_file_id_not_exist",
                "影像file_id不存在",
                AppError::InternalServerError,
            )
    }

    /// 根据 file_id 查询视频时长(ms).
    pub async fn query_duration_by_file_id(
        db: &impl ConnectionTrait,
        file_id: &str,
    ) -> Result<Option<u64>> {
        Entity::find()
            .select_only()
            .column(Column::DurationMs)
            .filter(Column::FileId.eq(file_id))
            .into_tuple::<i64>()
            .one(db)
            .await?
            .map(|d| Ord::max(d, 0) as u64)
            .to_ok()
    }

    /// 根据 file_id 查询 id.
    pub async fn query_visual_id_by_file_id(
        db: &impl ConnectionTrait,
        file_id: &str,
    ) -> Result<Option<VisualId>> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::FileId.eq(file_id))
            .into_tuple::<VisualId>()
            .one(db)
            .await?
            .to_ok()
    }
}

// 删除
impl VisualMapper {
    /// 删除影像
    pub async fn delete_by_ids(db: &impl ConnectionTrait, ids: &[VisualId]) -> Result<()> {
        Entity::delete_many()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }
}
