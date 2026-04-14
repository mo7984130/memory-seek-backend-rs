use crate::mappers::{CollectionMapper, CollectionPhotoMapper, PhotoMapper};
use crate::models::collection::{BatchOperationResultVO, CollectionPhotoCursor, CollectionPhotoVO, CollectionVO};
use crate::models::photo::CursorPageVO;
use chrono::Utc;
use common::constants::RedisKeys;
use common::error::AppError;
use common::utils::CacheExtension;
use deadpool_redis::Pool;
use sea_orm::{DatabaseConnection, TransactionTrait};
use std::collections::HashMap;

pub struct CollectionService;

impl CollectionService {
    /// 获取用户的收藏夹列表
    /// 
    /// 如果用户没有收藏夹，会自动创建"我喜欢"收藏夹
    /// 为每个收藏夹生成封面图token
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `_redis`: Redis连接池（暂未使用）
    /// - `user_id`: 用户ID
    /// - `encryption_key`: 加密密钥
    /// 
    /// # 返回
    /// 返回收藏夹VO列表
    pub async fn get_collection_list(
        db: &DatabaseConnection,
        _redis: &Pool,
        user_id: i64,
        encryption_key: &[u8; 32],
    ) -> Result<Vec<CollectionVO>, AppError> {
        let collections = CollectionMapper::find_by_user_id(db, user_id).await?;

        let collections = if collections.is_empty() {
            Self::create_favorite_collection(db, user_id).await?;
            CollectionMapper::find_by_user_id(db, user_id).await?
        } else {
            collections
        };

        let cover_ids: Vec<Option<i64>> = collections.iter().map(|c| c.cover_image_id).collect();

        let photos_with_covers = if cover_ids.iter().any(|id| id.is_some()) {
            let cover_ids: Vec<i64> = cover_ids.into_iter().flatten().collect();
            PhotoMapper::find_by_ids(db, cover_ids).await?
        } else {
            vec![]
        };
        let _photo_map: HashMap<i64, _> = photos_with_covers
            .into_iter()
            .map(|p| (p.id, p))
            .collect();

        let no_cover_ids: Vec<i64> = collections
            .iter()
            .filter(|c| c.cover_image_id.is_none())
            .map(|c| c.id)
            .collect();

        let latest_photo_map = CollectionPhotoMapper::find_latest_photo_ids_by_collections(db, no_cover_ids).await?;

        let all_photo_ids: Vec<i64> = collections
            .iter()
            .filter_map(|c| c.cover_image_id)
            .chain(latest_photo_map.values().cloned())
            .collect();

        let all_photo_map = PhotoMapper::find_by_ids_map(db, all_photo_ids).await?;

        let result: Vec<CollectionVO> = collections
            .into_iter()
            .map(|c| {
                let cover_file_id = c
                    .cover_image_id
                    .and_then(|id| all_photo_map.get(&id))
                    .or_else(|| {
                        latest_photo_map
                            .get(&c.id)
                            .and_then(|pid| all_photo_map.get(pid))
                    })
                    .map(|p| p.file_id.clone());

                let cover_token = cover_file_id.as_ref().and_then(|fid| {
                    let (thumbnail_token, _, _) = crate::models::photo::PhotoVO::generate_tokens(fid, encryption_key);
                    thumbnail_token
                });
                
                CollectionVO {
                    id: c.id.to_string(),
                    name: c.name,
                    description: c.description,
                    photo_count: c.photo_count,
                    cover_token,
                    is_favorite: c.is_favorite,
                    created_at: c.created_at.with_timezone(&Utc),
                }
            })
            .collect();

        Ok(result)
    }

