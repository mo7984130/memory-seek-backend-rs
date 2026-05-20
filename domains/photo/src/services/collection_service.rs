use crate::mappers::{CollectionMapper, CollectionPhotoMapper, PhotoMapper};
use crate::models::collection::{
    BatchOperationResultVO, CollectionPhotoCursor, CollectionPhotoVO, CollectionVO,
};
use crate::models::photo::CursorPageVO;
use crate::photo::PhotoVO;
use chrono::Utc;
use common::constants::RedisKeys;
use common::error::AppError;
use common::utils::{CacheExtension, ResultExt};
use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::TransactionTrait;
use std::collections::HashMap;
use std::os::linux::raw::stat;

use crate::state::PhotoState;

pub struct CollectionService;

// 定义全局缓存：Key 为 user_id (i64), Value 为 collection_id (i64)
// 设置最大容量 10000 条，过期时间 24 小时
static LOCAL_FAVORITE_ID_CACHE: Lazy<Cache<i64, i64>> = Lazy::new(|| {
    Cache::builder()
        .max_capacity(10000)
        .time_to_live(std::time::Duration::from_secs(24 * 60 * 60))
        .build()
});

impl CollectionService {
    /// 获取用户的收藏夹列表
    ///
    /// 如果用户没有收藏夹，会自动创建"我喜欢"收藏夹，
    /// 并为每个收藏夹生成封面图token。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    ///
    /// # 返回
    /// 返回收藏夹VO列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询或创建收藏夹失败
    pub async fn get_collection_list(
        state: &PhotoState,
        user_id: i64,
    ) -> Result<Vec<CollectionVO>, AppError> {
        // 获取用户收藏夹
        let collections = CollectionMapper::query_by_user_id(&state.db, user_id).await?;

        // 如果收藏夹为空, 创建默认的我喜欢收藏夹
        let collections = if collections.is_empty() {
            Self::create_favorite_collection(state, user_id).await?;
            CollectionMapper::query_by_user_id(&state.db, user_id).await?
        } else {
            collections
        };

        // 组装结果
        let result: Vec<CollectionVO> = collections
            .into_iter()
            .map(|c| CollectionVO::from_collection(c, &state.token_cipher))
            .collect();

        Ok(result)
    }

