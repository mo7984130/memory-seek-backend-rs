use common::error::AppError;
use common::utils::ResultExt;
use entities::{face_person, DrVector};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};

use crate::models::face::PersonCursor;
use std::collections::HashMap;

pub struct FacePersonMapper;

impl FacePersonMapper {
    /// 根据ID查询单个人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 人物ID
    /// 
    /// # 返回
    /// - 成功: 返回人物模型
    /// - 失败: 人物不存在返回404错误
    pub async fn find_by_id(db: &DatabaseConnection, id: i64) -> Result<face_person::Model, AppError> {
        face_person::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::not_found("人物不存在"))
    }

    /// 查询所有人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// 
    /// # 返回
    /// 返回所有人物列表，按名称排序
    pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<face_person::Model>, AppError> {
        face_person::Entity::find()
            .order_by_asc(face_person::Column::Name)
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 根据名称查询人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `name`: 人物名称
    /// 
    /// # 返回
    /// 返回匹配的人物，不存在返回None
    pub async fn find_by_name(db: &DatabaseConnection, name: &str) -> Result<Option<face_person::Model>, AppError> {
        face_person::Entity::find()
            .filter(face_person::Column::Name.eq(name))
            .one(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 游标分页查询人物列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `cursor`: 复合游标
    /// - `size`: 每页数量
    /// 
    /// # 返回
    /// 返回人物列表，按照片数量倒序、ID倒序排列
    pub async fn find_cursor_page(
        db: &DatabaseConnection,
        cursor: Option<&PersonCursor>,
        size: u64,
    ) -> Result<Vec<face_person::Model>, AppError> {
        let mut query = face_person::Entity::find()
            .order_by_desc(face_person::Column::TotalPhotoCount)
            .order_by_desc(face_person::Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            query = query.filter(
                Condition::any()
                    .add(face_person::Column::TotalPhotoCount.lt(c.total_photo_count))
                    .add(
                        Condition::all()
                            .add(face_person::Column::TotalPhotoCount.eq(c.total_photo_count))
                            .add(face_person::Column::Id.lt(c.id))
                    )
            );
        }

        query.all(db).await.map_internal_err("查询失败")
    }

    /// 根据ID列表批量查询人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `ids`: 人物ID列表
    /// 
    /// # 返回
    /// 返回匹配的人物列表
    pub async fn find_by_ids(db: &DatabaseConnection, ids: Vec<i64>) -> Result<Vec<face_person::Model>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        face_person::Entity::find()
            .filter(face_person::Column::Id.is_in(ids))
            .all(db)
            .await
            .map_internal_err("查询失败")
    }

    /// 根据ID列表批量查询人物，返回Map结构
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `ids`: 人物ID列表
    /// 
    /// # 返回
    /// 返回以人物ID为键的HashMap
    pub async fn find_by_ids_map(db: &DatabaseConnection, ids: Vec<i64>) -> Result<HashMap<i64, face_person::Model>, AppError> {
        let persons = Self::find_by_ids(db, ids).await?;
        Ok(persons.into_iter().map(|p| (p.id, p)).collect())
    }

    /// 创建人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `name`: 人物名称
    /// - `name_initials`: 名字首字母
    /// - `max_score_feature_id`: 最高分特征ID
    /// - `max_score`: 最高分值
    /// - `total_photo_count`: 照片总数
    /// - `centroid_embedding`: 质心向量
    /// - `total_weight_count`: 总权重
    /// 
    /// # 返回
    /// 返回创建的人物模型
    pub async fn insert<C: ConnectionTrait>(
        db: &C,
        name: String,
        name_initials: Option<String>,
        max_score_feature_id: i64,
        max_score: f32,
        total_photo_count: i64,
        centroid_embedding: DrVector,
        total_weight_count: f32,
    ) -> Result<face_person::Model, AppError> {
        let now = chrono::Utc::now();
        let person = face_person::ActiveModel {
            name: Set(name),
            name_initials: Set(name_initials),
            max_score_feature_id: Set(max_score_feature_id),
            max_score: Set(max_score),
            total_photo_count: Set(total_photo_count),
            centroid_embedding: Set(centroid_embedding),
            total_weight_count: Set(total_weight_count),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        person.insert(db).await.map_internal_err("创建人物失败")
    }

    /// 更新人物信息
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 人物ID
    /// - `name`: 新名称（可选）
    /// - `name_initials`: 新首字母（可选）
    /// - `max_score_feature_id`: 新最高分特征ID（可选）
    /// - `max_score`: 新最高分值（可选）
    /// - `total_photo_count`: 新照片总数（可选）
    /// - `centroid_embedding`: 新质心向量（可选）
    /// - `total_weight_count`: 新总权重（可选）
    /// 
    /// # 返回
    /// 返回更新后的人物模型
    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: i64,
        name: Option<String>,
        name_initials: Option<String>,
        max_score_feature_id: Option<i64>,
        max_score: Option<f32>,
        total_photo_count: Option<i64>,
        centroid_embedding: Option<DrVector>,
        total_weight_count: Option<f32>,
    ) -> Result<face_person::Model, AppError> {
        let existing = face_person::Entity::find_by_id(id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::not_found("人物不存在"))?;
        let mut active: face_person::ActiveModel = existing.into();

        if let Some(n) = name {
            active.name = Set(n);
        }
        if let Some(ni) = name_initials {
            active.name_initials = Set(Some(ni));
        }
        if let Some(f) = max_score_feature_id {
            active.max_score_feature_id = Set(f);
        }
        if let Some(s) = max_score {
            active.max_score = Set(s);
        }
        if let Some(c) = total_photo_count {
            active.total_photo_count = Set(c);
        }
        if let Some(e) = centroid_embedding {
            active.centroid_embedding = Set(e);
        }
        if let Some(w) = total_weight_count {
            active.total_weight_count = Set(w);
        }
        active.updated_at = Set(chrono::Utc::now().into());

        active.update(db).await.map_internal_err("更新失败")
    }

    /// 删除人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 人物ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        face_person::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_internal_err("删除人物失败")?;
        Ok(())
    }

    /// 根据首字母搜索人物（支持游标分页）
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `keyword`: 搜索关键词（首字母）
    /// - `cursor`: 复合游标
    /// - `size`: 返回数量
    /// 
    /// # 返回
    /// 返回匹配的人物列表
    pub async fn search_by_keyword(
        db: &DatabaseConnection,
        keyword: &str,
        cursor: Option<&PersonCursor>,
        size: u64,
    ) -> Result<Vec<face_person::Model>, AppError> {
        let keyword_lower = keyword.to_lowercase();

        let mut query = face_person::Entity::find()
            .filter(
                Condition::any()
                    .add(face_person::Column::NameInitials.like(format!("{}%", keyword_lower)))
                    .add(face_person::Column::Name.like(format!("{}%", keyword)))
            )
            .order_by_desc(face_person::Column::TotalPhotoCount)
            .order_by_desc(face_person::Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            query = query.filter(
                Condition::any()
                    .add(face_person::Column::TotalPhotoCount.lt(c.total_photo_count))
                    .add(
                        Condition::all()
                            .add(face_person::Column::TotalPhotoCount.eq(c.total_photo_count))
                            .add(face_person::Column::Id.lt(c.id))
                    )
            );
        }

        query.all(db).await.map_internal_err("搜索失败")
    }
}
