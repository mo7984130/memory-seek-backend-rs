use std::collections::HashMap;
use common::constants::redis_keys::photo::face_person_name;
use common::error::AppError;
use common::utils::RedisExt;
use deadpool_redis::Pool;
use entities::{face_feature, DrVector};
use crate::clustering::vector_utils;
use crate::mappers::{FaceFeatureMapper, FacePersonMapper};
use crate::models::face::FaceFeatureVO;

pub struct FeatureService;

impl FeatureService {
    /// 删除单个人脸特征并更新人物统计（减量计算）
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池（用于清除缓存）
    /// - `feature`: 已查询的特征模型（避免重复查询）
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_feature_with_decrement(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        feature: face_feature::Model,
    ) -> Result<(), AppError> {
        let person_id = feature.person_id;

        FaceFeatureMapper::delete_by_id(db, feature.id).await?;

        if let Some(pid) = person_id {
            Self::decrement_person_stats(db, redis, pid, &feature).await?;
        }

        Ok(())
    }

    /// 从人物统计中减去特征贡献（减量计算）
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `person_id`: 人物ID
    /// - `feature`: 被删除的特征
    async fn decrement_person_stats(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        person_id: i64,
        feature: &face_feature::Model,
    ) -> Result<(), AppError> {
        let person = match FacePersonMapper::find_by_id(db, person_id).await {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };

        if person.total_photo_count <= 1 {
            FacePersonMapper::delete_by_id(db, person_id).await?;
            Self::invalidate_person_cache(redis, person_id).await?;
            return Ok(());
        }

        let old_weight = person.total_weight_count;
        let centroid = person.centroid_embedding.to_vec();
        let embedding = feature.embedding.to_vec();

        // 使用 ndarray 优化的减量质心计算
        let new_centroid = vector_utils::decrement_centroid(&centroid, old_weight, &embedding);
        let new_centroid = vector_utils::l2_normalize(&new_centroid);

        let (max_feature_id, max_score) = if person.max_score_feature_id == feature.id {
            let remaining = FaceFeatureMapper::find_by_person_id(db, person_id).await?;
            let best = remaining
                .iter()
                .filter(|f| f.id != feature.id)
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
            match best {
                Some(b) => (b.id, b.score),
                None => {
                    FacePersonMapper::delete_by_id(db, person_id).await?;
                    Self::invalidate_person_cache(redis, person_id).await?;
                    return Ok(());
                }
            }
        } else {
            (person.max_score_feature_id, person.max_score)
        };

        FacePersonMapper::update(
            db,
            person_id,
            None,
            None,
            Some(max_feature_id),
            Some(max_score),
            Some(person.total_photo_count - 1),
            Some(DrVector::new(new_centroid.to_vec())),
            Some(old_weight - 1.0),
        ).await?;

        Self::invalidate_person_cache(redis, person_id).await?;
        Ok(())
    }

