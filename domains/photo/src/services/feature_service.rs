use crate::clustering::vector_utils;
use crate::mappers::{FaceFeatureMapper, FacePersonMapper};
use crate::models::face::FaceFeatureVO;
use common::constants::redis_keys::photo::face_person_name;
use common::error::AppError;
use common::ext::RedisExt;
use deadpool_redis::Pool;
use entities::{Embedding512, face_feature};
use std::collections::HashMap;

use crate::state::PhotoState;

pub struct FeatureService;

impl FeatureService {
    /// 删除单个人脸特征并更新人物统计（减量计算）
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `feature`: 已查询的特征模型（避免重复查询）
    ///
    /// # 错误
    /// - `AppError`: 删除特征或更新人物统计失败
    pub async fn delete_feature_with_decrement(
        state: &PhotoState,
        feature: face_feature::Model,
    ) -> Result<(), AppError> {
        let person_id = feature.person_id;

        FaceFeatureMapper::delete_by_id(&state.db, feature.id).await?;

        if let Some(pid) = person_id {
            Self::decrement_person_stats(state, pid, &feature).await?;
        }

        Ok(())
    }

    /// 从人物统计中减去特征贡献（减量计算）
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `person_id`: 人物ID
    /// - `feature`: 被删除的特征
    ///
    /// # 错误
    /// - `AppError`: 查询或更新人物统计失败
    async fn decrement_person_stats(
        state: &PhotoState,
        person_id: i64,
        feature: &face_feature::Model,
    ) -> Result<(), AppError> {
        let person = match FacePersonMapper::query_by_id(&state.db, person_id).await {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };

        if person.total_photo_count <= 1 {
            FacePersonMapper::delete_by_id(&state.db, person_id).await?;
            Self::invalidate_person_cache(&state.redis, person_id).await?;
            return Ok(());
        }

        let old_weight = person.total_weight_count;
        let centroid = person.centroid_embedding.to_vec();
        let embedding = feature.embedding.to_vec();

        // 使用 ndarray 优化的减量质心计算
        let new_centroid = vector_utils::decrement_centroid(&centroid, old_weight, &embedding);
        let new_centroid = vector_utils::l2_normalize(&new_centroid);

        let (max_feature_id, max_score) = if person.max_score_feature_id == feature.id {
            let remaining = FaceFeatureMapper::query_by_person_id(&state.db, person_id).await?;
            let best = remaining
                .iter()
                .filter(|f| f.id != feature.id)
                .max_by(|a, b| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            match best {
                Some(b) => (b.id, b.score),
                None => {
                    FacePersonMapper::delete_by_id(&state.db, person_id).await?;
                    Self::invalidate_person_cache(&state.redis, person_id).await?;
                    return Ok(());
                }
            }
        } else {
            (person.max_score_feature_id, person.max_score)
        };

        FacePersonMapper::update(
            &state.db,
            person_id,
            None,
            None,
            Some(max_feature_id),
            Some(max_score),
            Some(person.total_photo_count - 1),
            Some(Embedding512::new(new_centroid.to_vec())),
            Some(old_weight - 1.0),
        )
        .await?;

        Self::invalidate_person_cache(&state.redis, person_id).await?;
        Ok(())
    }

    /// 删除单个人脸特征并重新计算人物统计（全量计算）
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `feature_id`: 特征ID
    ///
    /// # 错误
    /// - `AppError`: 查询或删除特征失败
    pub async fn delete_feature(state: &PhotoState, feature_id: i64) -> Result<(), AppError> {
        let feature = FaceFeatureMapper::query_by_id(&state.db, feature_id).await?;
        let person_id = feature.person_id;

        FaceFeatureMapper::delete_by_id(&state.db, feature_id).await?;

        if let Some(pid) = person_id {
            Self::recalculate_person_stats(state, pid).await?;
        }

        Ok(())
    }

    /// 重新计算人物统计信息（全量计算）
    ///
    /// 更新以下字段：total_photo_count、centroid_embedding、total_weight_count、max_score_feature_id、max_score。
    /// 如果人物没有关联特征，则删除人物。
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `person_id`: 人物ID
    ///
    /// # 错误
    /// - `AppError`: 查询特征或更新人物统计失败
    pub async fn recalculate_person_stats(
        state: &PhotoState,
        person_id: i64,
    ) -> Result<(), AppError> {
        let features = FaceFeatureMapper::query_by_person_id(&state.db, person_id).await?;

        if features.is_empty() {
            FacePersonMapper::delete_by_id(&state.db, person_id).await?;
            Self::invalidate_person_cache(&state.redis, person_id).await?;
            return Ok(());
        }

        // 使用 ndarray 优化的质心计算（避免内存拷贝）
        let embeddings: Vec<&[f32]> = features.iter().map(|f| f.embedding.as_slice()).collect();
        let centroid = vector_utils::calculate_centroid(&embeddings);
        let centroid = vector_utils::l2_normalize(&centroid);
        let centroid_embedding = Embedding512::new(centroid.to_vec());

        let total_weight = features.len() as f32;

        let best_feature = features
            .iter()
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        FacePersonMapper::update(
            &state.db,
            person_id,
            None,
            None,
            Some(best_feature.id),
            Some(best_feature.score),
            Some(features.len() as i64),
            Some(centroid_embedding),
            Some(total_weight),
        )
        .await?;

        Self::invalidate_person_cache(&state.redis, person_id).await?;

        Ok(())
    }

    /// 清除人物缓存
    ///
    /// # 参数
    /// - `redis`: Redis 连接池
    /// - `person_id`: 人物ID
    ///
    /// # 错误
    /// - `AppError`: Redis 删除操作失败（已忽略错误）
    async fn invalidate_person_cache(redis: &Pool, person_id: i64) -> Result<(), AppError> {
        let key = face_person_name(person_id);
        redis.delete(&key).await.ok();
        Ok(())
    }

    /// 获取照片中的人脸特征列表
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回人脸特征 VO 列表，包含人物名称
    ///
    /// # 错误
    /// - `AppError`: 查询特征或人物名称失败
    pub async fn get_photo_features(
        state: &PhotoState,
        photo_id: i64,
    ) -> Result<Vec<FaceFeatureVO>, AppError> {
        let features = FaceFeatureMapper::query_by_photo_id(&state.db, photo_id).await?;

        if features.is_empty() {
            return Ok(vec![]);
        }

        let person_ids: Vec<i64> = features.iter().filter_map(|f| f.person_id).collect();

        let person_names = if !person_ids.is_empty() {
            Self::get_person_names_batch(&state.db, &person_ids).await?
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

                let bbox: face_feature::FaceBBox = serde_json::from_value(f.bbox.clone())
                    .unwrap_or(face_feature::FaceBBox {
                        x: 0.0,
                        y: 0.0,
                        w: 0.1,
                        h: 0.1,
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
    /// - `person_ids`: 人物ID列表
    ///
    /// # 返回
    /// 返回以人物 ID 为键、名称为值的 HashMap
    ///
    /// # 错误
    /// - `AppError`: 查询人物列表失败
    async fn get_person_names_batch(
        db: &sea_orm::DatabaseConnection,
        person_ids: &[i64],
    ) -> Result<HashMap<i64, String>, AppError> {
        let persons = FacePersonMapper::query_by_ids(db, person_ids).await?;
        Ok(persons.into_iter().map(|p| (p.id, p.name)).collect())
    }

    /// 更改人脸特征的人物归属
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `feature_id`: 人脸特征ID
    /// - `person_id`: 目标人物ID
    ///
    /// # 错误
    /// - `AppError`: 查询特征、人物或更新归属失败
    pub async fn change_face_belonging(
        state: &PhotoState,
        feature_id: i64,
        person_id: i64,
    ) -> Result<(), AppError> {
        let feature = FaceFeatureMapper::query_by_id(&state.db, feature_id).await?;
        let _person = FacePersonMapper::query_by_id(&state.db, person_id).await?;

        let old_person_id = feature.person_id;

        FaceFeatureMapper::update_person_id(&state.db, feature_id, Some(person_id)).await?;

        // 重新计算原人物统计
        if let Some(old_pid) = old_person_id {
            Self::recalculate_person_stats(state, old_pid).await?;
        }

        // 重新计算目标人物统计
        Self::recalculate_person_stats(state, person_id).await?;

        Ok(())
    }
}
