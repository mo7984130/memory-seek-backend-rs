use std::collections::HashMap;

use common::error::AppError;
use common::utils::ResultExt;
use entities::{Embedding512, face_feature::*};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityName, EntityTrait, IdenStatic, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};

pub struct FaceFeatureMapper;

impl FaceFeatureMapper {
    /// 根据ID查询单个人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 特征ID
    ///
    /// # 返回
    /// 返回特征模型
    ///
    /// # 错误
    /// - `AppError::NotFound`: 特征不存在
    pub async fn query_by_id(db: &DatabaseConnection, id: i64) -> Result<Model, AppError> {
        Entity::find_by_id(id)
            .one(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")?
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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_photo_id(
        db: &DatabaseConnection,
        photo_id: i64,
    ) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .all(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")
    }

    /// 查询人物的所有人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `person_id`: 人物ID
    ///
    /// # 返回
    /// 返回该人物关联的所有人脸特征
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_person_id<C: ConnectionTrait>(
        db: &C,
        person_id: i64,
    ) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .filter(Column::PersonId.eq(person_id))
            .all(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")
    }

    /// 根据ID列表批量查询人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `ids`: 特征ID列表
    ///
    /// # 返回
    /// 返回匹配的特征列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_ids<C: ConnectionTrait>(
        db: &C,
        ids: &[i64],
    ) -> Result<Vec<Model>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .all(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")
    }

    /// 查询所有人脸特征（按ID排序）
    ///
    /// # 参数
    /// - `db`: 数据库连接
    ///
    /// # 返回
    /// 返回所有特征列表，用于聚类算法
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_all_ordered(db: &DatabaseConnection) -> Result<Vec<Model>, AppError> {
        Entity::find()
            .order_by_asc(Column::Id)
            .all(db)
            .await
            .trace_to_internal_err("db_query_err", "查询特征失败")
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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_cursor_page(
        db: &DatabaseConnection,
        person_id: i64,
        cursor: Option<i64>,
        size: u64,
    ) -> Result<Vec<Model>, AppError> {
        let mut query = Entity::find()
            .filter(Column::PersonId.eq(person_id))
            .order_by_desc(Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            query = query.filter(Column::Id.lt(c));
        }

        query
            .all(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")
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
    ///
    /// # 错误
    /// - `AppError`: 数据库插入失败
    pub async fn insert(
        db: &DatabaseConnection,
        photo_id: i64,
        person_id: Option<i64>,
        embedding: Embedding512,
        bbox: sea_orm::JsonValue,
        score: f32,
    ) -> Result<Model, AppError> {
        let now = chrono::Utc::now();
        let feature = ActiveModel {
            photo_id: Set(photo_id),
            person_id: Set(person_id),
            embedding: Set(embedding),
            bbox: Set(bbox),
            score: Set(score),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        feature
            .insert(db)
            .await
            .trace_to_internal_err("db_insert_err", "保存特征失败")
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
    ///
    /// # 错误
    /// - `AppError::NotFound`: 特征不存在
    /// - `AppError`: 数据库更新失败
    pub async fn update_person_id<C: ConnectionTrait>(
        db: &C,
        id: i64,
        person_id: Option<i64>,
    ) -> Result<Model, AppError> {
        let existing = Entity::find_by_id(id)
            .one(db)
            .await
            .trace_to_internal_err("db_query_err", "查询失败")?
            .ok_or_else(|| AppError::not_found("特征不存在"))?;
        let mut active: ActiveModel = existing.into();
        active.person_id = Set(person_id);
        active.updated_at = Set(chrono::Utc::now().into());
        active
            .update(db)
            .await
            .trace_to_internal_err("db_update_err", "更新失败")
    }

    /// 删除单个人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 特征ID
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        Entity::delete_by_id(id)
            .exec(db)
            .await
            .trace_to_internal_err("db_delete_err", "删除特征失败")?;
        Ok(())
    }

    /// 根据ID列表批量删除人脸特征
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `ids`: 特征ID列表
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败
    pub async fn delete_by_ids<C: ConnectionTrait>(db: &C, ids: Vec<i64>) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }

        Entity::delete_many()
            .filter(Column::Id.is_in(ids))
            .exec(db)
            .await
            .trace_to_internal_err("db_delete_err", "批量删除人脸特征失败")?;

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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询或删除失败
    pub async fn delete_by_photo_id(
        db: &DatabaseConnection,
        photo_id: i64,
    ) -> Result<Vec<Option<i64>>, AppError> {
        let features = Self::query_by_photo_id(db, photo_id).await?;
        let person_ids: Vec<Option<i64>> = features.iter().map(|f| f.person_id).collect();

        Entity::delete_many()
            .filter(Column::PhotoId.eq(photo_id))
            .exec(db)
            .await
            .trace_to_internal_err("db_delete_err", "删除特征失败")?;

        Ok(person_ids)
    }

    /// 批量查询每个人物的最高分特征
    ///
    /// 使用窗口函数 `ROW_NUMBER()` 按人物分组取最高分特征，用于人物统计更新。
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `person_ids`: 人物ID列表
    ///
    /// # 返回
    /// 返回人物ID到 `(feature_id, score)` 的映射，无特征的人物值为 `None`
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_max_score_features_by_person_ids(
        db: &impl ConnectionTrait,
        person_ids: Vec<i64>,
    ) -> Result<HashMap<i64, Option<(i64, f32)>>, AppError> {
        if person_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let person_id_col = Column::PersonId;
        let id_col = Column::Id;
        let score_col = Column::Score;

        let sql = format!(
            r#"
            SELECT {person_id}, {id}, {score}
            FROM (
                SELECT
                    {person_id},
                    {id},
                    {score},
                    ROW_NUMBER() OVER (PARTITION BY {person_id} ORDER BY {score} DESC) as rn
                FROM {table}
                WHERE {person_id} = ANY($1)
            ) sub
            WHERE rn = 1
            "#,
            table = Entity.table_name(),
            person_id = person_id_col.as_str(),
            id = id_col.as_str(),
            score = score_col.as_str(),
        );

        let values: Vec<sea_orm::Value> = person_ids.iter().map(|&id| id.into()).collect();

        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                &sql,
                values,
            ))
            .await
            .trace_to_internal_err("db_query_err", "查询最高分特征失败")?;

        let mut result: HashMap<i64, Option<(i64, f32)>> =
            person_ids.iter().map(|&id| (id, None)).collect();

        for row in rows {
            let person_id: i64 = row
                .try_get("", person_id_col.as_str())
                .trace_to_internal_err("db_query_err", "获取person_id列错误")?;
            let feature_id: i64 = row
                .try_get("", id_col.as_str())
                .trace_to_internal_err("db_query_err", "获取feature_id列错误")?;
            let score: f32 = row
                .try_get("", score_col.as_str())
                .trace_to_internal_err("db_query_err", "获取score列错误")?;
            result.insert(person_id, Some((feature_id, score)));
        }

        Ok(result)
    }
}
