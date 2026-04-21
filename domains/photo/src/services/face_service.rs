use axum::body::Bytes;
use chrono::Utc;
use common::constants::redis_keys::photo::face_person_name;
use common::error::AppError;
use common::models::{FaceBBoxPixels, ImageToken};
use common::utils::{RedisExt, ResultExt, TokenCipher};
use deadpool_redis::Pool;
use entities::{face_feature, face_person, DrVector};
use face_engine::{FaceAligner, FaceEngine, LazyFaceEngine};
use sea_orm::{EntityTrait, Set, TransactionTrait};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::info;

use crate::clustering::union_find::{filter_valid_seeds, grow_stage, UnionFind};
use crate::clustering::vector_utils;
use crate::mappers::{FaceFeatureMapper, FacePersonMapper, PhotoMapper};
use crate::models::face::{
    FacePersonSimpleVO, FacePersonVO, FeatureNode, PersonCluster, PersonCursor,
};
use crate::models::photo::CursorPageVO;
use crate::services::photo_service::FaceTask;
use crate::utils::pinyin::to_pinyin_initials;

pub struct FaceService;

impl FaceService {
    /// 检测并识别人脸
    /// 
    /// 对照片进行人脸检测，提取特征向量并保存
    /// 过滤掉置信度低和面积小的人脸
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `face_engine`: 人脸引擎（检测器和特征提取器）
    /// - `photo_id`: 照片ID
    /// - `image_bytes`: 图片字节数据
    /// - `img_width`: 图片宽度（像素）
    /// - `img_height`: 图片高度（像素）
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn detect_and_recognize(
        db: &sea_orm::DatabaseConnection,
        face_engine: &FaceEngine,
        photo_id: i64,
        image_bytes: Bytes,
        img_width: u32,
        img_height: u32,
    ) -> Result<(), AppError> {
        info!("开始检测人脸, photo_id: {}", photo_id);

        let (img_width, img_height) = (img_width as f32, img_height as f32);

        let detections = face_engine
            .detect_faces(&image_bytes)
            .map_internal_err("人脸检测失败")?;

        info!("照片 {} 检测到 {} 张人脸", photo_id, detections.len());

        if detections.is_empty() {
            return Ok(());
        }

        let mut face_features = Vec::new();

        for detection in detections {
            let raw_bbox = &detection.bbox;
            let px_area = (raw_bbox.w * img_width) * (raw_bbox.h * img_height);

            if detection.score < 0.65
                || px_area < 160.0 * 160.0
                || raw_bbox.w < 0.05
                || raw_bbox.h < 0.05
            {
                continue;
            }

            let aligned_face = FaceAligner::align(&image_bytes, &detection.landmarks)
                .map_internal_err("人脸对齐失败")?;

            let embedding = face_engine
                .extract_embedding(&aligned_face)
                .map_internal_err("特征提取失败")?;

            let norm_embedding = vector_utils::l2_normalize(&embedding);
            let dr_vector = DrVector::new(norm_embedding.to_vec());

            let bbox_value = serde_json::json!({
                "x": detection.bbox.x,
                "y": detection.bbox.y,
                "w": detection.bbox.w,
                "h": detection.bbox.h
            });

            face_features.push(face_feature::ActiveModel {
                photo_id: Set(photo_id),
                person_id: Set(None),
                embedding: Set(dr_vector),
                bbox: Set(bbox_value),
                score: Set(detection.score),
                ..Default::default()
            });
        }

        if !face_features.is_empty() {
            let count = face_features.len();
            face_feature::Entity::insert_many(face_features)
                .exec(db)
                .await
                .map_internal_err("批量保存人脸特征失败")?;
            info!("照片 {} 处理完成，入库 {} 张人脸", photo_id, count);
        } else {
            info!("照片 {} 处理完成，无有效人脸", photo_id);
        }

        Ok(())
    }

