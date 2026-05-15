use common::error::AppError;
use common::utils::ResultExt;
use entities::photo::{Column, Entity, Model};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};

use crate::{models::photo::PhotoCursor, photo::TimeRange, photo_service::PageDirection};
use std::collections::HashSet;

pub struct PhotoMapper;

impl PhotoMapper {
    /// 根据ID查询单张照片
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 照片ID
    ///
    /// # 返回
    /// - 成功: 返回照片模型
    /// - 失败: 照片不存在返回404错误，数据库错误返回500错误
    pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<Model, AppError> {
        Entity::find_by_id(id)
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询照片失败")?
            .ok_or_else(|| AppError::not_found("照片不存在"))
    }

    /// 根据ID列表批量查询照片
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `ids`: 照片ID列表
    ///
    /// # 返回
    /// 返回匹配的照片列表，空ID列表返回空列表
    pub async fn find_by_ids(
        db: &DatabaseConnection,
        ids: Vec<i64>,
    ) -> Result<Vec<Model>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        Entity::find()
            .filter(Column::Id.is_in(ids))
            .all(db)
            .await
            .map_internal_err("查询照片失败")
    }

    /// 批量检查MD5是否已存在
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `md5s`: 文件MD5哈希值列表
    ///
    /// # 返回
    /// 返回已存在的MD5集合
    pub async fn exists_by_md5_batch<S: AsRef<str>>(
        db: &DatabaseConnection,
        md5s: &[S],
    ) -> Result<HashSet<String>, AppError> {
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

    /// 查询所有照片的时间范围
    ///
    /// # 参数
    /// - `db`: 数据库连接
    ///
    /// # 返回
    /// 返回最早和最晚照片的创建时间元组，无照片时返回当前时间
    pub async fn find_time_range(db: &DatabaseConnection) -> Result<TimeRange, AppError> {
        let result = Entity::find()
            .select_only()
            .column_as(Column::CreatedAt.min(), "min_time")
            .column_as(Column::CreatedAt.max(), "max_time")
            .into_model::<TimeRange>()
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询时间范围失败")?;
        Ok(result.unwrap_or_default())
    }

    /// 游标分页查询照片列表
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `cursor`: 复合游标
    /// - `size`: 每页数量
    /// - `direction`: 分页方向，"next"为下一页，其他为上一页
    ///
    /// # 返回
    /// 返回照片列表，按创建时间倒序、ID倒序排列
    pub async fn find_cursor_page(
        db: &DatabaseConnection,
        cursor: Option<&PhotoCursor>,
        size: u64,
        direction: &str,
    ) -> Result<Vec<Model>, AppError> {
        let mut query = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            if direction == "next" {
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(Column::CreatedAt.lt(c.created_at))
                        .add(
                            sea_orm::Condition::all()
                                .add(Column::CreatedAt.eq(c.created_at))
                                .add(Column::Id.lt(c.id)),
                        ),
                );
            } else {
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(Column::CreatedAt.gt(c.created_at))
                        .add(
                            sea_orm::Condition::all()
                                .add(Column::CreatedAt.eq(c.created_at))
                                .add(Column::Id.gt(c.id)),
                        ),
                );
            }
        }

        query.all(db).await.map_internal_err("查询失败")
    }

    /// 根据文件ID查询照片
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `file_id`: 文件路径
    ///
    /// # 返回
    /// 返回匹配的照片模型，不存在返回404错误
    pub async fn find_by_file_id(
        db: &DatabaseConnection,
        file_id: &str,
    ) -> Result<Model, AppError> {
        Entity::find()
            .filter(Column::FileId.eq(file_id))
            .one(db)
            .await
            .map_internal_err("查询照片失败")?
            .ok_or_else(|| AppError::not_found("照片不存在"))
    }

    /// 游标分页查询照片id
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `cursor`: 复合游标
    /// - `size`: 每页数量
    /// - `direction`: 分页方向，"next"为下一页，其他为上一页
    ///
    /// # 返回
    /// 返回照片id，按创建时间倒序、ID倒序排列
    pub async fn find_cursor_page_ids(
        db: &DatabaseConnection,
        cursor: Option<PhotoCursor>,
        size: u64,
        direction: PageDirection,
    ) -> Result<Vec<i64>, AppError> {
        let mut query = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .select_only()
            .column(Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            if direction == PageDirection::Next {
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(Column::CreatedAt.lt(c.created_at))
                        .add(
                            sea_orm::Condition::all()
                                .add(Column::CreatedAt.eq(c.created_at))
                                .add(Column::Id.lt(c.id)),
                        ),
                );
            } else if direction == PageDirection::Prev {
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(Column::CreatedAt.gt(c.created_at))
                        .add(
                            sea_orm::Condition::all()
                                .add(Column::CreatedAt.eq(c.created_at))
                                .add(Column::Id.gt(c.id)),
                        ),
                );
            }
        }

        let mut ids = query
            .into_tuple::<i64>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询 ID 列表失败")?;

        // Prev 方向：DB 用 asc 查出来后反转，还原为倒序
        if direction == PageDirection::Prev {
            ids.reverse();
        }

        Ok(ids)
    }

    /// 删除照片
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 照片ID
    ///
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_internal_err("删除照片失败")?;
        Ok(())
    }
}