    /// 创建新收藏夹
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    /// - `name`: 收藏夹名称
    /// - `description`: 收藏夹描述
    ///
    /// # 返回
    /// 返回创建的收藏夹VO
    ///
    /// # 错误
    /// - `AppError`: 数据库插入失败
    pub async fn create_collection(
        state: &PhotoState,
        user_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<CollectionVO, AppError> {
        let c = CollectionMapper::insert(&state.db, user_id, name, description, false).await?;
        Ok(CollectionVO::from_collection(c, &state.token_cipher))
    }

    /// 编辑收藏夹信息
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// - `name`: 新名称（可选）
    /// - `description`: 新描述（可选）
    ///
    /// # 返回
    /// 返回更新后的收藏夹VO
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 无权限编辑该收藏夹
    pub async fn edit_collection(
        state: &PhotoState,
        user_id: i64,
        collection_id: i64,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<CollectionVO, AppError> {
        // 效验权限
        let collection = CollectionMapper::query_by_id(&state.db, collection_id).await?;
        if collection.user_id != user_id {
            return Err(AppError::forbidden("无权限"));
        }

        let collection =
            CollectionMapper::update(&state.db, collection_id, name, description, None, None)
                .await?;

        Ok(CollectionVO::from_collection(
            collection,
            &state.token_cipher,
        ))
    }

    /// 删除收藏夹
    ///
    /// 使用事务保证原子性，先删除收藏夹内所有照片关联，再删除收藏夹本身。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 无权限或尝试删除"我喜欢"收藏夹
    /// - `AppError::InternalServerError`: 数据库事务失败
    pub async fn delete_collection(
        state: &PhotoState,
        user_id: i64,
        collection_id: i64,
    ) -> Result<(), AppError> {
        // 校验
        let collection = CollectionMapper::query_by_id(&state.db, collection_id).await?;
        if collection.user_id != user_id {
            return Err(AppError::forbidden("无权限"));
        }
        if collection.is_favorite {
            return Err(AppError::bad_request("我喜欢不可删除"));
        }

        state
            .db
            .transaction::<_, (), AppError>(|txn| {
                Box::pin(async move {
                    // 删除收藏夹里面的照片
                    CollectionPhotoMapper::delete_by_collection_id(txn, collection_id).await?;
                    // 删除收藏夹本身
                    CollectionMapper::delete_by_id(txn, collection_id).await?;
                    Ok(())
                })
            })
            .await
            .trace_to_internal_err("db_err", "删除收藏夹")?;

        Ok(())
    }

    /// 添加照片到收藏夹
    ///
    /// 使用事务保证原子性，插入关联记录并递增收藏夹照片计数。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 无权限或照片已在收藏夹中
    /// - `AppError::InternalServerError`: 数据库事务失败
    pub async fn add_photo_to_collection(
        state: &PhotoState,
        user_id: i64,
        collection_id: i64,
        photo_id: i64,
    ) -> Result<(), AppError> {
        // 校验权限
        let collection = CollectionMapper::query_by_id(&state.db, collection_id).await?;
        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        if !CollectionPhotoMapper::exists_in_collection(&state.db, collection_id, &[photo_id])
            .await?
            .is_empty()
        {
            return Err(AppError::bad_request("照片已在收藏夹中"));
        }

        state
            .db
            .transaction::<_, (), AppError>(|txn| {
                Box::pin(async move {
                    CollectionPhotoMapper::insert(txn, collection_id, photo_id, user_id).await?;
                    CollectionMapper::increment_photo_count(txn, collection_id, 1).await?;
                    Ok(())
                })
            })
            .await?;

        Ok(())
    }

    /// 从收藏夹移除照片
    ///
    /// 使用事务保证原子性，删除关联记录并递减收藏夹照片计数。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    /// - `collection_id`: 收藏夹ID
    /// - `photo_id`: 照片ID
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 未找到该收藏关系
    /// - `AppError::InternalServerError`: 数据库事务失败
    pub async fn remove_photo_from_collection(
        state: &PhotoState,
        user_id: i64,
        collection_id: i64,
        photo_id: i64,
    ) -> Result<(), AppError> {
        state
            .db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    let removed =
                        CollectionPhotoMapper::delete(txn, collection_id, photo_id, user_id)
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
            })
            .await
            .map_err(|e| {
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
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID（用于权限验证和查询收藏状态）
    /// - `collection_id`: 收藏夹ID
    /// - `cursor`: 复合游标（base64编码的字符串）
    /// - `size`: 每页数量
    ///
    /// # 返回
    /// 返回分页的收藏照片列表
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 无权限访问该收藏夹
    pub async fn get_collection_photos(
        state: &PhotoState,
        user_id: i64,
        collection_id: i64,
        cursor: Option<String>,
        size: u32,
    ) -> Result<CursorPageVO<CollectionPhotoVO, String>, AppError> {
        let collection = CollectionMapper::query_by_id(&state.db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        let decoded_cursor = cursor
            .as_ref()
            .and_then(|s| CollectionPhotoCursor::decode(s));
        let relations = CollectionPhotoMapper::query_by_collection_id(
            &state.db,
            collection_id,
            decoded_cursor.as_ref(),
            (size + 1) as u64,
        )
        .await?;

        let has_more = relations.len() > size as usize;
        let relations: Vec<_> = relations.into_iter().take(size as usize).collect();

        let photo_ids: Vec<i64> = relations.iter().map(|r| r.photo_id).collect();
        let photo_map = PhotoMapper::query_by_ids(&state.db, &photo_ids)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect::<HashMap<_, _>>();

        let favorite_collection_id = Self::get_favorite_collection_id(state, user_id).await?;
        let favorited_photo_ids = CollectionPhotoMapper::exists_in_collection(
            &state.db,
            favorite_collection_id,
            &photo_ids,
        )
        .await?
        .into_iter()
        .collect::<std::collections::HashSet<i64>>();

        let next_cursor = relations.last().map(|r| {
            CollectionPhotoCursor {
                created_at: r.created_at.with_timezone(&Utc),
                id: r.id,
            }
            .encode()
        });

        let records: Vec<CollectionPhotoVO> = relations
            .into_iter()
            .filter_map(|r| {
                let p = photo_map.get(&r.photo_id)?;
                let (thumbnail_token, preview_token, original_token) =
                    crate::models::photo::PhotoVO::generate_tokens(&p.file_id, &state.token_cipher);

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
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回收藏夹ID字符串列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败
    pub async fn find_collection_ids_by_photo(
        state: &PhotoState,
        user_id: i64,
        photo_id: i64,
    ) -> Result<Vec<String>, AppError> {
        let ids =
            CollectionPhotoMapper::query_collection_ids_by_photo(&state.db, user_id, photo_id)
                .await?;
        Ok(ids.iter().map(|id| id.to_string()).collect())
    }

    /// 创建"我喜欢"收藏夹
    ///
    /// 如果已存在则返回现有收藏夹。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    ///
    /// # 返回
    /// 返回"我喜欢"收藏夹VO
    ///
    /// # 错误
    /// - `AppError`: 数据库查询或创建失败
    pub async fn create_favorite_collection(
        state: &PhotoState,
        user_id: i64,
    ) -> Result<CollectionVO, AppError> {
        let existing = CollectionMapper::query_favorite_by_user_id(&state.db, user_id).await?;

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

        let collection = CollectionMapper::insert(
            &state.db,
            user_id,
            "我喜欢".to_string(),
            Some("喜欢收藏夹".to_string()),
            true,
        )
        .await?;

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
    /// 因为喜欢收藏夹的ID不会改变，所以加上本地缓存。
    /// 优先从本地缓存获取，其次从Redis缓存获取，最后查询数据库。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    ///
    /// # 返回
    /// 返回"我喜欢"收藏夹ID
    ///
    /// # 错误
    /// - `AppError::NotFound`: 未找到收藏夹
    pub async fn get_favorite_collection_id(
        state: &PhotoState,
        user_id: i64,
    ) -> Result<i64, AppError> {
        // 先从本地缓存获取
        if let Some(id) = LOCAL_FAVORITE_ID_CACHE.get(&user_id).await {
            return Ok(id);
        }

        // 从redis中获取
        let id = state
            .redis
            .get_or_load(
                RedisKeys::photo::favorite_collection_id(user_id),
                24 * 60 * 60,
                || async move {
                    // 从数据库中获取
                    CollectionMapper::query_favorite_collection_id(&state.db, user_id)
                        .await?
                        .ok_or_else(|| AppError::not_found("未找到收藏夹"))
                },
            )
            .await?;

        // 回填本地缓存
        LOCAL_FAVORITE_ID_CACHE.insert(user_id, id).await;

        Ok(id)
    }

    /// 批量添加照片到收藏夹
    ///
    /// 使用事务保证原子性，自动跳过已存在的照片和不存在的照片ID。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID（用于权限验证）
    /// - `collection_id`: 收藏夹ID
    /// - `photo_ids`: 照片ID列表
    ///
    /// # 返回
    /// 返回批量操作结果，包含成功数量、已存在数量和失败数量
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 无权限操作该收藏夹
    /// - `AppError::InternalServerError`: 数据库事务失败（会回滚）
    pub async fn batch_add_photos_to_collection(
        state: &PhotoState,
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

        let collection = CollectionMapper::query_by_id(&state.db, collection_id).await?;

        if collection.user_id != user_id {
            return Err(AppError::bad_request("无权限"));
        }

        let photo_ids_set: std::collections::HashSet<i64> = photo_ids.iter().cloned().collect();
        let unique_photo_ids: Vec<i64> = photo_ids_set.into_iter().collect();

        let already_exists_ids = CollectionPhotoMapper::exists_in_collection(
            &state.db,
            collection_id,
            &unique_photo_ids,
        )
        .await?;
        let already_exists_set: std::collections::HashSet<i64> =
            already_exists_ids.iter().cloned().collect();

        let not_exists_in_collection: Vec<i64> = unique_photo_ids
            .iter()
            .filter(|id| !already_exists_set.contains(id))
            .cloned()
            .collect();

        let existing_photos =
            PhotoMapper::query_by_ids(&state.db, &not_exists_in_collection).await?;
        let existing_photo_ids: std::collections::HashSet<i64> =
            existing_photos.iter().map(|p| p.id).collect();

        let valid_photo_ids: Vec<i64> = not_exists_in_collection
            .into_iter()
            .filter(|id| existing_photo_ids.contains(id))
            .collect();

        let failed_ids: Vec<i64> = unique_photo_ids
            .into_iter()
            .filter(|id| !already_exists_set.contains(&id) && !existing_photo_ids.contains(&id))
            .collect();

        let success_count = state
            .db
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
    /// 使用事务保证原子性，不存在的关联会被计入失败数量。
    ///
    /// # 参数
    /// - `state`: 照片模块状态
    /// - `user_id`: 用户ID
    /// - `collection_id`: 收藏夹ID
    /// - `photo_ids`: 照片ID列表
    ///
    /// # 返回
    /// 返回批量操作结果，包含成功数量和失败数量
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 数据库事务失败（会回滚）
    pub async fn batch_remove_photos_from_collection(
        state: &PhotoState,
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

        let success_count = state
            .db
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
                        CollectionMapper::increment_photo_count(
                            txn,
                            collection_id,
                            -(count as i32),
                        )
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
