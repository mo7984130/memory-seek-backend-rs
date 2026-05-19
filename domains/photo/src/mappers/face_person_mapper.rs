use std::collections::HashMap;

use common::error::AppError;
use common::utils::ResultExt;
use entities::{
    Embedding512,
    face_feature::FEATURE_DIM,
    face_person::{self, Column, Entity},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
    sea_query::{CaseStatement, Expr, SimpleExpr},
};
use tracing::warn;

use crate::{mappers::FaceFeatureMapper, models::face::PersonCursor};

pub struct FacePersonMapper;

impl FacePersonMapper {
    /// 根据ID查询单个人物
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `id`: 人物ID
    ///
    /// # 返回
    /// 返回人物模型
    ///
    /// # 错误
    /// - `AppError::NotFound`: 人物不存在
    pub async fn query_by_id(
        db: &DatabaseConnection,
        id: i64,
    ) -> Result<face_person::Model, AppError> {
        face_person::Entity::find_by_id(id)
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?
            .ok_or_else(|| AppError::not_found("人物不存在"))
    }

    /// 查询所有人物
    ///
    /// # 参数
    /// - `db`: 数据库连接
    ///
    /// # 返回
    /// 返回所有人物列表，按名称排序
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_all(db: &DatabaseConnection) -> Result<Vec<face_person::Model>, AppError> {
        face_person::Entity::find()
            .order_by_asc(face_person::Column::Name)
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
    }

