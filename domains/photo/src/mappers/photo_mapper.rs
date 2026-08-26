#[cfg(feature = "face")]
use std::collections::HashMap;
use std::collections::HashSet;

use common::ext::{ContextOptionExt, OkExt};
use common::{
    error::{AppError, ContextualError, contextual::Result},
    models::CursorPage,
    time::DateTime,
};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
use types::photo::{dto::photo::PageDirection, photo::*};

pub(crate) struct PhotoMapper;

// 创建
impl PhotoMapper {}

// 修改
impl PhotoMapper {
    /// 增量更新照片的评论数量.
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
            .filter(Column::Id.eq(photo_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// 增量更新照片的点赞数量.
    pub async fn update_like_count_delta(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::LikeCount, Expr::col(Column::LikeCount).add(delta))
            .filter(Column::Id.eq(photo_id))
            .exec(db)
            .await?;

        Ok(())
    }
}

// 查询
impl PhotoMapper {
    /// 检查照片是否存在.
    pub async fn exists(db: &impl ConnectionTrait, photo_id: PhotoId) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(photo_id))
            .count(db)
            .await?;
        Ok(count > 0)
    }

    /// 确保照片存在
    pub async fn ensure_exist(db: &impl ConnectionTrait, photo_id: PhotoId) -> Result<()> {
        if !Self::exists(db, photo_id).await? {
            return Err(ContextualError::warn_without_source(
                "photo_not_exist",
                "照片不存在",
                AppError::not_found("照片不存在"),
            ));
        }
        Ok(())
    }

    /// 批量检查md5.
    pub async fn exists_by_md5_batch(
        db: &impl ConnectionTrait,
        md5s: &[impl AsRef<str>],
    ) -> Result<HashSet<String>> {
        Entity::find()
            .filter(Column::Md5.is_in(md5s.iter().map(|s| s.as_ref())))
            .select_only()
            .column(Column::Md5)
            .into_tuple::<String>()
            .all(db)
            .await?
            .into_iter()
            .collect::<HashSet<_>>()
            .to_ok()
    }

    /// 检查md5.
    pub async fn exists_by_md5(db: &impl ConnectionTrait, md5: impl AsRef<str>) -> Result<bool> {
        let results = Self::exists_by_md5_batch(db, &[md5.as_ref()]).await?;
        Ok(!results.is_empty())
    }

    /// 构建照片游标查询, 并统一处理时间与 ID 的排序边界.
    fn build_cursor_query(
        cursor: Option<&TimeIdCursor<PhotoId>>,
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
                // Next (倒序): 找 created_at <= anchor 的照片
                query = query.filter(Column::CreatedAt.lte(anchor));
            } else {
                // Prev (正序): 找 created_at >= anchor 的照片
                query = query.filter(Column::CreatedAt.gte(anchor));
            }
        }

        query
    }

    /// 游标查询照片id.
    pub async fn query_cursor_page_ids(
        db: &impl ConnectionTrait,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTime>,
    ) -> Result<CursorPage<PhotoId, ()>> {
        let records = Self::build_cursor_query(cursor.as_ref(), size, direction, anchor_time)
            .select_only()
            .column(Column::Id)
            .into_tuple::<PhotoId>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size))
    }

    /// 按照片id查询记录.
    pub async fn query_by_ids(
        db: &impl ConnectionTrait,
        ids: &[PhotoId],
    ) -> Result<Vec<PhotoRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .all(db)
            .await?
            .into_iter()
            .map(PhotoRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    #[cfg(feature = "face")]
    /// 批量查询照片 ID 和 file_id.
    pub async fn query_id_and_file_id_by_ids(
        db: &impl ConnectionTrait,
        ids: impl IntoIterator<Item = &PhotoId>,
    ) -> Result<HashMap<PhotoId, String>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.into_iter().copied()))
            .select_only()
            .column(Column::Id)
            .column(Column::FileId)
            .into_tuple::<(PhotoId, String)>()
            .all(db)
            .await?
            .into_iter()
            .collect::<HashMap<PhotoId, String>>()
            .to_ok()
    }

    /// 根据照片id查询属于用户的照片
    pub async fn query_by_user_id_and_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        ids: &[PhotoId],
    ) -> Result<Vec<PhotoRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .filter(Column::UserId.eq(user_id))
            .all(db)
            .await?
            .into_iter()
            .map(PhotoRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 根据 file_id 查询图片尺寸.
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

    /// 根据照片 id 查询 file_id.
    pub async fn query_file_id_by_id(db: &impl ConnectionTrait, id: PhotoId) -> Result<String> {
        Entity::find()
            .select_only()
            .column(Column::FileId)
            .filter(Column::Id.eq(id))
            .into_tuple::<String>()
            .one(db)
            .await?
            .context_error_none(
                "photo_file_id_not_exist",
                "照片file_id不存在",
                AppError::InternalServerError,
            )
    }

    /// 根据file_id 查询 id.
    pub async fn query_photo_id_by_file_id(
        db: &impl ConnectionTrait,
        file_id: &str,
    ) -> Result<Option<PhotoId>> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::FileId.eq(file_id))
            .into_tuple::<PhotoId>()
            .one(db)
            .await?
            .to_ok()
    }
}

// 删除
impl PhotoMapper {
    /// 删除照片
    pub async fn delete_by_ids(db: &impl ConnectionTrait, ids: &[PhotoId]) -> Result<()> {
        Entity::delete_many()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }
}
