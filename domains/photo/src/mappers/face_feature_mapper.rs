use common::error::AppError;
use common::utils::ResultExt;
use entities::{face_feature, DrVector};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};

use std::collections::HashMap;

pub struct FaceFeatureMapper;

impl FaceFeatureMapper {
    /// 根据ID查询单个人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 特征ID
    ///
    /// # 返回
    /// - 成功: 返回特征模型
    /// - 失败: 特征不存在返回404错误
    pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<face_feature::Model, AppError> {
        face_feature::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::not_found("特征不存在"))
    }

    /// 查询照片中的所有人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回该照片中的所有人脸特征
    pub async fn find_by_photo_id(db: &DatabaseConnection, photo_id: i64) -> Result<Vec<face_feature::Model>, AppError> {
        face_feature::Entity::find()
            .filter(face_feature::Column::PhotoId.eq(photo_id))
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 查询人物的所有人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `person_id`: 人物ID
    ///
    /// # 返回
    /// 返回该人物关联的所有人脸特征
    pub async fn find_by_person_id<C: ConnectionTrait>(db: &C, person_id: i64) -> Result<Vec<face_feature::Model>, AppError> {
        face_feature::Entity::find()
            .filter(face_feature::Column::PersonId.eq(person_id))
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 根据ID列表批量查询人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `ids`: 特征ID列表
    ///
    /// # 返回
    /// 返回匹配的特征列表
    pub async fn find_by_ids<C: ConnectionTrait>(db: &C, ids: Vec<i64>) -> Result<Vec<face_feature::Model>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        face_feature::Entity::find()
            .filter(face_feature::Column::Id.is_in(ids))
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 根据ID列表批量查询人脸特征，返回Map结构
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `ids`: 特征ID列表
    ///
    /// # 返回
    /// 返回以特征ID为键的HashMap
    pub async fn find_by_ids_map<C: ConnectionTrait>(db: &C, ids: Vec<i64>) -> Result<HashMap<i64, face_feature::Model>, AppError> {
        let features = Self::find_by_ids(db, ids).await?;
        Ok(features.into_iter().map(|f| (f.id, f)).collect())
    }

    /// 查询所有人脸特征（按ID排序）
    ///
    /// # 参数
    /// - `db`: 数据库连接
    ///
    /// # 返回
    /// 返回所有特征列表，用于聚类算法
    pub async fn find_all_ordered(db: &DatabaseConnection) -> Result<Vec<face_feature::Model>, AppError> {
        face_feature::Entity::find()
            .order_by_asc(face_feature::Column::Id)
            .all(db)
            .await
            .map_internal_err("查询特征失败")
    }

    /// 游标分页查询人物的人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `person_id`: 人物ID
    /// - `cursor`: 游标值（特征ID）
    /// - `size`: 每页数量
    ///
    /// # 返回
    /// 返回特征列表，按ID倒序排列
    pub async fn find_cursor_page(
        db: &DatabaseConnection,
        person_id: i64,
        cursor: Option<i64>,
        size: u64,
    ) -> Result<Vec<face_feature::Model>, AppError> {
        let mut query = face_feature::Entity::find()
            .filter(face_feature::Column::PersonId.eq(person_id))
            .order_by_desc(face_feature::Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            query = query.filter(face_feature::Column::Id.lt(c));
        }

        query.all(db).await.map_internal_err("查询失败")
    }

    /// 创建新人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    /// - `person_id`: 人物ID（可选，未识别时为None）
    /// - `embedding`: 人脸特征向量
    /// - `bbox`: 人脸边界框（JSON格式）
    /// - `score`: 检测置信度
    ///
    /// # 返回
    /// 返回创建的特征模型
    pub async fn insert(
        db: &DatabaseConnection,
        photo_id: i64,
        person_id: Option<i64>,
        embedding: DrVector,
        bbox: sea_orm::JsonValue,
        score: f32,
    ) -> Result<face_feature::Model, AppError> {
        let now = chrono::Utc::now();
        let feature = face_feature::ActiveModel {
            photo_id: Set(photo_id),
            person_id: Set(person_id),
            embedding: Set(embedding),
            bbox: Set(bbox),
            score: Set(score),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        feature.insert(db).await.map_internal_err("保存特征失败")
    }

    /// 更新人脸特征的人物关联
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 特征ID
    /// - `person_id`: 新人物ID（传None取消关联）
    ///
    /// # 返回
    /// 返回更新后的特征模型
    pub async fn update_person_id<C: ConnectionTrait>(
        db: &C,
        id: i64,
        person_id: Option<i64>,
    ) -> Result<face_feature::Model, AppError> {
        let existing = face_feature::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::not_found("特征不存在"))?;
        let mut active: face_feature::ActiveModel = existing.into();
        active.person_id = Set(person_id);
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(db).await.map_internal_err("更新失败")
    }

    /// 删除单个人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 特征ID
    ///
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        face_feature::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_internal_err("删除特征失败")?;
        Ok(())
    }

    pub async fn delete_by_ids<C: ConnectionTrait>(
        db: &C,
        ids: Vec<i64>
    ) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }

        face_feature::Entity::delete_many()
            .filter(face_feature::Column::Id.is_in(ids))
            .exec(db)
            .await
            .map_internal_err("批量删除人脸特征失败")?;

        Ok(())
    }

    /// 删除照片的所有人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回被删除特征关联的 person_id 列表（用于更新人物统计）
    pub async fn delete_by_photo_id(db: &DatabaseConnection, photo_id: i64) -> Result<Vec<Option<i64>>, AppError> {
        let features = Self::find_by_photo_id(db, photo_id).await?;
        let person_ids: Vec<Option<i64>> = features.iter().map(|f| f.person_id).collect();

        face_feature::Entity::delete_many()
            .filter(face_feature::Column::PhotoId.eq(photo_id))
            .exec(db)
            .await
            .map_internal_err("删除特征失败")?;

        Ok(person_ids)
    }
}
