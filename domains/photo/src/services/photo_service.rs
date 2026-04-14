#[cfg(feature = "face_recognition")]
use crate::mappers::FaceFeatureMapper;
use crate::mappers::{
    CollectionMapper, CollectionPhotoMapper, CommentLikeMapper, CommentMapper, PhotoMapper,
};
use crate::models::photo::{CursorPageVO, PhotoCursor, PhotoCursorQuery, PhotoVO};
use crate::services::CollectionService;
#[cfg(feature = "face_recognition")]
use crate::services::feature_service::FeatureService;
use crate::services::timeline_stat_service::TimelineStatService;
use chrono::{DateTime, Utc};
use common::constants::RedisKeys;
use common::error::AppError;
use common::utils::{CacheExtension, FileValidator, ResultExt};
use deadpool_redis::Pool;
use entities::photo::{ActiveModel, Model};
use oss::S3Client;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use std::collections::HashSet;
#[cfg(feature = "face_recognition")]
use tokio::sync::mpsc;
use uuid::Uuid;

/// 人脸检测任务，包含照片ID、图片字节数据和元数据
#[cfg(feature = "face_recognition")]
pub struct FaceTask {
    pub photo_id: i64,
    pub image_bytes: Vec<u8>,
    /// 图片宽度（像素）
    pub img_width: u32,
    /// 图片高度（像素）
    pub img_height: u32,
}

pub struct PhotoService;

impl PhotoService {
    /// 上传照片
    ///
    /// 执行以下步骤：
    /// 1. 计算MD5检查是否重复
    /// 2. 验证图片格式和尺寸
    /// 3. 上传到OSS存储
    /// 4. 保存照片记录到数据库
    /// 5. 更新时间线统计
    /// 6. 发送人脸检测任务（如果启用了face_recognition feature）
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `_redis`: Redis连接池（暂未使用）
    /// - `s3`: OSS客户端
    /// - `face_tx`: 人脸检测任务发送通道（仅face_recognition feature）
    /// - `user_id`: 用户ID
    /// - `file_data`: 文件字节数据
    /// - `file_name`: 原始文件名
    /// - `content_type`: 文件MIME类型
    /// - `created_at`: 自定义创建时间（可选）
    /// - `encryption_key`: 加密密钥
    ///
    /// # 返回
    /// 返回上传成功的照片VO
    pub async fn upload_photo(
        db: &sea_orm::DatabaseConnection,
        _redis: &Pool,
        s3: &S3Client,
        #[cfg(feature = "face_recognition")] face_tx: &Option<mpsc::Sender<FaceTask>>,
        user_id: i64,
        file_data: Vec<u8>,
        file_name: String,
        content_type: String,
        created_at: Option<DateTime<Utc>>,
        encryption_key: &[u8; 32],
    ) -> Result<PhotoVO, AppError> {
        let md5_hash = format!("{:x}", md5::compute(&file_data));

        if PhotoMapper::exists_by_md5(db, &md5_hash).await? {
            return Err(AppError::bad_request("图片已存在"));
        }

        let metadata = FileValidator::validate_image(&file_data, file_name, content_type)
            .to_bad_request_error()?;

        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);

        let file_data_len = file_data.len();
        s3.upload(&file_id, file_data.clone(), &metadata.mime_type)
            .await
            .map_internal_err("OSS上传失败")?;

