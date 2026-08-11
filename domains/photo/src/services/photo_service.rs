use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use chrono::Utc;
use common::{
    error::AppError,
    ext::{OkExt, OptionExt, ResultInspectErrAsync, log_warn},
    inc_error, metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    timed,
    utils::{FileValidator, MetricsTimerExt, token_cipher},
};
use constants::RedisKeys;
use futures::Stream;
use oss::OssError;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    mappers::{
        photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper,
        timeline_stat_mapper::TimelineStatMapper,
    },
    state::PhotoState,
};
use common::Result;
use types::photo::{
    ImageToken, ImageTokenType,
    dto::photo::{PhotoCursorParam, PhotoView},
    models::{DeletePhotosParam, ExistsByMd5BatchParam, UploadPhotoParam},
};

#[cfg(feature = "face")]
use types::photo::person::PersonId;
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::photo::{ActiveModel, PhotoId, PhotoRecord},
};

pub(crate) struct PhotoService;

// 查询
impl PhotoService {
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %photo_ids.len())
    )]
    pub async fn load_photos_info(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<PhotoView>> {
        let (photos_result, liked_photo_ids_result) = tokio::join!(
            state.cache_photo_info.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_info(*id),
                Duration::from_secs(24 * 60 * 60),
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
                    .with_tokens(&file_id, user_id, token_cipher())
            })
            .collect::<Vec<_>>()
            .to_ok()
    }

    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_photo_cursor_page(
        state: &PhotoState,
        user_id: UserId,
        req: PhotoCursorParam,
    ) -> Result<CursorPage<PhotoView, String>> {
        metrics_group!();

        // 获取photo_ids
        let photo_ids = PhotoMapper::query_cursor_page_ids(
            &state.db,
            req.cursor,
            req.size,
            req.direction,
            req.anchor_time,
        )
        .timed(metrics_name!("find_cursor_page_ids"))
        .await?;
        if photo_ids.is_empty() {
            return Ok(CursorPage::empty());
        }

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, req.size);

        let photo_vos = Self::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        // 获取next_cursor
        let next_cursor = if has_more {
            photo_vos.last().map(|last_vo| {
                TimeIdCursor {
                    id: last_vo.id,
                    created_at: last_vo.created_at,
                }
                .encode()
            })
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
    #[instrument(
        skip_all,
        fields(user_id = %user_id, file_name = %req.file_name)
    )]
    pub async fn upload_photo(
        state: &PhotoState,
        user_id: UserId,
        file_data: Bytes,
        req: UploadPhotoParam,
    ) -> Result<PhotoView> {
        metrics_group!();

        // 效验文件
        let metadata = {
            timed!("validate_photo", {
                FileValidator::validate_image(&file_data, &req.file_name, &req.content_type)
                    .inspect_err(|_| inc_error!("validation"))?
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
        // 带三级缓存的 MD5 去重校验
        let md5_cache_key = RedisKeys::photo::photo::photo_md5(&md5_hash);
        let md5_hash_for_check = md5_hash.clone();
        let exists = state
            .cache_photo_md5
            .get_or_load(
                md5_cache_key.as_str(),
                Duration::from_secs(24 * 60 * 60),
                || {
                    Box::pin(async move {
                        PhotoMapper::exists_by_md5(&state.db, &md5_hash_for_check).await
                    })
                },
            )
            .timed(metrics_name!("cache_get_or_load"))
            .await?;
        if exists {
            inc_error!("conflict");
            return Err(log_warn(
                "upload_photo:img_exist",
                "图片已存在",
                AppError::bad_request("图片已存在"),
            ));
        }

        // 上传文件
        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);
        state
            .s3_client
            .upload(&file_id, &file_data, &metadata.mime_type)
            .timed(metrics_name!("s3_upload"))
            .await
            .inspect_err(|_| inc_error!("s3"))?;

        // 更新数据库
        let now = Utc::now();
        let photo = ActiveModel {
            user_id: Set(user_id),
            name: Set(metadata.name),
            size: Set(file_data.len() as i64),
            width: Set(metadata.width as i32),
            height: Set(metadata.height as i32),
            mime_type: Set(metadata.mime_type),
            md5: Set(md5_hash),
            file_id: Set(file_id.clone()),
            created_at: Set(req.created_at.unwrap_or(now)),
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
        .await
        .inspect_err(|_| inc_error!("db"))?;

        // 增加时间线统计
        // 错误不返回
        let _ = TimelineStatMapper::incr_stat(&state.db, photo.created_at).await;

        // 上传成功: 覆盖 MD5 去重缓存, 并失效月度统计缓存
        // 错误不返回
        let _ = tokio::join!(
            state
                .cache_photo_md5
                .put(&md5_cache_key, true, Duration::from_secs(24 * 60 * 60))
                .timed(metrics_name!("cache_put")),
            state
                .cache_timeline_stat
                .invalidate(RedisKeys::photo::timeline_stat::monthly_stats())
                .timed(metrics_name!("cache_invalidate"))
        );

        metrics_success!();

        let file_id = photo.file_id.clone();
        PhotoView::from(PhotoRecord::from(photo))
            .with_tokens(&file_id, user_id, token_cipher())
            .to_ok()
    }

    #[tracing::instrument(skip_all, fields(count = %req.md5s.len()))]
    pub async fn exists_by_md5_batch(
        state: &PhotoState,
        req: ExistsByMd5BatchParam,
    ) -> Result<Vec<bool>> {
        metrics_group!();

        let existing = PhotoMapper::exists_by_md5_batch(&state.db, &req.md5s).await?;
        let res = req
            .md5s
            .iter()
            .map(|md5| existing.contains(md5))
            .collect::<Vec<bool>>();

        metrics_success!();
        Ok(res)
    }

    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %req.photo_ids.len())
    )]
    pub async fn delete_photos(
        state: &PhotoState,
        user_id: UserId,
        req: DeletePhotosParam,
    ) -> Result<()> {
        metrics_group!();

        // 查询照片信息并鉴权
        let photos =
            PhotoMapper::query_by_user_id_and_ids(&state.db, user_id, &req.photo_ids).await?;

        // 在单个事务内执行删除步骤管道(主表删除恒在最后),任一步失败整体回滚
        let mut ctx = PhotoDeleteContext {
            photos,
            #[cfg(feature = "face")]
            person_ids: Vec::new(),
        };
        DELETE_PIPELINE
            .run(&state.db, &mut ctx)
            .timed(metrics_name!("db_transaction"))
            .await?;

        // 删除照片文件
        let file_ids = ctx.photos.iter().map(|p| &p.file_id).collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("s3_delete_batch"))
            .await?;

        // 失效照片信息缓存（L1 + L2）
        // 缓存键按照片拆分, 删除时逐一失效
        let cache_keys = ctx
            .photos
            .iter()
            .map(|p| RedisKeys::photo::photo::photo_info(p.id))
            .collect::<Vec<_>>();
        let dim_keys = ctx
            .photos
            .iter()
            .map(|p| RedisKeys::photo::photo::photo_dimensions(&p.file_id))
            .collect::<Vec<_>>();

        // 失效照片信息、照片尺寸、人物缓存, 并失效月度统计缓存
        // 错误不返回
        #[cfg(feature = "face")]
        let person_keys = ctx
            .person_ids
            .iter()
            .map(|&pid| RedisKeys::photo::person::person_info(pid))
            .collect::<Vec<_>>();
        #[cfg(feature = "face")]
        let _ = tokio::join!(
            state
                .cache_photo_info
                .invalidate_batch(&cache_keys)
                .timed(metrics_name!("cache_invalidate")),
            state
                .cache_photo_dimensions
                .invalidate_batch(&dim_keys)
                .timed(metrics_name!("cache_invalidate_dimensions")),
            state
                .cache_timeline_stat
                .invalidate(RedisKeys::photo::timeline_stat::monthly_stats())
                .timed(metrics_name!("cache_invalidate_timeline")),
            state
                .cache_person
                .invalidate_batch(&person_keys)
                .timed(metrics_name!("cache_invalidate_person"))
        );
        #[cfg(not(feature = "face"))]
        let _ = tokio::join!(
            state
                .cache_photo_info
                .invalidate_batch(&cache_keys)
                .timed(metrics_name!("cache_invalidate")),
            state
                .cache_photo_dimensions
                .invalidate_batch(&dim_keys)
                .timed(metrics_name!("cache_invalidate_dimensions")),
            state
                .cache_timeline_stat
                .invalidate(RedisKeys::photo::timeline_stat::monthly_stats())
                .timed(metrics_name!("cache_invalidate_timeline"))
        );

        metrics_success!();

        Ok(())
    }
}

