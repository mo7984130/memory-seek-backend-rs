use std::collections::HashSet;

use common::Result;
use common::ext::OptionExt;
use common::{
    error::AppError,
    ext::{OkExt, ResultErrExt},
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use entities::photo::photo::*;

use crate::models::photo::{PageDirection, PhotoCursor};

pub(crate) struct PhotoMapper;
impl PhotoMapper {
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
        }

        query
    }

    pub async fn query_cursor_page_ids(
        db: &impl ConnectionTrait,
        cursor: Option<PhotoCursor>,
        size: u64,
        direction: PageDirection,
    ) -> Result<Vec<PhotoId>> {
        Self::build_cursor_query(cursor.as_ref(), size, direction)
            .select_only()
            .column(Column::Id)
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询 ID 列表失败")?
            .into_iter()
            .map(PhotoId::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn query_by_ids(db: &impl ConnectionTrait, ids: &[PhotoId]) -> Result<Vec<Model>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().map(|id| id.0)))
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询照片失败")
    }

    pub async fn query_by_id(db: &impl ConnectionTrait, id: PhotoId) -> Result<Model> {
        Entity::find_by_id(id.0)
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询照片失败")?
            .ok_or_warn(
                "photo_not_found",
                "照片不存在",
                AppError::not_found("照片不存在"),
            )
    }

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
