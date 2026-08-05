use std::pin::Pin;

use bytes::Bytes;
use chrono::Utc;
use common::{
    error::AppError,
    ext::{
        BoolExt, CacheExtension, OkExt, OptionExt, ResultErrExt, ResultInspectErrAsync, ToErr,
        log_warn,
    },
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    timed,
    utils::{DbUtils, FileValidator, MetricsTimerExt},
};
use constants::RedisKeys;
use futures::Stream;
use oss::OssError;
use sea_orm::entity::prelude::DateTimeUtc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
        comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
        photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper,
        timeline_stat_mapper::TimelineStatMapper,
    },
    state::PhotoState,
};
use common::Result;
use types::photo::{
    ImageToken, ImageTokenType,
    dto::photo::{PageDirection, PhotoView},
    models::{DeletePhotosParam, ExistsByMd5BatchParam, UploadPhotoParam},
};

use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::photo::{ActiveModel, PhotoId, PhotoRecord},
};

pub(crate) struct PhotoService;

// 查询
impl PhotoService {
    #[tracing::instrument(skip_all)]
    pub async fn load_photos_info(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<PhotoView>> {
        let (photos_result, liked_photo_ids_result) = tokio::join!(
            state.redis.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_info(*id),
                24 * 60 * 60,
                |miss_ids| async move { PhotoMapper::query_by_ids(&state.db, &miss_ids).await },
                |photo| photo.id,
            ),
            PhotoLikeMapper::query_is_like_by_photo_ids(&state.db, user_id, photo_ids)
        );
        let photos = photos_result?;
        let liked_photo_ids = liked_photo_ids_result?;
        photos
            .into_iter()
            .flatten()
            .map(|p| {
                let liked = liked_photo_ids.contains(&p.id);
                let file_id = p.file_id.clone();
                PhotoView::from(p)
                    .with_liked(liked)
                    .with_tokens(&file_id, &state.token_cipher)
            })
            .collect::<Vec<_>>()
            .to_ok()
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_photo_cursor_page(
        state: &PhotoState,
        user_id: UserId,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
        direction: PageDirection,
        anchor_time: Option<DateTimeUtc>,
    ) -> Result<CursorPage<PhotoView, String>> {
        metrics_group!();

        // 获取photo_ids
        let photo_ids =
            PhotoMapper::query_cursor_page_ids(&state.db, cursor, size + 1, direction, anchor_time)
                .timed(metrics_name!("find_cursor_page_ids"))
                .await?;
        if photo_ids.is_empty() {
            return Ok(CursorPage::empty());
        }

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = Self::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        // 获取next_cursor
        let next_cursor = if has_more {
            match photo_vos.last() {
                Some(last_vo) => {
                    let id = last_vo
                        .id
                        .parse::<i64>()
                        .trace_internal_err("parse_photo_vo_id_err", "解析照片VOid错误")?;
                    Some(
                        TimeIdCursor {
                            id: PhotoId(id),
                            created_at: last_vo.created_at,
                        }
                        .encode(),
                    )
                }
                None => None,
            }
        } else {
            None
        };

        metrics_success!();

        Ok(CursorPage {
            records: photo_vos,
            next_cursor,
            has_more,
        })
    }
}

impl PhotoService {
    #[instrument(skip_all, fields(user_id, file_name = %param.file_name))]
    pub async fn upload_photo(
        state: &PhotoState,
        user_id: UserId,
        file_data: Bytes,
        param: UploadPhotoParam,
    ) -> Result<PhotoView> {
        metrics_group!();

        // 效验文件
        let metadata = {
            timed!("validate_photo", {
                FileValidator::validate_image(&file_data, &param.file_name, &param.content_type)?
            })
        };

        // 计算md5
        let md5_hash = {
            let file_data_clone = file_data.clone();
            timed!(
                "md5_hash",
                tokio::task::spawn_blocking(move || format!(
                    "{:x}",
                    md5::compute(&file_data_clone)
                ))
                .await?
            )
        };
        PhotoMapper::exists_by_md5(&state.db, &md5_hash)
            .await?
            .false_or_warn(
                "upload_photo:img_exist",
                "图片已存在",
                AppError::bad_request("图片已存在"),
            )?;

        // 上传文件
        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);
        state
            .s3_client
            .upload(&file_id, &file_data, &metadata.mime_type)
            .timed(metrics_name!("s3_upload"))
            .await?;

        // 更新数据库
        let now = Utc::now();
        let photo = ActiveModel {
            user_id: Set(user_id.0),
            name: Set(metadata.name),
            size: Set(file_data.len() as i64),
            width: Set(metadata.width as i32),
            height: Set(metadata.height as i32),
            mime_type: Set(metadata.mime_type),
            md5: Set(md5_hash),
            file_id: Set(file_id.clone()),
            created_at: Set(param.created_at.unwrap_or(now)),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&state.db)
        .timed(metrics_name!("db_insert"))
        .await
        .inspect_err_async(|_| async {
            let _ = state
                .s3_client
                .delete(&file_id)
                .await
                .map_err(AppError::from);
        })
        .await?;

        // 增加时间线统计
        // 错误不返回
        let _ = TimelineStatMapper::incr_stat(&state.db, photo.created_at).await;

        metrics_success!();

        let file_id = photo.file_id.clone();
        PhotoView::from(PhotoRecord::from(photo))
            .with_tokens(&file_id, &state.token_cipher)
            .to_ok()
    }

