use chrono::Utc;
use common::error::AppError;
use common::utils::ResultExt;
use entities::collection;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};

pub struct CollectionMapper;

impl CollectionMapper {
    /// 根据ID查询单个收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 收藏夹ID
    /// 
    /// # 返回
    /// - 成功: 返回收藏夹模型
    /// - 失败: 收藏夹不存在返回404错误
    pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<collection::Model, AppError> {
        collection::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询收藏夹失败")?
            .ok_or_else(|| AppError::not_found("收藏夹不存在"))
    }

    /// 查询用户的所有收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回用户的收藏夹列表，按是否收藏排序，再按创建时间倒序
    pub async fn find_by_user_id(db: &DatabaseConnection, user_id: i64) -> Result<Vec<collection::Model>, AppError> {
        collection::Entity::find()
            .filter(collection::Column::UserId.eq(user_id))
            .order_by_asc(collection::Column::IsFavorite)
            .order_by_desc(collection::Column::CreatedAt)
            .all(db)
            .await
            .map_internal_err("查询收藏夹失败")
    }

    /// 查询用户的"我喜欢"收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回用户的"我喜欢"收藏夹，不存在返回None
    pub async fn find_favorite_by_user_id(db: &DatabaseConnection, user_id: i64) -> Result<Option<collection::Model>, AppError> {
        collection::Entity::find()
            .filter(collection::Column::UserId.eq(user_id))
            .filter(collection::Column::IsFavorite.eq(true))
            .one(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 查询用户"我喜欢"收藏夹的ID
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回"我喜欢"收藏夹的ID，不存在返回None
    pub async fn find_favorite_collection_id(db: &DatabaseConnection, user_id: i64) -> Result<Option<i64>, AppError> {
        let result = collection::Entity::find()
            .filter(collection::Column::UserId.eq(user_id))
            .filter(collection::Column::IsFavorite.eq(true))
            .select_only()
            .column(collection::Column::Id)
            .into_tuple::<(i64,)>()
            .one(db)
            .await
            .map_internal_err("查询失败")?;
        Ok(result.map(|r| r.0))
    }

    /// 创建新收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `name`: 收藏夹名称
    /// - `description`: 收藏夹描述
    /// - `is_favorite`: 是否为"我喜欢"收藏夹
    /// 
    /// # 返回
    /// 返回创建的收藏夹模型
    pub async fn insert(
        db: &DatabaseConnection,
        user_id: i64,
        name: String,
        description: Option<String>,
        is_favorite: bool,
    ) -> Result<collection::Model, AppError> {
        let now = Utc::now();
        let collection = collection::ActiveModel {
            user_id: Set(user_id),
            name: Set(name),
            description: Set(description),
            photo_count: Set(0),
            cover_image_id: Set(None),
            is_favorite: Set(is_favorite),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        collection.insert(db).await.map_internal_err("创建收藏夹失败")
    }

    /// 更新收藏夹信息
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 收藏夹ID
    /// - `name`: 新名称（可选）
    /// - `description`: 新描述（可选）
    /// - `photo_count`: 新照片数量（可选）
    /// - `cover_image_id`: 新封面图片ID（可选，传None清除封面）
    /// 
    /// # 返回
    /// 返回更新后的收藏夹模型
    pub async fn update(
        db: &DatabaseConnection,
        id: i64,
        name: Option<String>,
        description: Option<String>,
        photo_count: Option<i64>,
        cover_image_id: Option<Option<i64>>,
    ) -> Result<collection::Model, AppError> {
        let existing = Self::find_by_id(db, id).await?;
        let mut active: collection::ActiveModel = existing.into();

        if let Some(n) = name {
            active.name = Set(n);
        }
        if let Some(d) = description {
            active.description = Set(Some(d));
        }
        if let Some(c) = photo_count {
            active.photo_count = Set(c);
        }
        if let Some(c) = cover_image_id {
            active.cover_image_id = Set(c);
        }
        active.updated_at = Set(Utc::now().into());

        active.update(db).await.map_internal_err("更新收藏夹失败")
    }

    /// 删除收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 收藏夹ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        collection::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_internal_err("删除收藏夹失败")?;
        Ok(())
    }

    /// 原子递增/递减照片数量
    /// 
    /// 使用查询+更新方式
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 收藏夹ID
    /// - `delta`: 变化量（正数递增，负数递减）
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn increment_photo_count<C: ConnectionTrait>(db: &C, id: i64, delta: i32) -> Result<(), AppError> {
        let collection = collection::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询收藏夹失败")?
            .ok_or_else(|| AppError::not_found("收藏夹不存在"))?;

        let mut active: collection::ActiveModel = collection.into();
        let new_count = (*active.photo_count.as_ref() + delta as i64).max(0);
        active.photo_count = Set(new_count);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await.map_internal_err("更新照片数量失败")?;
        Ok(())
    }
}
