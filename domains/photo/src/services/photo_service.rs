use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::{FileValidator, ResultExt};
use deadpool_redis::Pool;
use entities::photo;
use futures::future::join_all;
use img_url_generator::{ImageUrlGenerator, ImageUrlProvider};
use oss::S3Client;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::models::photo::{CursorPageVO, PhotoCursorQuery, PhotoVO};
use crate::services::timeline_stat_service::TimelineStatService;

pub struct FaceTask {
    pub photo_id: i64,
    pub image_bytes: Vec<u8>,
}

pub struct PhotoService;

impl PhotoService {
    pub async fn upload_photo(
        db: &DatabaseConnection,
        _redis: &Pool,
        s3: &S3Client,
        face_tx: &mpsc::Sender<FaceTask>,
        user_id: i64,
        file_data: Vec<u8>,
        file_name: String,
        content_type: String,
        created_at: Option<DateTime<Utc>>,
        img_url_generator: &ImageUrlProvider,
    ) -> Result<PhotoVO, AppError> {
        let md5_hash = format!("{:x}", md5::compute(&file_data));

        if Self::md5_exists(db, &md5_hash).await? {
            return Err(AppError::bad_request("图片已存在"));
        }

        let metadata = FileValidator::validate_image(
            &file_data, file_name, content_type
        ).to_bad_request_error()?;

        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);

        let file_data_len = file_data.len();
        s3.upload(&file_id, file_data.clone(), &metadata.mime_type)
            .await
            .map_internal_err("OSS上传失败")?;

        let now = Utc::now();
        let photo = photo::ActiveModel {
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

        let _ = face_tx
            .send(FaceTask {
                photo_id,
                image_bytes: file_data,
            })
            .await;

        Ok(PhotoVO {
            id: photo_id.to_string(),
            name: photo.name,
            thumbnail_url: img_url_generator.thumbnail(photo.file_id.clone()).await,
            preview_url: img_url_generator.preview(photo.file_id.clone()).await,
            original_url: img_url_generator.original(
                photo.file_id.clone(),
                Self::get_extension(&photo.file_id).to_string(),
            ).await,
            width: photo.width,
            height: photo.height,
            size: photo.size,
            created_at: photo_created_at,
            is_favorited: None,
            is_collected: None,
        })
    }

    pub async fn get_photo_cursor_page(
        db: &DatabaseConnection,
        _redis: &Pool,
        user_id: i64,
        query: PhotoCursorQuery,
        img_url_generator: &ImageUrlProvider,
    ) -> Result<CursorPageVO<PhotoVO, DateTime<Utc>>, AppError> {
        let size = query.size as u64;

        let mut photos_query = photo::Entity::find()
            .filter(photo::Column::UserId.eq(user_id))
            .order_by_desc(photo::Column::CreatedAt)
            .limit(size + 1);

        if let Some(cursor) = query.cursor {
            if query.direction == "next" {
                photos_query = photos_query.filter(photo::Column::CreatedAt.lt(cursor));
            } else {
                photos_query = photos_query.filter(photo::Column::CreatedAt.gt(cursor));
            }
        }

        let photos = photos_query.all(db).await.map_internal_err("查询失败")?;

        let has_more = photos.len() > size as usize;
        let photos: Vec<_> = photos.into_iter().take(size as usize).collect();

        let futures = photos.into_iter().map(|p| {
            let file_id = p.file_id.clone();
            let extension = Self::get_extension(&file_id).to_string();
            async move {
                let thumbnail_url = img_url_generator.thumbnail(file_id.clone()).await;
                let preview_url = img_url_generator.preview(file_id.clone()).await;
                let original_url = img_url_generator.original(file_id, extension).await;
                PhotoVO {
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
                }
            }
        });
        let records: Vec<PhotoVO> = join_all(futures).await;

        let next_cursor = records.last().map(|r| r.created_at);

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    pub async fn md5_exists(db: &DatabaseConnection, md5: &str) -> Result<bool, AppError> {
        let count = photo::Entity::find()
            .filter(photo::Column::Md5.eq(md5))
            .count(db)
            .await
            .map_internal_err("查询MD5失败")?;
        Ok(count > 0)
    }

    pub async fn get_time_range(
        db: &DatabaseConnection,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
        let min = photo::Entity::find()
            .order_by_asc(photo::Column::CreatedAt)
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        let max = photo::Entity::find()
            .order_by_desc(photo::Column::CreatedAt)
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        let min_time = min
            .map(|p| p.created_at.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let max_time = max
            .map(|p| p.created_at.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        Ok((min_time, max_time))
    }

    fn get_extension(file_id: &str) -> &str {
        file_id.rsplit('.').next().unwrap_or("jpg")
    }
}