    #[tracing::instrument(skip_all)]
    pub async fn exists_by_md5_batch(
        state: &PhotoState,
        param: ExistsByMd5BatchParam,
    ) -> Result<Vec<bool>> {
        metrics_group!();

        let existing = PhotoMapper::exists_by_md5_batch(&state.db, &param.md5s).await?;
        let res = param
            .md5s
            .iter()
            .map(|md5| existing.contains(md5))
            .collect::<Vec<bool>>();

        metrics_success!();
        Ok(res)
    }

    #[tracing::instrument(skip_all)]
    pub async fn delete_photos(
        state: &PhotoState,
        user_id: UserId,
        DeletePhotosParam { photo_ids }: DeletePhotosParam,
    ) -> Result<()> {
        metrics_group!();

        // 数据库方面
        let photos = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 查询照片信息
                let photos = PhotoMapper::query_by_ids(txn, &photo_ids).await?;

                // 鉴权
                if photos.iter().any(|p| p.user_id != user_id) {
                    return log_warn(
                        "del_photos_not_belong",
                        "用户尝试删除不属于它的照片",
                        AppError::bad_request("无法删除不属于自己的照片"),
                    )
                    .to_err();
                }

                // 删除收藏夹照片
                let affected_collections =
                    CollectionPhotoMapper::delete_by_photo_ids(txn, &photo_ids).await?;

                // 更新收藏夹照片计数
                CollectionMapper::update_photo_count_delta_batch(txn, &affected_collections)
                    .await?;

                // 删除照片评论点赞
                let comment_ids = CommentMapper::delete_by_photo_ids(txn, &photo_ids).await?;
                CommentLikeMapper::delete_by_comment_ids(txn, &comment_ids).await?;

                // 删除照片点赞
                PhotoLikeMapper::delete_all_by_photo_ids(txn, &photo_ids).await?;

                // 更新照片时间线统计
                TimelineStatMapper::decr_stat_by_created_ats(
                    txn,
                    &photos.iter().map(|p| &p.created_at).collect::<Vec<_>>(),
                )
                .await?;

                // todo 人脸部分

                // 删除数据库照片
                PhotoMapper::delete_by_ids(txn, &photo_ids).await?;

                Ok(photos)
            })
        })
        .timed(metrics_name!("db_transaction"))
        .await?;

        // 删除照片文件
        let file_ids = photos.iter().map(|p| &p.file_id).collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("s3_delete_batch"))
            .await?;

        metrics_success!();

        Ok(())
    }
}

/// 图片下载结果，Controller 根据此类型构建 HTTP 响应
pub(crate) enum ImageDownloadData {
    /// 处理后的图片（缩略图/预览/裁剪），始终为 webp 格式
    Processed(Bytes),
    /// 原始图片，以流式返回，动态内容类型
    Original {
        /// 图片字节流
        stream: Pin<Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>>,
        /// 根据文件扩展名推断的 MIME 类型
        content_type: &'static str,
    },
}

// 图片下载
impl PhotoService {
    /// 根据 ImageToken 下载图片，返回处理后的数据或原始流
    #[tracing::instrument(skip_all)]
    pub async fn download_image(
        state: &PhotoState,
        token: ImageToken,
    ) -> Result<ImageDownloadData> {
        metrics_group!();

        match token.token_type {
            ImageTokenType::Thumbnail | ImageTokenType::Preview | ImageTokenType::Crop => {
                let process_param: String = match token.token_type {
                    ImageTokenType::Thumbnail => "image/resize,w_300/format,webp".to_string(),
                    ImageTokenType::Preview => "image/resize,w_1920/format,webp".to_string(),
                    ImageTokenType::Crop => {
                        let bbox = token.bbox.ok_or_warn_bad_request(
                            "image_token_crop_info_not_found",
                            "token里面没有包含裁剪信息",
                            "token不包含裁剪信息",
                        )?;
                        let size = 200;
                        let (width, height) =
                            PhotoMapper::query_dimensions_by_file_id(&state.db, &token.file_id)
                                .await?
                                .ok_or_warn_bad_request(
                                    "photo_not_found",
                                    "裁剪图片不存在",
                                    "照片不存在",
                                )?;
                        let (x, y, w, h) = bbox.to_pixel_rect(width as u32, height as u32);
                        format!("image/crop,x_{x},y_{y},w_{w},h_{h}/resize,w_{size}/format,webp")
                    }
                    _ => unreachable!(),
                };
                let bytes = state
                    .s3_client
                    .download_with_process(&token.file_id, &process_param)
                    .timed(metrics_name!("s3_download_process"))
                    .await?;

                metrics_success!();
                Ok(ImageDownloadData::Processed(bytes))
            }
            ImageTokenType::Original => {
                let stream_resp = state
                    .s3_client
                    .get_download_stream_response(&token.file_id)
                    .timed(metrics_name!("s3_download_stream"))
                    .await?;

                let stream: Pin<
                    Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>,
                > = Box::pin(stream_resp);

                let content_type = Self::get_image_content_type(&token.file_id);
                metrics_success!();
                Ok(ImageDownloadData::Original {
                    stream,
                    content_type,
                })
            }
        }
    }

    /// 根据文件扩展名获取图片 MIME 类型
    fn get_image_content_type(file_id: &str) -> &'static str {
        let ext = file_id
            .split('.')
            .next_back()
            .unwrap_or("jpg")
            .to_lowercase();
        match ext.as_str() {
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            _ => "image/jpeg",
        }
    }
}