    /// 创建新收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `name`: 收藏夹名称
    /// - `description`: 收藏夹描述
    /// 
    /// # 返回
    /// 返回创建的收藏夹VO
    pub async fn create_collection(
        db: &DatabaseConnection,
        user_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<CollectionVO, AppError> {
        let collection = CollectionMapper::insert(db, user_id, name, description, false).await?;

        Ok(CollectionVO {
            id: collection.id.to_string(),
            name: collection.name,
            description: collection.description,
            photo_count: 0,
            cover_token: None,
            is_favorite: false,
            created_at: collection.created_at.with_timezone(&Utc),
        })
    }

    /// 编辑收藏夹信息
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// - `name`: 新名称（可选）
    /// - `description`: 新描述（可选）
    /// 
    /// # 返回
    /// 返回更新后的收藏夹VO
    /// 
    /// # 错误
    /// - 无权限返回400错误
    pub async fn edit_collection(
        db: &DatabaseConnection,
        user_id: i64,
        collection_id: i64,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<CollectionVO, AppError> {
        let collection = CollectionMapper::find_by_id(db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        let collection = CollectionMapper::update(db, collection_id, name, description, None, None).await?;

        Ok(CollectionVO {
            id: collection.id.to_string(),
            name: collection.name,
            description: collection.description,
            photo_count: collection.photo_count,
            cover_token: None,
            is_favorite: collection.is_favorite,
            created_at: collection.created_at.with_timezone(&Utc),
        })
    }

    /// 删除收藏夹
    /// 
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// 
    /// # 错误
    /// - 无权限返回400错误
    /// - 尝试删除"我喜欢"返回400错误
    pub async fn delete_collection(
        db: &DatabaseConnection,
        user_id: i64,
        collection_id: i64,
    ) -> Result<(), AppError> {
        let collection = CollectionMapper::find_by_id(db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        if collection.is_favorite {
            return Err(AppError::bad_request("我喜欢不可删除"));
        }

        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                CollectionPhotoMapper::delete_by_collection_id(txn, collection_id)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                CollectionMapper::delete_by_id(txn, collection_id)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                Ok(())
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "删除收藏夹失败: {:?}", e);
            AppError::InternalServerError
        })
    }

    /// 添加照片到收藏夹
    /// 
    /// 使用事务保证原子性，使用 ON CONFLICT 检测重复
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    /// 
    /// # 错误
    /// - 无权限返回400错误
    /// - 照片已在收藏夹中返回400错误
    pub async fn add_photo_to_collection(
        db: &DatabaseConnection,
        user_id: i64,
        collection_id: i64,
        photo_id: i64,
    ) -> Result<(), AppError> {
        let collection = CollectionMapper::find_by_id(db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        if CollectionPhotoMapper::exists_photo_in_collection(db, collection_id, photo_id).await? {
            return Err(AppError::bad_request("照片已在收藏夹中"));
        }

        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                CollectionPhotoMapper::insert(txn, collection_id, photo_id, user_id)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                CollectionMapper::increment_photo_count(txn, collection_id, 1)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                Ok(())
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "添加到收藏夹失败: {:?}", e);
            AppError::InternalServerError
        })
    }

    /// 从收藏夹移除照片
    /// 
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    /// 
    /// # 错误
    /// - 未找到收藏关系返回400错误
    pub async fn remove_photo_from_collection(
        db: &DatabaseConnection,
        user_id: i64,
        collection_id: i64,
        photo_id: i64,
    ) -> Result<(), AppError> {
        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                let removed = CollectionPhotoMapper::delete(txn, collection_id, photo_id, user_id)
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                if removed {
                    CollectionMapper::increment_photo_count(txn, collection_id, -1)
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    Ok(())
                } else {
                    Err(sea_orm::DbErr::Custom("未找到该收藏关系".to_string()))
                }
            })
        }).await.map_err(|e| {
            tracing::error!(target:"logs", "从收藏夹移除失败: {:?}", e);
            if e.to_string().contains("未找到该收藏关系") {
                AppError::bad_request("未找到该收藏关系")
            } else {
                AppError::InternalServerError
            }
        })
    }

    /// 获取收藏夹中的照片列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `user_id`: 用户ID（用于权限验证和查询收藏状态）
    /// - `collection_id`: 收藏夹ID
    /// - `cursor`: 复合游标（base64编码的字符串）
    /// - `size`: 每页数量
    /// - `encryption_key`: 加密密钥
    /// 
    /// # 返回
    /// 返回分页的收藏照片列表
    /// 
    /// # 错误
    /// - 无权限返回400错误
    pub async fn get_collection_photos(
        db: &DatabaseConnection,
        redis: &Pool,
        user_id: i64,
        collection_id: i64,
        cursor: Option<String>,
        size: u32,
        encryption_key: &[u8; 32],
    ) -> Result<CursorPageVO<CollectionPhotoVO, String>, AppError> {
        let collection = CollectionMapper::find_by_id(db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        let decoded_cursor = cursor.as_ref().and_then(|s| CollectionPhotoCursor::decode(s));
        let relations = CollectionPhotoMapper::find_by_collection_id(db, collection_id, decoded_cursor.as_ref(), (size + 1) as u64).await?;

        let has_more = relations.len() > size as usize;
        let relations: Vec<_> = relations.into_iter().take(size as usize).collect();

        let photo_ids: Vec<i64> = relations.iter().map(|r| r.photo_id).collect();
        let photo_map = PhotoMapper::find_by_ids_map(db, photo_ids.clone()).await?;

        let favorite_collection_id = Self::get_favorite_collection_id(db, redis, user_id).await?;
        let favorited_photo_ids = CollectionPhotoMapper::exists_in_collection(db, favorite_collection_id, &photo_ids).await?.into_iter().collect::<std::collections::HashSet<i64>>();

        let next_cursor = relations.last().map(|r| {
            CollectionPhotoCursor {
                created_at: r.created_at.with_timezone(&Utc),
                id: r.id,
            }.encode()
        });

        let records: Vec<CollectionPhotoVO> = relations
            .into_iter()
            .filter_map(|r| {
                let p = photo_map.get(&r.photo_id)?;
                let (thumbnail_token, preview_token, original_token) = 
                    crate::models::photo::PhotoVO::generate_tokens(&p.file_id, encryption_key);
                
                Some(CollectionPhotoVO {
                    photo: crate::models::photo::PhotoVO {
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
                    },
                    collected_at: r.created_at.with_timezone(&Utc),
                })
            })
            .collect();

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    /// 查询照片所在的收藏夹ID列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `photo_id`: 照片ID
    /// 
    /// # 返回
    /// 返回收藏夹ID字符串列表
    pub async fn find_collection_ids_by_photo(
        db: &DatabaseConnection,
        user_id: i64,
        photo_id: i64,
    ) -> Result<Vec<String>, AppError> {
        let ids = CollectionPhotoMapper::find_collection_ids_by_photo(db, user_id, photo_id).await?;
        Ok(ids.iter().map(|id| id.to_string()).collect())
    }

    /// 创建"我喜欢"收藏夹
    /// 
    /// 如果已存在则返回现有收藏夹
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回"我喜欢"收藏夹VO
    pub async fn create_favorite_collection(
        db: &DatabaseConnection,
        user_id: i64,
    ) -> Result<CollectionVO, AppError> {
        let existing = CollectionMapper::find_favorite_by_user_id(db, user_id).await?;

        if let Some(c) = existing {
            return Ok(CollectionVO {
                id: c.id.to_string(),
                name: c.name,
                description: c.description,
                photo_count: c.photo_count,
                cover_token: None,
                is_favorite: true,
                created_at: c.created_at.with_timezone(&Utc),
            });
        }

        let collection = CollectionMapper::insert(db, user_id, "我喜欢".to_string(), Some("喜欢收藏夹".to_string()), true).await?;

        Ok(CollectionVO {
            id: collection.id.to_string(),
            name: collection.name,
            description: collection.description,
            photo_count: 0,
            cover_token: None,
            is_favorite: true,
            created_at: collection.created_at.with_timezone(&Utc),
        })
    }

    /// 获取用户"我喜欢"收藏夹ID
    /// 
    /// 使用Redis缓存，缓存时间24小时
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `user_id`: 用户ID
    /// 
    /// # 返回
    /// 返回"我喜欢"收藏夹ID
    /// 
    /// # 错误
    /// - 未找到收藏夹返回404错误
    pub async fn get_favorite_collection_id(
        db: &DatabaseConnection,
        redis: &Pool,
        user_id: i64,
    ) -> Result<i64, AppError> {
        redis.get_or_load(
            RedisKeys::photo::favorite_collection_id(user_id),
            24 * 60 * 60,
            || async move {
                CollectionMapper::find_favorite_collection_id(db, user_id)
                    .await?
                    .ok_or_else(|| AppError::not_found("未找到收藏夹"))
            }
        ).await
    }

    /// 批量添加照片到收藏夹
    /// 
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// - `photo_ids`: 照片ID列表
    /// 
    /// # 返回
    /// 返回批量操作结果
    /// - success_count: 成功添加数量
    /// - already_exists_count/count: 已存在于收藏夹的数量
    /// - failed_count: 失败数量（照片不存在等原因）
    /// 
    /// # 错误
    /// - 无权限返回400错误
    /// - 数据库错误返回500错误（事务会回滚）
    pub async fn batch_add_photos_to_collection(
        db: &DatabaseConnection,
        user_id: i64,
        collection_id: i64,
        photo_ids: Vec<i64>,
    ) -> Result<BatchOperationResultVO, AppError> {
        if photo_ids.is_empty() {
            return Ok(BatchOperationResultVO {
                success_count: 0,
                already_exists_count: 0,
                already_exists_photo_ids: vec![],
                failed_count: 0,
                failed_photo_ids: vec![],
            });
        }

        let collection = CollectionMapper::find_by_id(db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        let photo_ids_set: std::collections::HashSet<i64> = photo_ids.iter().cloned().collect();
        let unique_photo_ids: Vec<i64> = photo_ids_set.into_iter().collect();

        let already_exists_ids = CollectionPhotoMapper::exists_in_collection(
            db,
            collection_id,
            &unique_photo_ids.clone(),
        )
        .await?;
        let already_exists_set: std::collections::HashSet<i64> =
            already_exists_ids.iter().cloned().collect();

        let not_exists_in_collection: Vec<i64> = unique_photo_ids
            .iter()
            .filter(|id| !already_exists_set.contains(id))
            .cloned()
            .collect();

        let existing_photos = PhotoMapper::find_by_ids(db, not_exists_in_collection.clone()).await?;
        let existing_photo_ids: std::collections::HashSet<i64> =
            existing_photos.iter().map(|p| p.id).collect();

        let valid_photo_ids: Vec<i64> = not_exists_in_collection
            .into_iter()
            .filter(|id| existing_photo_ids.contains(id))
            .collect();

        let failed_ids: Vec<i64> = unique_photo_ids
            .into_iter()
            .filter(|id| {
                !already_exists_set.contains(&id) && !existing_photo_ids.contains(&id)
            })
            .collect();

        let success_count = db
            .transaction::<_, u32, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    let count = CollectionPhotoMapper::batch_insert(
                        txn,
                        collection_id,
                        valid_photo_ids,
                        user_id,
                    )
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    if count > 0 {
                        CollectionMapper::increment_photo_count(txn, collection_id, count as i32)
                            .await
                            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    }

                    Ok(count)
                })
            })
            .await
            .map_err(|e| {
                tracing::error!(target: "logs", "批量添加到收藏夹失败: {:?}", e);
                AppError::InternalServerError
            })?;

        Ok(BatchOperationResultVO {
            success_count,
            already_exists_count: already_exists_ids.len() as u32,
            already_exists_photo_ids: already_exists_ids.iter().map(|id| id.to_string()).collect(),
            failed_count: failed_ids.len() as u32,
            failed_photo_ids: failed_ids.iter().map(|id| id.to_string()).collect(),
        })
    }

    /// 批量从收藏夹移除照片
    /// 
    /// 使用事务保证原子性
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `user_id`: 用户ID
    /// - `collection_id`: 收藏夹ID
    /// - `photo_ids`: 照片ID列表
    /// 
    /// # 返回
    /// 返回批量操作结果
    /// - success_count: 成功移除数量
    /// - failed_count: 失败数量（不在收藏夹中）
    pub async fn batch_remove_photos_from_collection(
        db: &DatabaseConnection,
        user_id: i64,
        collection_id: i64,
        photo_ids: Vec<i64>,
    ) -> Result<BatchOperationResultVO, AppError> {
        if photo_ids.is_empty() {
            return Ok(BatchOperationResultVO {
                success_count: 0,
                already_exists_count: 0,
                already_exists_photo_ids: vec![],
                failed_count: 0,
                failed_photo_ids: vec![],
            });
        }

        let photo_ids_set: std::collections::HashSet<i64> = photo_ids.iter().cloned().collect();
        let unique_photo_ids: Vec<i64> = photo_ids_set.into_iter().collect();
        let total_count = unique_photo_ids.len() as u32;

        let success_count = db
            .transaction::<_, u32, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    let count = CollectionPhotoMapper::batch_delete(
                        txn,
                        collection_id,
                        unique_photo_ids,
                        user_id,
                    )
                    .await
                    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    if count > 0 {
                        CollectionMapper::increment_photo_count(txn, collection_id, -(count as i32))
                            .await
                            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    }

                    Ok(count)
                })
            })
            .await
            .map_err(|e| {
                tracing::error!(target: "logs", "批量从收藏夹移除失败: {:?}", e);
                AppError::InternalServerError
            })?;

        Ok(BatchOperationResultVO {
            success_count,
            already_exists_count: 0,
            already_exists_photo_ids: vec![],
            failed_count: total_count - success_count,
            failed_photo_ids: vec![],
        })
    }
}
