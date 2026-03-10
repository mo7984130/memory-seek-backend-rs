use chrono::Utc;
use common::error::AppError;
use common::utils::ResultExt;
use deadpool_redis::Pool;
use entities::{face_feature, face_person, photo, DrVector};
use face_engine::{FaceAligner, FaceEngine};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::info;

use crate::clustering::union_find::{filter_valid_seeds, grow_stage, UnionFind};
use crate::clustering::vector_utils;
use crate::models::face::{
    FaceFeatureVO, FacePersonSimpleVO, FacePersonVO, FeatureNode, PersonCluster,
};
use crate::models::photo::CursorPageVO;
use crate::services::photo_service::FaceTask;
use futures::future::join_all;
use img_url_generator::{ImageUrlGenerator, ImageUrlProvider};

pub struct FaceService;

impl FaceService {
    pub async fn detect_and_recognize(
        db: &DatabaseConnection,
        face_engine: &FaceEngine,
        photo_id: i64,
        image_bytes: Vec<u8>,
    ) -> Result<(), AppError> {
        info!("开始检测照片 {} 的人脸", photo_id);

        let detections = face_engine
            .detect_faces(&image_bytes)
            .map_internal_err("人脸检测失败")?;

        info!("在照片 {} 中检测到 {} 张人脸", photo_id, detections.len());

        for detection in detections {
            let px_area = detection.bbox.w * detection.bbox.h;
            if detection.score < 0.65 || px_area < 0.01 {
                continue;
            }

            let aligned_face = FaceAligner::align(&image_bytes, &detection.landmarks)
                .map_internal_err("人脸对齐失败")?;

            let embedding = face_engine
                .extract_embedding(&aligned_face)
                .map_internal_err("特征提取失败")?;

            let embedding = vector_utils::l2_normalize(&embedding);
            let embedding = DrVector::new(embedding.to_vec());

            let bbox_json = serde_json::json!({
                "x": detection.bbox.x,
                "y": detection.bbox.y,
                "w": detection.bbox.w,
                "h": detection.bbox.h
            });

            let now = Utc::now();
            let feature = face_feature::ActiveModel {
                photo_id: Set(photo_id),
                person_id: Set(None),
                embedding: Set(embedding),
                bbox: Set(sea_orm::JsonValue::Object(bbox_json.as_object().unwrap().clone())),
                score: Set(detection.score),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };

            feature
                .insert(db)
                .await
                .map_internal_err("保存特征失败")?;
        }

        Ok(())
    }

    pub async fn perform_clustering(
        db: &DatabaseConnection,
        seed_radius: f64,
        min_points: usize,
    ) -> Result<Vec<PersonCluster>, AppError> {
        info!("开始执行人脸聚类");

        let features = face_feature::Entity::find()
            .order_by_asc(face_feature::Column::Id)
            .all(db)
            .await
            .map_internal_err("查询特征失败")?;

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

        let clusters = UnionFind::cluster(&nodes, seed_radius);
        let mut seeds = filter_valid_seeds(clusters, &nodes, min_points);

        grow_stage(&mut seeds, &nodes, 0.75, true);
        grow_stage(&mut seeds, &nodes, 0.85, false);

        Self::sync_to_db(db, &seeds).await?;

        info!("聚类完成，共 {} 个人物", seeds.len());

        Ok(seeds)
    }

    async fn sync_to_db(
        db: &DatabaseConnection,
        seeds: &[PersonCluster],
    ) -> Result<(), AppError> {
        for seed in seeds {
            if seed.member_nodes.is_empty() {
                continue;
            }

            let best = seed
                .member_nodes
                .iter()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
                .unwrap();

            let centroid = DrVector::new(seed.vector.clone());

            let now = Utc::now();

            let person = face_person::ActiveModel {
                name: Set(format!(
                    "人物_{}_{}",
                    now.format("%Y%m%d"),
                    uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
                )),
                max_score_feature_id: Set(best.id),
                max_score: Set(best.score),
                total_photo_count: Set(seed.member_nodes.len() as i64),
                centroid_embedding: Set(centroid),
                total_weight_count: Set(seed.total_weight),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };

            let person = person.insert(db).await.map_internal_err("创建人物失败")?;
            let person_id = person.id;

            for node in &seed.member_nodes {
                if let Some(feature) = face_feature::Entity::find_by_id(node.id)
                    .one(db)
                    .await
                    .map_internal_err("查询特征失败")?
                {
                    let mut active: face_feature::ActiveModel = feature.into();
                    active.person_id = Set(Some(person_id));
                    active.updated_at = Set(now.into());
                    let _ = active.update(db).await;
                }
            }
        }

        Ok(())
    }

