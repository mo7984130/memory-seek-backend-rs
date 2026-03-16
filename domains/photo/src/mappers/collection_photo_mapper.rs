use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::collection_photo;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set};

use std::collections::HashMap;

pub struct CollectionPhotoMapper;

impl CollectionPhotoMapper {
    /// 游标分页查询收藏夹中的照片关系
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `collection_id`: 收藏夹ID
    /// - `cursor`: 游标时间点
    /// - `size`: 每页数量
    /// 
    /// # 返回
    /// 返回收藏夹-照片关系列表，按添加时间倒序
    pub async fn find_by_collection_id(
        db: &DatabaseConnection,
        collection_id: i64,
        cursor: Option<DateTime<Utc>>,
        size: u64,
    ) -> Result<Vec<collection_photo::Model>, AppError> {
        let limit = size + 1;
        let mut query = collection_photo::Entity::find()
            .filter(collection_photo::Column::CollectionId.eq(collection_id))
            .order_by_desc(collection_photo::Column::CreatedAt)
            .limit(limit);

        if let Some(c) = cursor {
            query = query.filter(collection_photo::Column::CreatedAt.lt(c));
        }

        query.all(db).await.map_internal_err("查询失败")
    }

    /// 批量查询多个收藏夹中的照片关系
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `collection_ids`: 收藏夹ID列表
    /// 
    /// # 返回
    /// 返回所有匹配的收藏夹-照片关系
    pub async fn find_by_collection_ids(
        db: &DatabaseConnection,
        collection_ids: Vec<i64>,
    ) -> Result<Vec<collection_photo::Model>, AppError> {
        if collection_ids.is_empty() {
            return Ok(vec![]);
        }
        collection_photo::Entity::find()
            .filter(collection_photo::Column::CollectionId.is_in(collection_ids))
            .order_by_desc(collection_photo::Column::CreatedAt)
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 查询照片所在的收藏夹ID列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 返回该照片所在的所有收藏夹ID列表
    pub async fn find_collection_ids_by_photo(
        db: &DatabaseConnection,
        user_id: i64,
        photo_id: i64,
    ) -> Result<Vec<i64>, AppError> {
        let relations = collection_photo::Entity::find()
            .filter(collection_photo::Column::UserId.eq(user_id))
            .filter(collection_photo::Column::PhotoId.eq(photo_id))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(relations.iter().map(|r| r.collection_id).collect())
    }

    /// 查询每个收藏夹的最新照片ID
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `collection_ids`: 收藏夹ID列表
    /// 
    /// # 返回
    /// 返回以收藏夹ID为键、最新照片ID为值的HashMap
    pub async fn find_latest_photo_ids_by_collections(
        db: &DatabaseConnection,
        collection_ids: Vec<i64>,
    ) -> Result<HashMap<i64, i64>, AppError> {
        if collection_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let relations = collection_photo::Entity::find()
            .filter(collection_photo::Column::CollectionId.is_in(collection_ids))
            .order_by_desc(collection_photo::Column::CreatedAt)
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        let mut result = HashMap::new();
        for cp in relations {
            if !result.contains_key(&cp.collection_id) {
                result.insert(cp.collection_id, cp.photo_id);
            }
        }
        Ok(result)
    }

    /// 检查照片是否在收藏夹中
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `collection_id`: 收藏夹ID
    /// - `photo_ids`: 照片ID列表
    /// 
    /// # 返回
    /// 返回在收藏夹中的照片ID列表
    pub async fn exists_in_collection(
        db: &DatabaseConnection,
        collection_id: i64,
        photo_ids: Vec<i64>,
    ) -> Result<Vec<i64>, AppError> {
        if photo_ids.is_empty() {
            return Ok(vec![]);
        }

        let relations = collection_photo::Entity::find()
            .filter(collection_photo::Column::CollectionId.eq(collection_id))
            .filter(collection_photo::Column::PhotoId.is_in(photo_ids))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(relations.iter().map(|r| r.photo_id).collect())
    }

    /// 检查单张照片是否在收藏夹中
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 存在返回true，否则返回false
    pub async fn exists_photo_in_collection(
        db: &DatabaseConnection,
        collection_id: i64,
        photo_id: i64,
    ) -> Result<bool, AppError> {
        let count = collection_photo::Entity::find()
            .filter(collection_photo::Column::CollectionId.eq(collection_id))
            .filter(collection_photo::Column::PhotoId.eq(photo_id))
            .count(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(count > 0)
    }

    /// 添加照片到收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回创建的收藏夹-照片关系模型
    pub async fn insert<C: ConnectionTrait>(
        db: &C,
        collection_id: i64,
        photo_id: i64,
        user_id: i64,
    ) -> Result<collection_photo::Model, AppError> {
        let now = Utc::now();
        let relation = collection_photo::ActiveModel {
            collection_id: Set(collection_id),
            photo_id: Set(photo_id),
            user_id: Set(user_id),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        relation.insert(db).await.map_internal_err("添加到收藏夹失败")
    }

    /// 从收藏夹移除照片
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 成功移除返回true，未找到关系返回false
    pub async fn delete<C: ConnectionTrait>(
        db: &C,
        collection_id: i64,
        photo_id: i64,
        user_id: i64,
    ) -> Result<bool, AppError> {
        let result = collection_photo::Entity::delete_many()
            .filter(collection_photo::Column::CollectionId.eq(collection_id))
            .filter(collection_photo::Column::PhotoId.eq(photo_id))
            .filter(collection_photo::Column::UserId.eq(user_id))
            .exec(db)
            .await
            .map_internal_err("移除失败")?;

        Ok(result.rows_affected > 0)
    }

    /// 删除收藏夹中的所有照片关系
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `collection_id`: 收藏夹ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_collection_id<C: ConnectionTrait>(db: &C, collection_id: i64) -> Result<(), AppError> {
        collection_photo::Entity::delete_many()
            .filter(collection_photo::Column::CollectionId.eq(collection_id))
            .exec(db)
            .await
            .map_internal_err("删除收藏夹照片失败")?;
        Ok(())
    }

    /// 删除照片的所有收藏关联
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 返回受影响的收藏夹ID列表
    pub async fn delete_by_photo_id<C: ConnectionTrait>(db: &C, photo_id: i64) -> Result<Vec<i64>, AppError> {
        let relations = collection_photo::Entity::find()
            .filter(collection_photo::Column::PhotoId.eq(photo_id))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        let collection_ids: Vec<i64> = relations.iter().map(|r| r.collection_id).collect();

        if !collection_ids.is_empty() {
            collection_photo::Entity::delete_many()
                .filter(collection_photo::Column::PhotoId.eq(photo_id))
                .exec(db)
                .await
                .map_internal_err("删除收藏关联失败")?;
        }

        Ok(collection_ids)
    }
}
