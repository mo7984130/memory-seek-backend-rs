use std::collections::HashSet;

use common::Result;
use common::error::AppError;
use common::ext::{BoolExt, OkExt};
use common::models::TimeIdCursor;
use sea_orm::entity::prelude::DateTimeUtc;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

use types::photo::{dto::photo::PageDirection, photo::*};

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
            .filter(Column::Id.eq(photo_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// 更新照片点赞数（增量）
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
    pub async fn exists(db: &impl ConnectionTrait, photo_id: PhotoId) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(photo_id))
            .count(db)
            .await?;
        Ok(count > 0)
    }

    pub async fn ensure_exist(db: &impl ConnectionTrait, photo_id: PhotoId) -> Result<()> {
        Self::exists(db, photo_id).await?.true_or_warn(
            "photo_not_exist",
            "照片不存在",
            AppError::not_found("照片不存在"),
        )
    }

    pub async fn exists_by_md5_batch(
        db: &impl ConnectionTrait,
        md5s: &[impl AsRef<str>],
    ) -> Result<HashSet<String>> {
        if md5s.is_empty() {
            return Ok(HashSet::new());
        }
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

    pub async fn exists_by_md5(db: &impl ConnectionTrait, md5: impl AsRef<str>) -> Result<bool> {
        let results = Self::exists_by_md5_batch(db, &[md5.as_ref()]).await?;
        Ok(!results.is_empty())
    }

    fn build_cursor_query(
        cursor: Option<&TimeIdCursor<PhotoId>>,
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

    pub async fn query_cursor_page_ids(
        db: &impl ConnectionTrait,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTimeUtc>,
    ) -> Result<Vec<PhotoId>> {
        Self::build_cursor_query(cursor.as_ref(), size, direction, anchor_time)
            .select_only()
            .column(Column::Id)
            .into_tuple::<i64>()
            .all(db)
            .await?
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
            .filter(Column::Id.is_in(ids.iter().copied()))
            .all(db)
            .await?
            .into_iter()
            .map(PhotoRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 按 ID 集合查询照片分页 ID(游标过滤, 按 created_at/id 倒序)
    pub async fn query_ids_page_by_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
        cursor: Option<&TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        if photo_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut query = Entity::find()
            .filter(Column::Id.is_in(photo_ids.iter().copied()))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            query = query.filter(
                sea_orm::Condition::any()
                    .add(Column::CreatedAt.lt(c.created_at))
                    .add(
                        sea_orm::Condition::all()
                            .add(Column::CreatedAt.eq(c.created_at))
                            .add(Column::Id.lt(c.id)),
                    ),
            );
        }

        query
            .select_only()
            .column(Column::Id)
            .into_tuple::<i64>()
            .all(db)
            .await?
            .into_iter()
            .map(PhotoId)
            .collect::<Vec<_>>()
            .to_ok()
    }

    #[expect(dead_code)]
    pub async fn query_by_id(
        db: &impl ConnectionTrait,
        id: PhotoId,
    ) -> Result<Option<PhotoRecord>> {
        Entity::find()
            .filter(Column::Id.eq(id))
            .one(db)
            .await?
            .map(PhotoRecord::from)
            .to_ok()
    }

    /// 根据文件 ID 查询图片宽高（裁剪 token 归一化坐标换算用）
    pub async fn query_dimensions_by_file_id(
        db: &impl ConnectionTrait,
        file_id: &str,
    ) -> Result<Option<(i32, i32)>> {
        Entity::find()
            .select_only()
            .column(Column::Width)
            .column(Column::Height)
            .filter(Column::FileId.eq(file_id))
            .into_tuple::<(i32, i32)>()
            .one(db)
            .await?
            .to_ok()
    }

    pub async fn query_file_id_by_id(
        db: &impl ConnectionTrait,
        id: PhotoId,
    ) -> Result<Option<String>> {
        Entity::find()
            .select_only()
            .column(Column::FileId)
            .filter(Column::Id.eq(id))
            .into_tuple::<String>()
            .one(db)
            .await?
            .to_ok()
    }
}

// 删除
impl PhotoMapper {
    pub async fn delete_by_ids(db: &impl ConnectionTrait, ids: &[PhotoId]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        Entity::delete_many()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }
}