    /// 执行人脸聚类
    /// 
    /// 使用Union-Find算法对人脸特征进行聚类
    /// 通过种子点生长阶段优化聚类结果
    /// 将聚类结果同步到数据库
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `seed_radius`: 种子点半径阈值
    /// - `min_points`: 最小聚类点数
    /// 
    /// # 返回
    /// 返回聚类结果列表
    pub async fn perform_clustering(
        db: &sea_orm::DatabaseConnection,
        seed_radius: f32,
        min_points: usize,
    ) -> Result<Vec<PersonCluster>, AppError> {
        info!("开始执行人脸聚类");

        let features = FaceFeatureMapper::find_all_ordered(db).await?;

        if features.len() < min_points {
            return Ok(vec![]);
        }

        let nodes: Vec<FeatureNode> = features
            .iter()
            .map(|f| FeatureNode {
                id: f.id,
                photo_id: f.photo_id,
                embedding: f.embedding.to_vec(),
                score: f.score,
                person_id: f.person_id,
            })
            .collect();

        let clusters = UnionFind::cluster(&nodes, seed_radius as f64);
        let mut seeds = filter_valid_seeds(clusters, &nodes, min_points);

        grow_stage(&mut seeds, &nodes, 0.75, true);
        grow_stage(&mut seeds, &nodes, 0.85, false);

        Self::sync_to_db(db, &seeds).await?;

        info!("聚类完成，共 {} 个人物", seeds.len());

        Ok(seeds)
    }