    /// 删除单个人脸特征并重新计算人物统计（全量计算）
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池（用于清除缓存）
    /// - `feature_id`: 特征ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn delete_feature(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        feature_id: i64,
    ) -> Result<(), AppError> {
        let feature = FaceFeatureMapper::find_by_id(db, feature_id).await?;
        let person_id = feature.person_id;

        FaceFeatureMapper::delete_by_id(db, feature_id).await?;

        if let Some(pid) = person_id {
            Self::recalculate_person_stats(db, redis, pid).await?;
        }

        Ok(())
    }

    /// 重新计算人物统计信息（全量计算）
    /// 
    /// 更新以下字段：
    /// - total_photo_count: 特征数量
    /// - centroid_embedding: 质心向量
    /// - total_weight_count: 总权重
    /// - max_score_feature_id: 最高分特征ID
    /// - max_score: 最高分
    /// 
    /// 如果人物没有关联特征，则删除人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `person_id`: 人物ID
    pub async fn recalculate_person_stats(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        person_id: i64,
    ) -> Result<(), AppError> {
        let features = FaceFeatureMapper::find_by_person_id(db, person_id).await?;

        if features.is_empty() {
            FacePersonMapper::delete_by_id(db, person_id).await?;
            Self::invalidate_person_cache(redis, person_id).await?;
            return Ok(());
        }

        // 使用 ndarray 优化的质心计算（避免内存拷贝）
        let embeddings: Vec<&[f32]> = features.iter().map(|f| f.embedding.as_slice()).collect();
        let centroid = vector_utils::calculate_centroid(&embeddings);
        let centroid = vector_utils::l2_normalize(&centroid);
        let centroid_embedding = DrVector::new(centroid.to_vec());

        let total_weight = features.len() as f32;

        let best_feature = features
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
            .unwrap();

        FacePersonMapper::update(
            db,
            person_id,
            None,
            None,
            Some(best_feature.id),
            Some(best_feature.score),
            Some(features.len() as i64),
            Some(centroid_embedding),
            Some(total_weight),
        ).await?;

        Self::invalidate_person_cache(redis, person_id).await?;

        Ok(())
    }

    /// 清除人物缓存
    /// 
    /// # 参数
    /// - `redis`: Redis连接池
    /// - `person_id`: 人物ID
    async fn invalidate_person_cache(redis: &Pool, person_id: i64) -> Result<(), AppError> {
        let key = face_person_name(person_id);
        redis.delete(&key).await.ok();
        Ok(())
    }

    /// 获取照片中的人脸特征列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池（用于缓存人物名称）
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 返回人脸特征VO列表，包含人物名称
    pub async fn get_photo_features(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        photo_id: i64,
    ) -> Result<Vec<FaceFeatureVO>, AppError> {
        let features = FaceFeatureMapper::find_by_photo_id(db, photo_id).await?;

        if features.is_empty() {
            return Ok(vec![]);
        }

        let person_ids: Vec<i64> = features.iter().filter_map(|f| f.person_id).collect();

        let person_names = if !person_ids.is_empty() {
            Self::get_person_names_batch(db, redis, &person_ids).await?
        } else {
            HashMap::new()
        };

        Ok(features
            .iter()
            .map(|f| {
                let person_name = f
                    .person_id
                    .and_then(|pid| person_names.get(&pid).cloned())
                    .unwrap_or_else(|| "未知人物".to_string());

                let bbox: face_feature::FaceBBox =
                    serde_json::from_value(f.bbox.clone()).unwrap_or_else(|_| {
                        face_feature::FaceBBox {
                            x: 0.0,
                            y: 0.0,
                            w: 0.1,
                            h: 0.1,
                        }
                    });

                FaceFeatureVO {
                    id: f.id.to_string(),
                    person_id: f.person_id.map(|id| id.to_string()),
                    person_name,
                    bbox: crate::models::face::FaceBBox {
                        x: bbox.x,
                        y: bbox.y,
                        w: bbox.w,
                        h: bbox.h,
                    },
                    score: f.score,
                }
            })
            .collect())
    }

    /// 批量获取人物名称
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `_redis`: Redis连接池（暂未使用缓存）
    /// - `person_ids`: 人物ID列表
    /// 
    /// # 返回
    /// 返回以人物ID为键、名称为值的HashMap
    async fn get_person_names_batch(
        db: &sea_orm::DatabaseConnection,
        _redis: &Pool,
        person_ids: &[i64],
    ) -> Result<HashMap<i64, String>, AppError> {
        let persons = FacePersonMapper::find_by_ids(db, person_ids.to_vec()).await?;
        Ok(persons.into_iter().map(|p| (p.id, p.name)).collect())
    }

    /// 更改人脸特征的人物归属
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `feature_id`: 人脸特征ID
    /// - `person_id`: 目标人物ID
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn change_face_belonging(
        db: &sea_orm::DatabaseConnection,
        feature_id: i64,
        person_id: i64,
    ) -> Result<(), AppError> {
        let _feature = FaceFeatureMapper::find_by_id(db, feature_id).await?;
        let _person = FacePersonMapper::find_by_id(db, person_id).await?;

        FaceFeatureMapper::update_person_id(db, feature_id, Some(person_id)).await?;

        Ok(())
    }
}
