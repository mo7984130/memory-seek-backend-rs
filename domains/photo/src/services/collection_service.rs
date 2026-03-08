use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use deadpool_redis::Pool;
use entities::{collection, collection_photo, photo};
use imgproxy::ImgProxyService;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::collections::HashMap;

use crate::models::collection::{CollectionPhotoVO, CollectionVO};
use crate::models::photo::CursorPageVO;

pub struct CollectionService;

impl CollectionService {
    pub async fn get_collection_list(
        db: &DatabaseConnection,
        _redis: &Pool,
        user_id: u32,
        imgproxy: &ImgProxyService,
    ) -> Result<Vec<CollectionVO>, AppError> {
        let collections = collection::Entity::find()
            .filter(collection::Column::UserId.eq(user_id as i32))
            .order_by_asc(collection::Column::IsFavorite)
            .order_by_desc(collection::Column::CreatedAt)
            .all(db)
            .await
            .map_internal_err("查询收藏夹失败")?;

        let collections = if collections.is_empty() {
            Self::create_favorite_collection(db, user_id).await?;
            collection::Entity::find()
                .filter(collection::Column::UserId.eq(user_id as i32))
                .order_by_asc(collection::Column::IsFavorite)
                .order_by_desc(collection::Column::CreatedAt)
                .all(db)
                .await
                .map_internal_err("查询收藏夹失败")?
        } else {
            collections
        };

        let cover_ids: Vec<Option<i32>> = collections.iter().map(|c| c.cover_image_id).collect();

        let photos_with_covers = if cover_ids.iter().any(|id| id.is_some()) {
            let cover_ids: Vec<i32> = cover_ids.into_iter().flatten().collect();
            photo::Entity::find()
                .filter(photo::Column::Id.is_in(cover_ids.iter().map(|&id| id as i64)))
                .all(db)
                .await
                .map_internal_err("查询封面失败")?
        } else {
            vec![]
        };
        let _photo_map: HashMap<i64, _> = photos_with_covers
            .into_iter()
            .map(|p| (p.id, p))
            .collect();

        let no_cover_ids: Vec<i32> = collections
            .iter()
            .filter(|c| c.cover_image_id.is_none())
            .map(|c| c.id)
            .collect();

        let latest_photos = if !no_cover_ids.is_empty() {
            collection_photo::Entity::find()
                .filter(collection_photo::Column::CollectionId.is_in(no_cover_ids))
                .order_by_desc(collection_photo::Column::CreatedAt)
                .all(db)
                .await
                .map_internal_err("查询最新照片失败")?
        } else {
            vec![]
        };

        let mut latest_photo_map: HashMap<i32, i64> = HashMap::new();
        for cp in latest_photos {
            if !latest_photo_map.contains_key(&cp.collection_id) {
                latest_photo_map.insert(cp.collection_id, cp.photo_id);
            }
        }

        let all_photo_ids: Vec<i64> = collections
            .iter()
            .filter_map(|c| c.cover_image_id.map(|id| id as i64))
            .chain(latest_photo_map.values().cloned())
            .collect();

        let all_photos = if !all_photo_ids.is_empty() {
            photo::Entity::find()
                .filter(photo::Column::Id.is_in(all_photo_ids))
                .all(db)
                .await
                .map_internal_err("查询照片失败")?
        } else {
            vec![]
        };
        let all_photo_map: HashMap<i64, _> =
            all_photos.into_iter().map(|p| (p.id, p)).collect();

        let result: Vec<CollectionVO> = collections
            .iter()
            .map(|c| {
                let cover_photo = c
                    .cover_image_id
                    .and_then(|id| all_photo_map.get(&(id as i64)))
                    .or_else(|| {
                        latest_photo_map
                            .get(&c.id)
                            .and_then(|pid| all_photo_map.get(pid))
                    });

                CollectionVO {
                    id: c.id.to_string(),
                    name: c.name.clone(),
                    description: c.description.clone(),
                    photo_count: c.photo_count,
                    cover_image_url: cover_photo.map(|p| imgproxy.generate_thumbnail_url(&p.file_id)),
                    is_favorite: c.is_favorite,
                    created_at: c.created_at.with_timezone(&Utc),
                }
            })
            .collect();

        Ok(result)
    }