    pub async fn get_person_page(
        db: &DatabaseConnection,
        cursor: Option<i64>,
        size: u32,
        img_url_generator: &ImageUrlProvider,
    ) -> Result<CursorPageVO<FacePersonVO, i64>, AppError> {
        let limit = size as u64 + 1;
        let mut query = face_person::Entity::find()
            .order_by_desc(face_person::Column::TotalPhotoCount)
            .limit(limit);

        if let Some(c) = cursor {
            query = query.filter(face_person::Column::TotalPhotoCount.lt(c));
        }

        let persons = query.all(db).await.map_internal_err("查询失败")?;

        let has_more = persons.len() > size as usize;
        let persons: Vec<_> = persons.into_iter().take(size as usize).collect();

        let feature_ids: Vec<i64> = persons.iter().map(|p| p.max_score_feature_id).collect();

        let features = face_feature::Entity::find()
            .filter(face_feature::Column::Id.is_in(feature_ids))
            .all(db)
            .await
            .map_internal_err("查询特征失败")?;

        let feature_map: HashMap<i64, _> = features.into_iter().map(|f| (f.id, f)).collect();

        let photo_ids: Vec<i64> = feature_map.values().map(|f| f.photo_id).collect();
        let photos = photo::Entity::find()
            .filter(photo::Column::Id.is_in(photo_ids))
            .all(db)
            .await
            .map_internal_err("查询照片失败")?;

        let photo_map: HashMap<i64, _> = photos.into_iter().map(|p| (p.id, p)).collect();

        let next_cursor = persons.last().map(|p| p.total_photo_count);

        let futures = persons.into_iter().map(|p| {
            let feature_opt = feature_map.get(&p.max_score_feature_id).cloned();
            let photo_opt = feature_opt.as_ref().and_then(|f| photo_map.get(&f.photo_id).cloned());
            let person_id = p.id;
            let person_name = p.name;
            let total_photo_count = p.total_photo_count;
            async move {
                if let Some(photo) = photo_opt {
                    if let Some(f) = feature_opt {
                        let bbox: face_feature::FaceBBox =
                            serde_json::from_value(f.bbox.clone()).unwrap_or_else(|_| {
                                face_feature::FaceBBox {
                                    x: 0.0,
                                    y: 0.0,
                                    w: 0.1,
                                    h: 0.1,
                                }
                            });

                        let cw = (bbox.w * photo.width as f32) as i64;
                        let ch = (bbox.h * photo.height as f32) as i64;
                        let cx = (bbox.x * photo.width as f32) as i64;
                        let cy = (bbox.y * photo.height as f32) as i64;

                        let cover_url = img_url_generator.crop(
                            photo.file_id.clone(),
                            cx as i32,
                            cy as i32,
                            cw as i32,
                            ch as i32,
                            200,
                        ).await;

                        return Some(FacePersonVO {
                            id: person_id.to_string(),
                            name: person_name,
                            total_photo_count,
                            cover_url: Some(cover_url),
                        });
                    }
                }
                None
            }
        });
        let records: Vec<FacePersonVO> = join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    pub async fn get_all_person(
        db: &DatabaseConnection,
    ) -> Result<Vec<FacePersonSimpleVO>, AppError> {
        let persons = face_person::Entity::find()
            .order_by_asc(face_person::Column::Name)
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(persons
            .iter()
            .map(|p| FacePersonSimpleVO {
                id: p.id.to_string(),
                name: p.name.clone(),
            })
            .collect())
    }

    pub async fn rename_person(
        db: &DatabaseConnection,
        redis: &Pool,
        person_id: i64,
        new_name: String,
    ) -> Result<FacePersonVO, AppError> {
        let existing = face_person::Entity::find()
            .filter(face_person::Column::Name.eq(&new_name))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        if existing.is_some() {
            return Err(AppError::bad_request("人物名称已存在"));
        }

        let person = face_person::Entity::find_by_id(person_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("人物不存在"))?;

        let mut active: face_person::ActiveModel = person.into();
        active.name = Set(new_name);
        active.updated_at = Set(Utc::now().into());

        let person = active.update(db).await.map_internal_err("更新失败")?;

        let _ = Self::invalidate_person_cache(redis, person_id).await;

        Ok(FacePersonVO {
            id: person.id.to_string(),
            name: person.name,
            total_photo_count: person.total_photo_count,
            cover_url: None,
        })
    }

    pub async fn merge_person(
        db: &DatabaseConnection,
        redis: &Pool,
        source_id: i64,
        target_id: i64,
    ) -> Result<FacePersonVO, AppError> {
        if source_id == target_id {
            return Err(AppError::bad_request("源人物和目标人物相同"));
        }

        let source = face_person::Entity::find_by_id(source_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("源人物不存在"))?;

        let target = face_person::Entity::find_by_id(target_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("目标人物不存在"))?;

        let w1 = source.total_weight_count;
        let w2 = target.total_weight_count;
        let new_weight = w1 + w2;

        let c1 = source.centroid_embedding.to_vec();
        let c2 = target.centroid_embedding.to_vec();

        let mut merged = vec![0.0f32; 512];
        for i in 0..512 {
            merged[i] = (c1[i] * w1 + c2[i] * w2) / new_weight;
        }
        let merged = vector_utils::l2_normalize(&merged);
        let merged_embedding = DrVector::new(merged.to_vec());

        let new_photo_count = target.total_photo_count + source.total_photo_count;

        let mut target_active: face_person::ActiveModel = target.into();
        target_active.centroid_embedding = Set(merged_embedding);
        target_active.total_weight_count = Set(new_weight);
        target_active.total_photo_count = Set(new_photo_count);
        target_active.updated_at = Set(Utc::now().into());

        let target = target_active.update(db).await.map_internal_err("更新失败")?;

        let source_features = face_feature::Entity::find()
            .filter(face_feature::Column::PersonId.eq(source_id))
            .all(db)
            .await
            .map_internal_err("查询源人物特征失败")?;

        for feature in source_features {
            let mut active: face_feature::ActiveModel = feature.into();
            active.person_id = Set(Some(target_id));
            active.updated_at = Set(Utc::now().into());
            let _ = active.update(db).await;
        }

        face_person::Entity::delete_by_id(source_id)
            .exec(db)
            .await
            .map_internal_err("删除源人物失败")?;

        let _ = Self::invalidate_person_cache(redis, source_id).await;

        Ok(FacePersonVO {
            id: target.id.to_string(),
            name: target.name,
            total_photo_count: target.total_photo_count,
            cover_url: None,
        })
    }

    async fn invalidate_person_cache(redis: &Pool, person_id: i64) -> Result<(), AppError> {
        let key = format!("photo:face:person:name:{}", person_id);
        let mut conn = redis.get().await.map_internal_err("Redis连接失败")?;
        use deadpool_redis::redis::AsyncCommands;
        let _: Option<()> = conn.del(&key).await.ok();
        Ok(())
    }

    pub async fn get_photo_features(
        db: &DatabaseConnection,
        redis: &Pool,
        photo_id: i64,
    ) -> Result<Vec<FaceFeatureVO>, AppError> {
        let features = face_feature::Entity::find()
            .filter(face_feature::Column::PhotoId.eq(photo_id))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

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

    async fn get_person_names_batch(
        db: &DatabaseConnection,
        _redis: &Pool,
        person_ids: &[i64],
    ) -> Result<HashMap<i64, String>, AppError> {
        let persons = face_person::Entity::find()
            .filter(face_person::Column::Id.is_in(person_ids.to_vec()))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(persons.into_iter().map(|p| (p.id, p.name)).collect())
    }

    pub async fn get_person_info(
        db: &DatabaseConnection,
        person_id: i64,
        img_url_generator: &ImageUrlProvider,
    ) -> Result<FacePersonVO, AppError> {
        let person = face_person::Entity::find_by_id(person_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("人物不存在"))?;

        let feature = face_feature::Entity::find_by_id(person.max_score_feature_id)
            .one(db)
            .await
            .map_internal_err("查询特征失败")?;

        let cover_url = if let Some(f) = feature {
            let photo = photo::Entity::find_by_id(f.photo_id)
                .one(db)
                .await
                .map_internal_err("查询照片失败")?;
            if let Some(p) = photo {
                let bbox: face_feature::FaceBBox =
                    serde_json::from_value(f.bbox.clone()).unwrap_or_else(|_| {
                        face_feature::FaceBBox {
                            x: 0.0,
                            y: 0.0,
                            w: 0.1,
                            h: 0.1,
                        }
                    });
                let cw = (bbox.w * p.width as f32) as i64;
                let ch = (bbox.h * p.height as f32) as i64;
                let cx = (bbox.x * p.width as f32) as i64;
                let cy = (bbox.y * p.height as f32) as i64;
                Some(img_url_generator.crop(p.file_id.clone(), cx as i32, cy as i32, cw as i32, ch as i32, 200).await)
            } else {
                None
            }
        } else {
            None
        };

        Ok(FacePersonVO {
            id: person.id.to_string(),
            name: person.name,
            total_photo_count: person.total_photo_count,
            cover_url,
        })
    }

    pub async fn get_person_photo(
        db: &DatabaseConnection,
        person_id: i64,
        cursor: Option<i64>,
        size: u32,
        img_url_generator: &ImageUrlProvider,
    ) -> Result<CursorPageVO<crate::models::photo::PhotoVO, i64>, AppError> {
        let limit = size as u64 + 1;
        let mut query = face_feature::Entity::find()
            .filter(face_feature::Column::PersonId.eq(person_id))
            .order_by_desc(face_feature::Column::Id)
            .limit(limit);

        if let Some(c) = cursor {
            query = query.filter(face_feature::Column::Id.lt(c));
        }

        let features = query.all(db).await.map_internal_err("查询失败")?;

        let has_more = features.len() > size as usize;
        let features: Vec<_> = features.into_iter().take(size as usize).collect();

        let photo_ids: Vec<i64> = features.iter().map(|f| f.photo_id).collect();
        let photos = photo::Entity::find()
            .filter(photo::Column::Id.is_in(photo_ids))
            .all(db)
            .await
            .map_internal_err("查询照片失败")?;

        let photo_map: HashMap<i64, _> = photos.into_iter().map(|p| (p.id, p)).collect();

        let next_cursor = features.last().map(|f| f.id);

        let futures = features.into_iter().map(|f| {
            let photo_opt = photo_map.get(&f.photo_id).cloned();
            async move {
                if let Some(p) = photo_opt {
                    let file_id = p.file_id.clone();
                    let extension = file_id.rsplit('.').next().unwrap_or("jpg").to_string();
                    let thumbnail_url = img_url_generator.thumbnail(file_id.clone()).await;
                    let preview_url = img_url_generator.preview(file_id.clone()).await;
                    let original_url = img_url_generator.original(file_id, extension).await;
                    Some(crate::models::photo::PhotoVO {
                        id: p.id.to_string(),
                        name: p.name,
                        thumbnail_url,
                        preview_url,
                        original_url,
                        width: p.width,
                        height: p.height,
                        size: p.size,
                        created_at: p.created_at.with_timezone(&Utc),
                        is_favorited: None,
                        is_collected: None,
                    })
                } else {
                    None
                }
            }
        });
        let records: Vec<crate::models::photo::PhotoVO> = join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    pub async fn delete_person(db: &DatabaseConnection, person_id: i64) -> Result<bool, AppError> {
        let _person = face_person::Entity::find_by_id(person_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("人物不存在"))?;

        let features = face_feature::Entity::find()
            .filter(face_feature::Column::PersonId.eq(person_id))
            .all(db)
            .await
            .map_internal_err("查询特征失败")?;

        for feature in features {
            let mut active: face_feature::ActiveModel = feature.into();
            active.person_id = Set(None);
            active.updated_at = Set(Utc::now().into());
            let _ = active.update(db).await;
        }

        face_person::Entity::delete_by_id(person_id)
            .exec(db)
            .await
            .map_internal_err("删除人物失败")?;

        Ok(true)
    }

    pub async fn change_face_belonging(
        db: &DatabaseConnection,
        feature_id: i64,
        person_id: i64,
    ) -> Result<(), AppError> {
        let feature = face_feature::Entity::find_by_id(feature_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("特征不存在"))?;

        let _person = face_person::Entity::find_by_id(person_id)
            .one(db)
            .await
            .map_internal_err("查询失败")?
            .ok_or_else(|| AppError::bad_request("目标人物不存在"))?;

        let mut feature_active: face_feature::ActiveModel = feature.into();
        feature_active.person_id = Set(Some(person_id));
        feature_active.updated_at = Set(Utc::now().into());
        feature_active.update(db).await.map_internal_err("更新失败")?;

        Ok(())
    }

    pub async fn process_face_tasks(
        db: &DatabaseConnection,
        mut rx: mpsc::Receiver<FaceTask>,
    ) {
        let face_engine = match FaceEngine::new(
            "models/det_10g.onnx",
            "models/w600k_r50.onnx",
        ) {
            Ok(engine) => engine,
            Err(e) => {
                tracing::error!("Failed to initialize face engine: {}", e);
                return;
            }
        };

        while let Some(task) = rx.recv().await {
            info!("Processing face task for photo {}", task.photo_id);
            
            if let Err(e) = Self::detect_and_recognize(
                db,
                &face_engine,
                task.photo_id,
                task.image_bytes,
            ).await {
                tracing::error!("Face processing failed for photo {}: {}", task.photo_id, e);
            }

            if let Err(e) = Self::perform_clustering(&db, 0.6, 3).await {
                tracing::error!("Face clustering failed: {}", e);
            }
        }
    }
}