    /// 将聚类结果同步到数据库
    /// 
    /// 为每个聚类创建人物记录，并更新特征的关联
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `seeds`: 聚类结果列表
    async fn sync_to_db(
        db: &sea_orm::DatabaseConnection,
        seeds: &[PersonCluster],
    ) -> Result<(), AppError> {
        let seeds: Vec<PersonCluster> = seeds.to_vec();

        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                for seed in seeds {
                    if seed.member_ids.is_empty() {
                        continue;
                    }

                    // 根据 ID 列表查询特征信息
                    let features = FaceFeatureMapper::find_by_ids(txn, seed.member_ids.clone())
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    if features.is_empty() {
                        continue;
                    }

                    let best = features
                        .iter()
                        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
                        .unwrap();

                    let centroid = DrVector::new(seed.vector.clone());

                    let name = format!(
                        "人物_{}_{}",
                        Utc::now().format("%Y%m%d"),
                        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
                    );
                    let name_initials = Some(to_pinyin_initials(&name));

                    let person = FacePersonMapper::insert(
                        txn,
                        name,
                        name_initials,
                        best.id,
                        best.score,
                        features.len() as i64,
                        centroid,
                        seed.total_weight,
                    ).await.map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    let person_id = person.id;

                    for feature in &features {
                        FaceFeatureMapper::update_person_id(txn, feature.id, Some(person_id))
                            .await
                            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    }
                }

                Ok(())
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "同步聚类结果失败: {:?}", e);
            AppError::InternalServerError
        })
    }

    /// 获取人物列表（游标分页）
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `query`: 分页查询参数
    /// - `token_cipher`: 加密密钥
    /// 
    /// # 返回
    /// 返回分页人物列表
    pub async fn get_person_page(
        db: &sea_orm::DatabaseConnection,
        _redis: &Pool,
        query: crate::models::face::PersonPageQuery,
        token_cipher: &TokenCipher,
    ) -> Result<CursorPageVO<FacePersonVO, String>, AppError> {
        let size = query.size.unwrap_or(20) as u64;
        let decoded_cursor = query.cursor.as_ref().and_then(|s| PersonCursor::decode(s));
        
        let persons = FacePersonMapper::find_cursor_page(db, decoded_cursor.as_ref(), size + 1).await?;

        let has_more = persons.len() > size as usize;
        let persons: Vec<_> = persons.into_iter().take(size as usize).collect();

        let feature_ids: Vec<i64> = persons.iter().map(|p| p.max_score_feature_id).collect();
        let features = FaceFeatureMapper::find_by_ids(db, feature_ids).await?;
        let photo_ids: Vec<i64> = features.iter().map(|f| f.photo_id).collect();
        let photos = PhotoMapper::find_by_ids_map(db, photo_ids).await?;

        let records: Vec<FacePersonVO> = persons
            .into_iter()
            .filter_map(|p| {
                let feature = features.iter().find(|f| f.id == p.max_score_feature_id)?;
                let photo = photos.get(&feature.photo_id)?;
                let bbox: face_feature::FaceBBox = serde_json::from_value(feature.bbox.clone()).unwrap_or_else(|_| {
                    face_feature::FaceBBox { x: 0.0, y: 0.0, w: 0.1, h: 0.1 }
                });
                let x = (bbox.x * photo.width as f32) as i32;
                let y = (bbox.y * photo.height as f32) as i32;
                let w = (bbox.w * photo.width as f32) as i32;
                let h = (bbox.h * photo.height as f32) as i32;
                let cover_token = token_cipher
                    .encrypt(&ImageToken::crop(photo.file_id.clone(), FaceBBoxPixels { x, y, w, h }), Some(&photo.file_id))
                    .ok()?;

                Some(FacePersonVO {
                    id: p.id.to_string(),
                    name: p.name,
                    total_photo_count: Some(p.total_photo_count),
                    cover_token: Some(cover_token),
                })
            })
            .collect();

        let next_cursor = records.last().and_then(|r| {
            let person_id = r.id.parse().ok()?;
            let count = r.total_photo_count?;
            Some(PersonCursor {
                total_photo_count: count,
                id: person_id,
            }.encode())
        });

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    /// 获取所有人物简单列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// 
    /// # 返回
    /// 返回所有人物的简单信息列表（ID和名称）
    pub async fn get_all_person(
        db: &sea_orm::DatabaseConnection,
    ) -> Result<Vec<FacePersonSimpleVO>, AppError> {
        let persons = FacePersonMapper::find_all(db).await?;

        Ok(persons
            .iter()
            .map(|p| FacePersonSimpleVO {
                id: p.id.to_string(),
                name: p.name.clone(),
            })
            .collect())
    }

    /// 重命名人物
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池（用于清除缓存）
    /// - `person_id`: 人物ID
    /// - `new_name`: 新名称
    /// 
    /// # 返回
    /// 返回更新后的人物VO
    /// 
    /// # 错误
    /// - 名称已存在返回400错误
    pub async fn rename_person(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        person_id: i64,
        new_name: String,
    ) -> Result<FacePersonVO, AppError> {
        let existing = FacePersonMapper::find_by_name(db, &new_name).await?;

        if existing.is_some() {
            return Err(AppError::bad_request("人物名称已存在"));
        }

        let name_initials = to_pinyin_initials(&new_name);

        let person = FacePersonMapper::update(db, person_id, Some(new_name), Some(name_initials), None, None, None, None, None).await?;

        let _ = Self::invalidate_person_cache(redis, person_id).await;

        Ok(FacePersonVO {
            id: person.id.to_string(),
            name: person.name,
            total_photo_count: Some(person.total_photo_count),
            cover_token: None,
        })
    }

    /// 合并两个人物
    /// 
    /// 将源人物的所有特征转移到目标人物
    /// 重新计算目标人物的质心向量和照片数量
    /// 删除源人物
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池（用于清除缓存）
    /// - `source_id`: 源人物ID
    /// - `target_id`: 目标人物ID
    /// 
    /// # 返回
    /// 返回合并后的人物VO
    /// 
    /// # 错误
    /// - 源和目标相同返回400错误
    pub async fn merge_person(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        source_id: i64,
        target_id: i64,
    ) -> Result<FacePersonVO, AppError> {
        if source_id == target_id {
            return Err(AppError::bad_request("源人物和目标人物相同"));
        }

        let source = FacePersonMapper::find_by_id(db, source_id).await?;
        let target = FacePersonMapper::find_by_id(db, target_id).await?;

        let w1 = source.total_weight_count;
        let w2 = target.total_weight_count;
        let new_weight = w1 + w2;

        let c1 = source.centroid_embedding.to_vec();
        let c2 = target.centroid_embedding.to_vec();

        // 使用 ndarray 优化的加权合并
        let merged = vector_utils::weighted_merge(&c1, w1, &c2, w2);
        let merged = vector_utils::l2_normalize(&merged);
        let merged_embedding = DrVector::new(merged.to_vec());

        let new_photo_count = target.total_photo_count + source.total_photo_count;

        let target = db.transaction::<_, face_person::Model, sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                let target = FacePersonMapper::update(
                    txn,
                    target_id,
                    None,
                    None,
                    None,
                    None,
                    Some(new_photo_count),
                    Some(merged_embedding),
                    Some(new_weight),
                ).await.map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                let source_features = FaceFeatureMapper::find_by_person_id(txn, source_id).await.map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                for feature in source_features {
                    FaceFeatureMapper::update_person_id(txn, feature.id, Some(target_id))
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                }

                FacePersonMapper::delete_by_id(txn, source_id).await.map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                Ok(target)
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "合并人物失败: {:?}", e);
            AppError::InternalServerError
        })?;

        let _ = Self::invalidate_person_cache(redis, source_id).await;

        Ok(FacePersonVO {
            id: target.id.to_string(),
            name: target.name,
            total_photo_count: Some(target.total_photo_count),
            cover_token: None,
        })
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

    /// 获取人物详细信息
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `person_id`: 人物ID
    /// - `token_cipher`: 加密密钥
    /// 
    /// # 返回
    /// 返回人物VO，包含封面图token
    pub async fn get_person_info(
        db: &sea_orm::DatabaseConnection,
        person_id: i64,
        token_cipher: &TokenCipher,
    ) -> Result<FacePersonVO, AppError> {
        let person = FacePersonMapper::find_by_id(db, person_id).await?;

        let feature = FaceFeatureMapper::find_by_id(db, person.max_score_feature_id).await.ok();

        let cover_token = if let Some(f) = feature {
            let photo = PhotoMapper::find_by_id(db, f.photo_id).await.ok();
            if let Some(p) = photo {
                let bbox: face_feature::FaceBBox = serde_json::from_value(f.bbox.clone()).unwrap_or_else(|_| {
                    face_feature::FaceBBox { x: 0.0, y: 0.0, w: 0.1, h: 0.1 }
                });
                let x = (bbox.x * p.width as f32) as i32;
                let y = (bbox.y * p.height as f32) as i32;
                let w = (bbox.w * p.width as f32) as i32;
                let h = (bbox.h * p.height as f32) as i32;
                token_cipher
                    .encrypt(&ImageToken::crop(p.file_id.clone(), FaceBBoxPixels { x, y, w, h }), Some(&p.file_id))
                    .ok()
            } else {
                None
            }
        } else {
            None
        };

        Ok(FacePersonVO {
            id: person.id.to_string(),
            name: person.name,
            total_photo_count: Some(person.total_photo_count),
            cover_token,
        })
    }

    /// 获取人物的照片列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `user_id`: 用户ID（用于查询收藏状态）
    /// - `person_id`: 人物ID
    /// - `cursor`: 游标值（特征ID）
    /// - `size`: 每页数量
    /// - `token_cipher`: 加密密钥
    /// 
    /// # 返回
    /// 返回分页的照片列表
    pub async fn get_person_photo(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        user_id: i64,
        person_id: i64,
        cursor: Option<i64>,
        size: u32,
        token_cipher: &TokenCipher,
    ) -> Result<CursorPageVO<crate::models::photo::PhotoVO, i64>, AppError> {
        let features = FaceFeatureMapper::find_cursor_page(db, person_id, cursor, (size + 1) as u64).await?;

        let has_more = features.len() > size as usize;
        let features: Vec<_> = features.into_iter().take(size as usize).collect();

        let photo_ids: Vec<i64> = features.iter().map(|f| f.photo_id).collect();
        let photo_map = PhotoMapper::find_by_ids_map(db, photo_ids.clone()).await?;

        let favorite_collection_id = crate::services::CollectionService::get_favorite_collection_id(db, redis, user_id).await?;
        let favorited_photo_ids = crate::mappers::CollectionPhotoMapper::exists_in_collection(db, favorite_collection_id, &photo_ids).await?.into_iter().collect::<std::collections::HashSet<i64>>();

        let next_cursor = features.last().map(|f| f.id);

        let records: Vec<crate::models::photo::PhotoVO> = features
            .into_iter()
            .filter_map(|f| {
                let p = photo_map.get(&f.photo_id)?;
                let (thumbnail_token, preview_token, original_token) = 
                    crate::models::photo::PhotoVO::generate_tokens(&p.file_id, token_cipher);
                
                Some(crate::models::photo::PhotoVO {
                    id: p.id.to_string(),
                    name: p.name.clone(),
                    width: p.width,
                    height: p.height,
                    size: p.size,
                    created_at: p.created_at.with_timezone(&Utc),
                    is_favorited: Some(favorited_photo_ids.contains(&p.id)),
                    is_collected: None,
                    thumbnail_token,
                    preview_token,
                    original_token,
                })
            })
            .collect();

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    /// 删除人物
    /// 
    /// 将人物关联的所有特征解除关联，然后删除人物
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `person_id`: 人物ID
    /// 
    /// # 返回
    /// 成功返回true
    pub async fn delete_person(db: &sea_orm::DatabaseConnection, person_id: i64) -> Result<bool, AppError> {
        let _person = FacePersonMapper::find_by_id(db, person_id).await?;

        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                let features = FaceFeatureMapper::find_by_person_id(txn, person_id).await.map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                for feature in features {
                    FaceFeatureMapper::update_person_id(txn, feature.id, None)
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                }

                FacePersonMapper::delete_by_id(txn, person_id).await.map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                Ok(())
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "删除人物失败: {:?}", e);
            AppError::InternalServerError
        })?;

        Ok(true)
    }

    /// 搜索人物（游标分页）
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `query`: 搜索查询参数
    /// - `token_cipher`: 加密密钥
    /// 
    /// # 返回
    /// 返回匹配的人物列表
    pub async fn search_person(
        db: &sea_orm::DatabaseConnection,
        query: crate::models::face::PersonSearchQuery,
        token_cipher: &TokenCipher,
    ) -> Result<CursorPageVO<FacePersonVO, String>, AppError> {
        let size = query.size.unwrap_or(20) as u64;
        let decoded_cursor = query.cursor.as_ref().and_then(|s| PersonCursor::decode(s));

        let persons = FacePersonMapper::search_by_keyword(db, &query.keyword, decoded_cursor.as_ref(), size + 1).await?;

        let has_more = persons.len() > size as usize;
        let persons: Vec<_> = persons.into_iter().take(size as usize).collect();

        if query.detailed {
            let feature_ids: Vec<i64> = persons.iter().map(|p| p.max_score_feature_id).collect();
            let features = FaceFeatureMapper::find_by_ids(db, feature_ids).await?;
            let photo_ids: Vec<i64> = features.iter().map(|f| f.photo_id).collect();
            let photos = PhotoMapper::find_by_ids_map(db, photo_ids).await?;

            let records: Vec<FacePersonVO> = persons
                .into_iter()
                .filter_map(|p| {
                    let feature = features.iter().find(|f| f.id == p.max_score_feature_id)?;
                    let photo = photos.get(&feature.photo_id)?;
                    let bbox: face_feature::FaceBBox = serde_json::from_value(feature.bbox.clone()).unwrap_or_else(|_| {
                        face_feature::FaceBBox { x: 0.0, y: 0.0, w: 0.1, h: 0.1 }
                    });
                    let x = (bbox.x * photo.width as f32) as i32;
                    let y = (bbox.y * photo.height as f32) as i32;
                    let w = (bbox.w * photo.width as f32) as i32;
                    let h = (bbox.h * photo.height as f32) as i32;
                    let cover_token = token_cipher
                        .encrypt(&ImageToken::crop(photo.file_id.clone(), FaceBBoxPixels { x, y, w, h }), Some(&photo.file_id))
                        .ok()?;

                    Some(FacePersonVO {
                        id: p.id.to_string(),
                        name: p.name,
                        total_photo_count: Some(p.total_photo_count),
                        cover_token: Some(cover_token),
                    })
                })
                .collect();

            let next_cursor = records.last().and_then(|r| {
                let person_id = r.id.parse().ok()?;
                let count = r.total_photo_count?;
                Some(PersonCursor {
                    total_photo_count: count,
                    id: person_id,
                }.encode())
            });

            Ok(CursorPageVO {
                records,
                next_cursor,
                has_more,
            })
        } else {
            let records: Vec<FacePersonVO> = persons
                .into_iter()
                .map(|p| FacePersonVO {
                    id: p.id.to_string(),
                    name: p.name,
                    total_photo_count: Some(p.total_photo_count),
                    cover_token: None,
                })
                .collect();

            let next_cursor = records.last().and_then(|r| {
                let person_id = r.id.parse().ok()?;
                let count = r.total_photo_count?;
                Some(PersonCursor {
                    total_photo_count: count,
                    id: person_id,
                }.encode())
            });

            Ok(CursorPageVO {
                records,
                next_cursor,
                has_more,
            })
        }
    }

    /// 处理人脸检测任务的后台任务
    /// 
    /// 从通道接收照片上传任务，执行人脸检测和聚类
    /// 使用懒加载引擎，模型在第一次请求时加载，闲置10分钟后自动释放
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `rx`: 人脸检测任务接收通道
    /// - `lazy_engine`: 懒加载人脸引擎
    pub async fn process_face_tasks(
        db: &sea_orm::DatabaseConnection,
        mut rx: mpsc::Receiver<FaceTask>,
        lazy_engine: Arc<LazyFaceEngine>,
    ) {
        let cpus = num_cpus::get();
        let max_concurrency = (cpus / 2).max(1);
        let semaphore = Arc::new(Semaphore::new(max_concurrency));

        info!("人脸处理任务处理器启动, cpu核心数为: {}, 最大并发数为: {}", cpus, max_concurrency);

        while let Some(task) = rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db_clone = db.clone();
            let lazy_engine_clone = lazy_engine.clone();

            info!("处理照片, id为 {}", task.photo_id);

            tokio::spawn(async move {
                let engine = match lazy_engine_clone.get_or_load().await {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::error!("加载人脸模型失败: {}", e);
                        return;
                    }
                };

                let res = tokio::task::spawn_blocking(move || {
                    let _permit = permit;

                    let handle = tokio::runtime::Handle::current();
                    handle.block_on(async {
                        if let Err(e) = Self::detect_and_recognize(
                            &db_clone,
                            &engine,
                            task.photo_id,
                            task.image_bytes,
                            task.img_width,
                            task.img_height,
                        ).await {
                            tracing::error!("人脸处理时出现问题, 照片id为: {}, 错误为: {}", task.photo_id, e);
                        }
                    })
                }).await;

                if let Err(e) = res {
                    tracing::error!("人脸处理任务出现错误: {:?}", e);
                }
            });
        }
    }
}