    pub async fn create_collection(
        db: &DatabaseConnection,
        user_id: u32,
        name: String,
        description: Option<String>,
    ) -> Result<CollectionVO, AppError> {
        let now = Utc::now();
        let collection = collection::ActiveModel {
            user_id: Set(user_id as i32),
            name: Set(name),
            description: Set(description),
            photo_count: Set(0),
            cover_image_id: Set(None),
            is_favorite: Set(false),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let collection = collection
            .insert(db)
            .await
            .map_internal_err("创建收藏夹失败")?;

        Ok(CollectionVO {
            id: collection.id.to_string(),
            name: collection.name,
            description: collection.description,
            photo_count: 0,
            cover_image_url: None,
            is_favorite: false,
            created_at: collection.created_at.with_timezone(&Utc),
        })
    }

    pub async fn edit_collection(
        db: &DatabaseConnection,
        user_id: u32,
        collection_id: u32,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<CollectionVO, AppError> {
        let collection = collection::Entity::find_by_id(collection_id as i32)
            .one(db)
            .await
            .map_internal_err("查询收藏夹失败")?
            .ok_or_else(|| AppError::bad_request("收藏夹不存在"))?;

        if collection.user_id != user_id as i32 {
            return Err(AppError::bad_request("无权限"));
        }

        let mut active: collection::ActiveModel = collection.into();
        if let Some(n) = name {
            active.name = Set(n);
        }
        if let Some(d) = description {
            active.description = Set(Some(d));
        }
        active.updated_at = Set(Utc::now().into());

        let collection = active
            .update(db)
            .await
            .map_internal_err("更新收藏夹失败")?;

        Ok(CollectionVO {
            id: collection.id.to_string(),
            name: collection.name,
            description: collection.description,
            photo_count: collection.photo_count,
            cover_image_url: None,
            is_favorite: collection.is_favorite,
            created_at: collection.created_at.with_timezone(&Utc),
        })
    }

    pub async fn delete_collection(
        db: &DatabaseConnection,
        user_id: u32,
        collection_id: u32,
    ) -> Result<(), AppError> {
        let collection = collection::Entity::find_by_id(collection_id as i32)
            .one(db)
            .await
            .map_internal_err("查询收藏夹失败")?
            .ok_or_else(|| AppError::bad_request("收藏夹不存在"))?;

        if collection.user_id != user_id as i32 {
            return Err(AppError::bad_request("无权限"));
        }

        if collection.is_favorite {
            return Err(AppError::bad_request("我喜欢不可删除"));
        }

        collection_photo::Entity::delete_many()
            .filter(collection_photo::Column::CollectionId.eq(collection_id as i32))
            .exec(db)
            .await
            .map_internal_err("删除收藏夹照片失败")?;

        collection::Entity::delete_by_id(collection_id as i32)
            .exec(db)
            .await
            .map_internal_err("删除收藏夹失败")?;

        Ok(())
    }

    pub async fn add_photo_to_collection(
        db: &DatabaseConnection,
        user_id: u32,
        collection_id: u32,
        photo_id: u64,
    ) -> Result<(), AppError> {
        let collection = collection::Entity::find_by_id(collection_id as i32)
            .one(db)
            .await
            .map_internal_err("查询收藏夹失败")?
            .ok_or_else(|| AppError::bad_request("收藏夹不存在"))?;

        if collection.user_id != user_id as i32 {
            return Err(AppError::bad_request("无权限"));
        }

        let now = Utc::now();
        let relation = collection_photo::ActiveModel {
            collection_id: Set(collection_id as i32),
            photo_id: Set(photo_id as i64),
            user_id: Set(user_id as i32),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        match relation.insert(db).await {
            Ok(_) => {
                let updated = collection::ActiveModel {
                    id: Set(collection_id as i32),
                    photo_count: Set(collection.photo_count + 1),
                    ..Default::default()
                };
                let _ = updated.update(db).await;
                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("duplicate") {
                    Err(AppError::bad_request("照片已在收藏夹中"))
                } else {
                    tracing::error!(target:"logs", "添加到收藏夹失败: {:?}", e);
                    Err(AppError::InternalServerError)
                }
            }
        }
    }

    pub async fn remove_photo_from_collection(
        db: &DatabaseConnection,
        user_id: u32,
        collection_id: u32,
        photo_id: u64,
    ) -> Result<(), AppError> {
        let result = collection_photo::Entity::delete_many()
            .filter(collection_photo::Column::CollectionId.eq(collection_id as i32))
            .filter(collection_photo::Column::PhotoId.eq(photo_id as i64))
            .filter(collection_photo::Column::UserId.eq(user_id as i32))
            .exec(db)
            .await
            .map_internal_err("移除失败")?;

        if result.rows_affected > 0 {
            let collection = collection::Entity::find_by_id(collection_id as i32)
                .one(db)
                .await
                .map_internal_err("查询失败")?;
            if let Some(c) = collection {
                let updated = collection::ActiveModel {
                    id: Set(collection_id as i32),
                    photo_count: Set((c.photo_count - 1).max(0)),
                    ..Default::default()
                };
                let _ = updated.update(db).await;
            }
            Ok(())
        } else {
            Err(AppError::bad_request("未找到该收藏关系"))
        }
    }

    pub async fn get_collection_photos(
        db: &DatabaseConnection,
        user_id: u32,
        collection_id: u32,
        cursor: Option<DateTime<Utc>>,
        size: u32,
        imgproxy: &ImgProxyService,
    ) -> Result<CursorPageVO<CollectionPhotoVO, DateTime<Utc>>, AppError> {
        let collection = collection::Entity::find_by_id(collection_id as i32)
            .one(db)
            .await
            .map_internal_err("查询收藏夹失败")?
            .ok_or_else(|| AppError::bad_request("收藏夹不存在"))?;

        if collection.user_id != user_id as i32 {
            return Err(AppError::bad_request("无权限"));
        }

        let limit = size as u64 + 1;
        let mut query = collection_photo::Entity::find()
            .filter(collection_photo::Column::CollectionId.eq(collection_id as i32))
            .order_by_desc(collection_photo::Column::CreatedAt)
            .limit(limit);

        if let Some(c) = cursor {
            query = query.filter(collection_photo::Column::CreatedAt.lt(c));
        }

        let relations = query.all(db).await.map_internal_err("查询失败")?;

        let has_more = relations.len() > size as usize;
        let relations: Vec<_> = relations.into_iter().take(size as usize).collect();

        let photo_ids: Vec<i64> = relations.iter().map(|r| r.photo_id).collect();

        let photos = if !photo_ids.is_empty() {
            photo::Entity::find()
                .filter(photo::Column::Id.is_in(photo_ids))
                .all(db)
                .await
                .map_internal_err("查询照片失败")?
        } else {
            vec![]
        };
        let photo_map: HashMap<i64, _> = photos.into_iter().map(|p| (p.id, p)).collect();

        let records: Vec<CollectionPhotoVO> = relations
            .iter()
            .filter_map(|r| {
                photo_map.get(&r.photo_id).map(|p| CollectionPhotoVO {
                    photo: crate::models::photo::PhotoVO {
                        id: p.id.to_string(),
                        name: p.name.clone(),
                        thumbnail_url: imgproxy.generate_thumbnail_url(&p.file_id),
                        preview_url: imgproxy.generate_preview_url(&p.file_id),
                        original_url: imgproxy.generate_original_url(
                            &p.file_id,
                            p.file_id.rsplit('.').next().unwrap_or("jpg"),
                        ),
                        width: p.width,
                        height: p.height,
                        size: p.size,
                        created_at: p.created_at.with_timezone(&Utc),
                        is_favorited: None,
                        is_collected: None,
                    },
                    collected_at: r.created_at.with_timezone(&Utc),
                })
            })
            .collect();

        let next_cursor = relations.last().map(|r| r.created_at.with_timezone(&Utc));

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    pub async fn find_collection_ids_by_photo(
        db: &DatabaseConnection,
        user_id: u32,
        photo_id: u64,
    ) -> Result<Vec<String>, AppError> {
        let relations = collection_photo::Entity::find()
            .filter(collection_photo::Column::UserId.eq(user_id as i32))
            .filter(collection_photo::Column::PhotoId.eq(photo_id as i64))
            .all(db)
            .await
            .map_internal_err("查询失败")?;

        Ok(relations.iter().map(|r| r.collection_id.to_string()).collect())
    }

    pub async fn create_favorite_collection(
        db: &DatabaseConnection,
        user_id: u32,
    ) -> Result<CollectionVO, AppError> {
        let existing = collection::Entity::find()
            .filter(collection::Column::UserId.eq(user_id as i32))
            .filter(collection::Column::IsFavorite.eq(true))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(c) = existing {
            return Ok(CollectionVO {
                id: c.id.to_string(),
                name: c.name,
                description: c.description,
                photo_count: c.photo_count,
                cover_image_url: None,
                is_favorite: true,
                created_at: c.created_at.with_timezone(&Utc),
            });
        }

        let now = Utc::now();
        let collection = collection::ActiveModel {
            user_id: Set(user_id as i32),
            name: Set("我喜欢".to_string()),
            description: Set(Some("喜欢收藏夹".to_string())),
            photo_count: Set(0),
            cover_image_id: Set(None),
            is_favorite: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let collection = collection
            .insert(db)
            .await
            .map_internal_err("创建我喜欢失败")?;

        Ok(CollectionVO {
            id: collection.id.to_string(),
            name: collection.name,
            description: collection.description,
            photo_count: 0,
            cover_image_url: None,
            is_favorite: true,
            created_at: collection.created_at.with_timezone(&Utc),
        })
    }

    pub async fn get_favorite_collection_id(
        db: &DatabaseConnection,
        _redis: &Pool,
        user_id: u32,
    ) -> Result<i32, AppError> {
        let collection = collection::Entity::find()
            .filter(collection::Column::UserId.eq(user_id as i32))
            .filter(collection::Column::IsFavorite.eq(true))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(c) = collection {
            Ok(c.id)
        } else {
            let created = Self::create_favorite_collection(db, user_id).await?;
            Ok(created.id.parse().unwrap_or(0))
        }
    }
}