    /// 根据名称查询人物
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `name`: 人物名称
    ///
    /// # 返回
    /// 返回匹配的人物，不存在返回None
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<face_person::Model>, AppError> {
        face_person::Entity::find()
            .filter(face_person::Column::Name.eq(name))
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_cursor_page(
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
                            .add(face_person::Column::Id.lt(c.id)),
                    ),
            );
        }

        query
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
    }

    /// 根据ID列表批量查询人物
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `ids`: 人物ID列表
    ///
    /// # 返回
    /// 返回匹配的人物列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn query_by_ids(
        db: &impl ConnectionTrait,
        ids: &[i64],
    ) -> Result<Vec<face_person::Model>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        face_person::Entity::find()
            .filter(face_person::Column::Id.is_in(ids.iter().copied()))
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
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
    ///
    /// # 错误
    /// - `AppError`: 数据库插入失败
    pub async fn insert<C: ConnectionTrait>(
        db: &C,
        name: String,
        name_initials: Option<String>,
        max_score_feature_id: i64,
        max_score: f32,
        total_photo_count: i64,
        centroid_embedding: Embedding512,
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

        person
            .insert(db)
            .await
            .trace_internal_err("db_insert_err", "创建人物失败")
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
    ///
    /// # 错误
    /// - `AppError::NotFound`: 人物不存在
    /// - `AppError`: 数据库更新失败
    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: i64,
        name: Option<String>,
        name_initials: Option<String>,
        max_score_feature_id: Option<i64>,
        max_score: Option<f32>,
        total_photo_count: Option<i64>,
        centroid_embedding: Option<Embedding512>,
        total_weight_count: Option<f32>,
    ) -> Result<face_person::Model, AppError> {
        let existing = face_person::Entity::find_by_id(id)
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?
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

        active
            .update(db)
            .await
            .trace_internal_err("db_update_err", "更新失败")
    }

    /// 删除人物
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `id`: 人物ID
    ///
    /// # 错误
    /// - `AppError`: 数据库删除失败
    pub async fn delete_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), AppError> {
        face_person::Entity::delete_by_id(id)
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "删除人物失败")?;
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
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
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
                    .add(face_person::Column::Name.like(format!("{}%", keyword))),
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
                            .add(face_person::Column::Id.lt(c.id)),
                    ),
            );
        }

        query
            .all(db)
            .await
            .trace_internal_err("db_query_err", "搜索失败")
    }

    /// 批量减少人物特征的权重
    ///
    /// 调用前需先删除特征记录，因为寻找最高分特征时是全局查找。
    /// 重新计算质心、权重、照片数及最高分特征。
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `features`: 特征列表，每项为 `(feature_id, person_id, embedding, score)`
    ///
    /// # 错误
    /// - `AppError`: 数据库查询或更新失败
    pub async fn decr_by_features(
        db: &impl ConnectionTrait,
        features: &[(i64, Option<i64>, Vec<f32>, f32)], // (feature_id, person_id, embedding, score)
    ) -> Result<(), AppError> {
        struct FeatureInfo {
            feature_id: i64,
            embedding: Vec<f32>,
            weight: f32,
        }

        // 去除掉person_id 为空的feature
        // 按照person_id 分组
        // features = (feature_id, embedding, score)
        let mut person_features: HashMap<i64, Vec<FeatureInfo>> = HashMap::new();
        for (feature_id, person_id, embedding, score) in features {
            if let Some(pid) = person_id {
                person_features.entry(*pid).or_default().push(FeatureInfo {
                    feature_id: *feature_id,
                    embedding: embedding.clone(),
                    weight: *score,
                });
            }
        }
        if person_features.is_empty() {
            return Ok(());
        }

        // 查询person的信息
        let persons = Self::query_by_ids(db, &person_features.keys().copied().collect::<Vec<_>>())
            .await
            .trace_internal_err("db_query_err", "获取人物信息错误")?;

        // 减量计算
        let mut updates: Vec<UpdateInfo> = Vec::with_capacity(persons.len());
        for person in &persons {
            let features = match person_features.get(&person.id) {
                Some(v) => v,
                None => continue,
            };

            // 减去的总权重
            let removed_weight: f32 = features.iter().map(|f| f.weight).sum();
            // 减去的加权向量
            let mut removed_weighted_embedding: Vec<f32> = vec![0.0; FEATURE_DIM];
            for feature in features {
                for (i, val) in feature.embedding.iter().enumerate() {
                    removed_weighted_embedding[i] += val * feature.weight;
                }
            }

            let mut new_total_weight = person.total_weight_count - removed_weight;
            if new_total_weight < 0.0 {
                warn!(
                    person_id = person.id,
                    new_total_weight,
                    feature_ids = ?features.iter().map(|f| f.feature_id).collect::<Vec<_>>(),
                    "重新计算权重的时候, 权重小于0"
                );
                new_total_weight = 0.0;
            }
            let mut new_total_photo_count = person.total_photo_count - features.len() as i64;
            if new_total_photo_count < 0 {
                warn!(
                    person_id = person.id,
                    new_total_photo_count,
                    feature_ids = ?features.iter().map(|f| f.feature_id).collect::<Vec<_>>(),
                    "重新计算照片数量的时候, 数量小于0"
                );
                new_total_photo_count = 0;
            }

            // 计算质心
            let new_centroid = if new_total_weight <= 0.0 {
                vec![0.0f32; FEATURE_DIM]
            } else {
                let old_centroid = &person.centroid_embedding;
                let old_weight = &person.total_weight_count;
                (0..FEATURE_DIM)
                    .map(|i| {
                        (old_centroid[i] * old_weight - removed_weighted_embedding[i])
                            / new_total_weight
                    })
                    .collect::<Vec<_>>()
            };

            // 当删除的照片里面有人物的max_score_feature时, 需要重新计算max_score_feature
            updates.push(UpdateInfo {
                person_id: person.id,
                new_centroid: Embedding512::from(new_centroid),
                new_total_weight,
                new_total_photo_count,
                need_update_max_score: features
                    .iter()
                    .any(|f| f.feature_id == person.max_score_feature_id),
                new_max_score: None,
            });
        }

        // 计算max_score_feature_id
        let need_refresh_person_ids: Vec<i64> = updates
            .iter()
            .filter(|u| u.need_update_max_score)
            .map(|u| u.person_id)
            .collect();

        let top_map =
            FaceFeatureMapper::query_max_score_features_by_person_ids(db, need_refresh_person_ids)
                .await?;

        for update in &mut updates {
            if !update.need_update_max_score {
                continue;
            }
            update.new_max_score = Some(
                top_map
                    .get(&update.person_id)
                    .copied()
                    .flatten()
                    .unwrap_or((-1, 0.0)), // 该 person 已无剩余 feature
            );
        }

        // 批量更新
        // max_score
        Self::update_persons(db, updates).await?;

        Ok(())
    }

    // 批量更新人物的质心、权重、照片数及最高分特征
    async fn update_persons(
        db: &impl ConnectionTrait,
        updates: Vec<UpdateInfo>,
    ) -> Result<(), AppError> {
        if updates.is_empty() {
            return Ok(());
        }

        let person_ids: Vec<i64> = updates.iter().map(|u| u.person_id).collect();

        // 构建 CASE WHEN 表达式
        let mut centroid_case = CaseStatement::new();
        let mut weight_case = CaseStatement::new();
        let mut photo_count_case = CaseStatement::new();
        // max_score 相关（只有需要更新的才加入 CASE）
        let mut max_score_case = CaseStatement::new();
        let mut max_score_feature_id_case = CaseStatement::new();
        let mut has_max_score_update = false;

        for update in &updates {
            let cond = Expr::col(Column::Id).eq(update.person_id);

            centroid_case = centroid_case.case(
                cond.clone(),
                SimpleExpr::from(Expr::value(update.new_centroid.clone())),
            );
            weight_case = weight_case.case(
                cond.clone(),
                SimpleExpr::from(Expr::value(update.new_total_weight)),
            );
            photo_count_case = photo_count_case.case(
                cond.clone(),
                SimpleExpr::from(Expr::value(update.new_total_photo_count)),
            );

            if let Some((feature_id, score)) = update.new_max_score {
                has_max_score_update = true;
                max_score_case =
                    max_score_case.case(cond.clone(), SimpleExpr::from(Expr::value(score)));
                max_score_feature_id_case =
                    max_score_feature_id_case.case(cond, SimpleExpr::from(Expr::value(feature_id)));
            }
        }

        // ELSE 保持原值
        centroid_case = centroid_case.finally(Expr::col(Column::CentroidEmbedding));
        weight_case = weight_case.finally(Expr::col(Column::TotalWeightCount));
        photo_count_case = photo_count_case.finally(Expr::col(Column::TotalPhotoCount));

        let mut query = Entity::update_many()
            .col_expr(
                Column::CentroidEmbedding,
                SimpleExpr::Case(Box::new(centroid_case)).into(),
            )
            .col_expr(
                Column::TotalWeightCount,
                SimpleExpr::Case(Box::new(weight_case)).into(),
            )
            .col_expr(
                Column::TotalPhotoCount,
                SimpleExpr::Case(Box::new(photo_count_case)).into(),
            )
            .col_expr(Column::UpdatedAt, Expr::current_timestamp().into());

        // 有需要更新 max_score 的才加入
        if has_max_score_update {
            max_score_case = max_score_case.finally(Expr::col(Column::MaxScore));
            max_score_feature_id_case =
                max_score_feature_id_case.finally(Expr::col(Column::MaxScoreFeatureId));

            query = query
                .col_expr(
                    Column::MaxScore,
                    SimpleExpr::Case(Box::new(max_score_case)).into(),
                )
                .col_expr(
                    Column::MaxScoreFeatureId,
                    SimpleExpr::Case(Box::new(max_score_feature_id_case)).into(),
                );
        }

        query
            .filter(Column::Id.is_in(person_ids))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "批量更新人物错误")?;

        Ok(())
    }
}

struct UpdateInfo {
    person_id: i64,
    new_centroid: Embedding512,
    new_total_weight: f32,
    new_total_photo_count: i64,
    need_update_max_score: bool,
    new_max_score: Option<(i64, f32)>,
}