        let now = Utc::now();
        let photo = ActiveModel {
            user_id: Set(user_id),
            name: Set(metadata.name),
            size: Set(file_data_len as i64),
            width: Set(metadata.width as i32),
            height: Set(metadata.height as i32),
            mime_type: Set(metadata.mime_type),
            md5: Set(md5_hash),
            file_id: Set(file_id.clone()),
            created_at: Set(created_at.unwrap_or(now).into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_internal_err("保存照片失败")?;

        let photo_id = photo.id;
        let photo_created_at = photo.created_at.with_timezone(&Utc);

        let _ = TimelineStatService::incr_stat(db, photo_created_at).await;

        #[cfg(feature = "face_recognition")]
        if let Some(tx) = face_tx {
            let _ = tx
                .send(FaceTask {
                    photo_id,
                    image_bytes: file_data,
                    img_width: metadata.width,
                    img_height: metadata.height,
                })
                .await;
        }

        let (thumbnail_token, preview_token, original_token) =
            PhotoVO::generate_tokens(&file_id, encryption_key);

        Ok(PhotoVO {
            id: photo_id.to_string(),
            name: photo.name,
            width: photo.width,
            height: photo.height,
            size: photo.size,
            created_at: photo_created_at,
            is_favorited: None,
            is_collected: None,
            thumbnail_token,
            preview_token,
            original_token,
        })
    }

    /// 游标分页查询照片列表
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `user_id`: 用户ID
    /// - `query`: 分页查询参数
    /// - `encryption_key`: 加密密钥
    ///
    /// # 返回
    /// 返回分页照片列表，包含是否有更多数据的标识
    pub async fn get_photo_cursor_page(
        db: &sea_orm::DatabaseConnection,
        redis: &Pool,
        user_id: i64,
        query: PhotoCursorQuery,
        encryption_key: &[u8; 32],
    ) -> Result<CursorPageVO<PhotoVO, String>, AppError> {
        let size = query.size as u64;
        let decoded_cursor = query.cursor.and_then(|s| PhotoCursor::decode(s));

        let mut photo_ids = PhotoMapper::find_cursor_page_ids(db, decoded_cursor, size + 1, &query.direction).await?;
        if photo_ids.is_empty() {
            return Ok(CursorPageVO::empty());
        }

        let has_more = photo_ids.len() > size as usize;
        photo_ids.truncate(size as usize);

        let photos = redis
            .get_or_load_batch(
                &photo_ids,
                |id| RedisKeys::photo::photo_info(*id),
                24 * 60 * 60,
                |miss_ids| async move { Ok(PhotoMapper::find_by_ids(db, miss_ids).await?) },
                |photo| photo.id,
            )
            .await?;

        // TODO LOCAL CACHE
        let favorite_collection_id =
            CollectionService::get_favorite_collection_id(db, redis, user_id).await?;
        let favorited_photo_ids =
            CollectionPhotoMapper::exists_in_collection(db, favorite_collection_id, &photo_ids)
                .await?
                .into_iter()
                .collect::<HashSet<i64>>();

        let next_cursor = photos.iter().flatten().last().map(|p| {
            PhotoCursor {
                created_at: p.created_at,
                id: p.id
            }
            .encode()
        });
        let records: Vec<PhotoVO> = photos
            .into_iter()
            .flatten()
            .map(|p| {
                let (thumbnail_token, preview_token, original_token) =
                    PhotoVO::generate_tokens(&p.file_id, encryption_key);

                PhotoVO {
                    id: p.id.to_string(),
                    name: p.name,
                    width: p.width,
                    height: p.height,
                    size: p.size,
                    created_at: p.created_at.with_timezone(&Utc),
                    is_favorited: Some(favorited_photo_ids.contains(&p.id)),
                    is_collected: None,
                    thumbnail_token,
                    preview_token,
                    original_token,
                }
            })
            .collect();

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    /// 根据ID获取照片详情
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回照片模型，不存在返回404错误
    pub async fn get_photo_by_id(
        db: &sea_orm::DatabaseConnection,
        photo_id: i64,
    ) -> Result<Model, AppError> {
        PhotoMapper::find_by_id(db, photo_id).await
    }

    /// 检查MD5是否已存在
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `md5`: 文件MD5哈希值
    ///
    /// # 返回
    /// 存在返回true，否则返回false
    pub async fn md5_exists(db: &sea_orm::DatabaseConnection, md5: &str) -> Result<bool, AppError> {
        PhotoMapper::exists_by_md5(db, md5).await
    }

    /// 获取所有照片的时间范围
    ///
    /// # 参数
    /// - `db`: 数据库连接
    ///
    /// # 返回
    /// 返回最早和最晚照片的创建时间元组
    pub async fn get_time_range(
        db: &sea_orm::DatabaseConnection,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
        PhotoMapper::find_time_range(db).await
    }

    /// 删除照片
    ///
    /// 执行以下步骤：
    /// 1. 验证权限（仅管理员可删除）
    /// 2. 获取照片的所有人脸特征，逐个删除（减量计算更新人物统计）（仅face_recognition feature）
    /// 3. 删除照片的所有收藏关联，更新收藏夹照片数量
    /// 4. 删除照片的所有评论及其点赞记录
    /// 5. 删除照片记录
    /// 6. 删除 OSS 文件
    /// 7. 更新时间线统计
    ///
    /// 使用事务保证数据库操作的原子性
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `s3`: OSS客户端
    /// - `user_id`: 用户ID
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 成功返回空元组
    ///
    /// # 错误
    /// - 非管理员返回403错误
    /// - 照片不存在返回404错误
    pub async fn delete_photo(
        db: &sea_orm::DatabaseConnection,
        #[cfg(feature = "face_recognition")] redis: &Pool,
        #[cfg(not(feature = "face_recognition"))] _redis: &Pool,
        s3: &S3Client,
        user_id: i64,
        photo_id: i64,
    ) -> Result<(), AppError> {
        if user_id != 1 {
            return Err(AppError::forbidden("只有管理员可以删除照片"));
        }

        let photo = PhotoMapper::find_by_id(db, photo_id).await?;
        let file_id = photo.file_id.clone();
        let created_at = photo.created_at.with_timezone(&Utc);

        #[cfg(feature = "face_recognition")]
        {
            let features = FaceFeatureMapper::find_by_photo_id(db, photo_id).await?;
            let person_ids: std::collections::HashSet<i64> =
                features.iter().filter_map(|f| f.person_id).collect();

            let features = features;
            db.transaction::<_, (), sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    for feature in features {
                        FaceFeatureMapper::delete_by_id(txn, feature.id)
                            .await
                            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
                    }

                    Self::delete_photo_transaction_body(txn, photo_id).await
                })
            })
            .await
            .map_err(|e| {
                tracing::error!(target:"logs", "删除照片失败: {:?}", e);
                AppError::InternalServerError
            })?;

