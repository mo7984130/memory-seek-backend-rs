use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::photo::{Entity, Model, Column};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};

use std::collections::HashMap;

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
            .map_internal_err("查询照片失败")?
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
    pub async fn find_by_ids(db: &DatabaseConnection, ids: Vec<i64>) -> Result<Vec<Model>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        Entity::find()
            .filter(Column::Id.is_in(ids))
            .all(db)
            .await
            .map_internal_err("查询照片失败")
    }

    /// 根据ID列表批量查询照片，返回Map结构
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `ids`: 照片ID列表
    /// 
    /// # 返回
    /// 返回以照片ID为键的HashMap
    pub async fn find_by_ids_map(db: &DatabaseConnection, ids: Vec<i64>) -> Result<HashMap<i64, Model>, AppError> {
        let photos = Self::find_by_ids(db, ids).await?;
        Ok(photos.into_iter().map(|p| (p.id, p)).collect())
    }

    /// 检查MD5是否已存在
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `md5`: 文件MD5哈希值
    /// 
    /// # 返回
    /// 返回该MD5是否存在
    pub async fn exists_by_md5(db: &DatabaseConnection, md5: &str) -> Result<bool, AppError> {
        let count = Entity::find()
            .filter(Column::Md5.eq(md5))
            .count(db)
            .await
            .map_internal_err("查询MD5失败")?;
        Ok(count > 0)
    }

    /// 查询所有照片的时间范围
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// 
    /// # 返回
    /// 返回最早和最晚照片的创建时间元组，无照片时返回当前时间
    pub async fn find_time_range(db: &DatabaseConnection) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
        let min = Entity::find()
            .order_by_asc(Column::CreatedAt)
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        let max = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        let min_time = min
            .map(|p| p.created_at.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let max_time = max
            .map(|p| p.created_at.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        Ok((min_time, max_time))
    }

    /// 游标分页查询照片列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `cursor`: 游标时间点，用于分页
    /// - `size`: 每页数量
    /// - `direction`: 分页方向，"next"为下一页，其他为上一页
    /// 
    /// # 返回
    /// 返回照片列表，按创建时间倒序排列
    pub async fn find_cursor_page(
        db: &DatabaseConnection,
        cursor: Option<DateTime<Utc>>,
        size: u64,
        direction: &str,
    ) -> Result<Vec<Model>, AppError> {
        let mut query = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .limit(size);

        if let Some(c) = cursor {
            if direction == "next" {
                query = query.filter(Column::CreatedAt.lt(c));
            } else {
                query = query.filter(Column::CreatedAt.gt(c));
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
    pub async fn find_by_file_id(db: &DatabaseConnection, file_id: &str) -> Result<Model, AppError> {
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
    /// - `cursor`: 游标时间点，用于分页
    /// - `size`: 每页数量
    /// - `direction`: 分页方向，"next"为下一页，其他为上一页
    ///
    /// # 返回
    /// 返回照片id，按创建时间倒序排列
    pub async fn find_cursor_page_ids(
        db: &DatabaseConnection,
        cursor: Option<DateTime<Utc>>,
        size: u64,
        direction: &str,
    ) -> Result<Vec<i64>, AppError> {
        let mut query = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .select_only()
            .column(Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            if direction == "next" {
                query = query.filter(Column::CreatedAt.lt(c));
            } else {
                query = query.filter(Column::CreatedAt.gt(c));
            }
        }

        query.into_tuple::<i64>()
            .all(db)
            .await
            .map_internal_err("查询 ID 列表失败")
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