/// 照片删除步骤共享上下文(由 `PhotoService::delete_photos` 提前查询并鉴权后填充)
pub(crate) struct PhotoDeleteContext {
    pub photos: Vec<PhotoRecord>,
    /// 受影响人物 ID（由人脸清理步骤填充, 删除后用于失效人物缓存）
    #[cfg(feature = "face")]
    pub person_ids: Vec<PersonId>,
}

impl PhotoDeleteContext {
    pub fn photo_ids(&self) -> Vec<PhotoId> {
        self.photos.iter().map(|p| p.id).collect()
    }
}

step_derive::declare_pipeline!(PhotoDeleteContext, PHOTO_DELETE_STEPS, DELETE_PIPELINE);

/// 删除照片主表记录(受外键约束,`is_final` 使其恒在管道最后执行)
#[step_derive::declare_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "photo_record_delete",
    owns = ["PhotoMapper"],
    is_final = true,
)]
impl PhotoService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        PhotoMapper::delete_by_ids(txn, &ctx.photo_ids()).await?;
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
    #[tracing::instrument(
        skip_all,
        fields(viewer_id = %token.viewer_id, file_id = %token.file_id)
    )]
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
                        let file_id_for_cache = token.file_id.clone();
                        let (width, height) = state
                            .cache_photo_dimensions
                            .get_or_load(
                                RedisKeys::photo::photo::photo_dimensions(&file_id_for_cache)
                                    .as_str(),
                                Duration::from_secs(24 * 60 * 60),
                                || {
                                    Box::pin(async move {
                                        PhotoMapper::query_dimensions_by_file_id(
                                            &state.db,
                                            &file_id_for_cache,
                                        )
                                        .await?
                                        .ok_or_warn_bad_request(
                                            "photo_not_found",
                                            "裁剪图片不存在",
                                            "照片不存在",
                                        )
                                    })
                                },
                            )
                            .timed(metrics_name!("cache_get_or_load"))
                            .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use common::pipeline::Step;

    /// 验证 `linkme` 定义即注册:全部清理步骤均被收集,且存在唯一的 final 步骤(主表删除)
    #[test]
    fn step_registry_collects_all_steps() {
        let steps: Vec<&'static dyn Step<PhotoDeleteContext>> = PHOTO_DELETE_STEPS.to_vec();

        #[cfg(feature = "face")]
        assert_eq!(steps.len(), 6);
        #[cfg(not(feature = "face"))]
        assert_eq!(steps.len(), 5);

        let finals: Vec<_> = steps.iter().filter(|step| step.is_final()).collect();
        assert_eq!(finals.len(), 1);
    }
}