            for person_id in person_ids {
                let _ = FeatureService::recalculate_person_stats(db, redis, person_id).await;
            }
        }

        #[cfg(not(feature = "face_recognition"))]
        {
            db.transaction::<_, (), sea_orm::DbErr>(|txn| {
                Box::pin(async move { Self::delete_photo_transaction_body(txn, photo_id).await })
            })
            .await
            .map_err(|e| {
                tracing::error!(target:"logs", "删除照片失败: {:?}", e);
                AppError::InternalServerError
            })?;
        }

        let _ = s3.delete(&file_id).await;

        let _ = TimelineStatService::decr_stat(db, created_at).await;

        Ok(())
    }

    async fn delete_photo_transaction_body(
        txn: &sea_orm::DatabaseTransaction,
        photo_id: i64,
    ) -> Result<(), sea_orm::DbErr> {
        let collection_ids = CollectionPhotoMapper::delete_by_photo_id(txn, photo_id)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
        for collection_id in collection_ids {
            CollectionMapper::increment_photo_count(txn, collection_id, -1)
                .await
                .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
        }

        let comment_ids = CommentMapper::find_ids_by_photo_id(txn, photo_id)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
        if !comment_ids.is_empty() {
            CommentLikeMapper::delete_by_comment_ids(txn, comment_ids)
                .await
                .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
        }
        CommentMapper::delete_by_photo_id(txn, photo_id)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

        PhotoMapper::delete_by_id(txn, photo_id)
            .await
            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

        Ok(())
    }
}
