use std::pin::Pin;

use bytes::Bytes;
use chrono::Utc;
use common::{
    error::AppError,
    ext::{ContextualResultExt, IntoContextualExt, OptionExt, ResultInspectErrAsync, log_warn},
    inc_error, metrics_name,
    models::CursorPage,
    timed,
    utils::{FileValidator, MetricsTimerExt, token_cipher},
};
use futures::Stream;
use oss::OssError;
use sea_orm::ActiveValue::Set;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    mappers::photo_mapper::PhotoMapper, repo::photo_repo::PhotoDeleteContext, state::PhotoState,
};
use common::Result;
use types::photo::{
    ImageToken, ImageTokenType,
    dto::photo::{PhotoCursorParam, PhotoView},
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
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %photo_ids.len())
    )]
    pub async fn load_photos_info(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<PhotoView>> {
        let (photos, liked_photo_ids) = state.repo.load_photo_records(user_id, photo_ids).await?;
        let views = photos
            .into_iter()
            .flatten()
            .map(|p| {
                let liked = liked_photo_ids.contains(&p.id);
                Ok(
                    PhotoView::from_record_with_tokens(p, user_id, token_cipher())?
                        .with_liked(liked),
                )
            })
            .collect::<common::error::contextual::Result<Vec<_>>>()?;
        Ok(views)
    }

    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_photo_cursor_page(
        state: &PhotoState,
        user_id: UserId,
        req: PhotoCursorParam,
    ) -> Result<CursorPage<PhotoView, String>> {
        // 获取photo_ids
        let size = req.size;
        let photo_ids = state
            .repo
            .query_photo_cursor_ids(req)
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

        Ok(CursorPage {
            records: photo_vos,
            next_cursor,
            has_more,
        })
    }
}

impl PhotoService {
    #[common::metered]
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
                .await
                .into_contextual()?
            )
        };
        // 带三级缓存的 MD5 去重校验
        let exists = state.repo.exists_by_md5(&md5_hash).await?;
        if exists {
            return inc_error!("conflict" => log_warn(
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
            .inspect_err(|_| inc_error!("s3"))
            .into_contextual()?;

        // 更新数据库
        let now = Utc::now();
        let photo = state
            .repo
            .insert_photo(ActiveModel {
                user_id: Set(user_id),
                name: Set(metadata.name),
                size: Set(file_data.len() as i64),
                width: Set(metadata.width as i32),
                height: Set(metadata.height as i32),
                mime_type: Set(metadata.mime_type),
                md5: Set(md5_hash.clone()),
                file_id: Set(file_id.clone()),
                created_at: Set(req.created_at.unwrap_or(now)),
                updated_at: Set(now),
                ..Default::default()
            })
            .timed(metrics_name!("db_insert"))
            .await
            .inspect_err_async(|_| async {
                state
                    .s3_client
                    .delete(&file_id)
                    .await
                    .into_contextual()
                    .emit_if_err();
            })
            .await
            .inspect_err(|_| inc_error!("db"))
            .into_contextual()?;

        // 增加时间线统计、覆盖 MD5 缓存并失效月度统计缓存；失败不阻断上传
        state
            .repo
            .record_uploaded_photo(&md5_hash, photo.created_at)
            .await;

        Ok(PhotoView::from_record_with_tokens(
            PhotoRecord::from(photo),
            user_id,
            token_cipher(),
        )?)
    }

    #[common::metered]
    #[tracing::instrument(skip_all, fields(count = %req.md5s.len()))]
    pub async fn exists_by_md5_batch(
        state: &PhotoState,
        req: ExistsByMd5BatchParam,
    ) -> Result<Vec<bool>> {
        Ok(state.repo.exists_by_md5_batch(&req.md5s).await?)
    }

    #[common::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %req.photo_ids.len())
    )]
    pub async fn delete_photos(
        state: &PhotoState,
        user_id: UserId,
        req: DeletePhotosParam,
    ) -> Result<()> {
        // 在单个事务内执行删除步骤管道(主表删除恒在最后),任一步失败整体回滚
        let ctx = state
            .repo
            .delete_photos(user_id, &req.photo_ids)
            .timed(metrics_name!("db_transaction"))
            .await?;

        // 删除照片文件
        let file_ids = ctx.photos.iter().map(|p| &p.file_id).collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("s3_delete_batch"))
            .await
            .into_contextual()?;

        // 失效照片信息、照片尺寸、人物缓存, 并失效月度统计缓存
        // 错误不返回
        state
            .repo
            .invalidate_deleted_photos(
                &ctx.photos,
                #[cfg(feature = "face")]
                &ctx.person_ids,
            )
            .await;

        Ok(())
    }
}

/// 删除照片主表记录(受外键约束,`is_final` 使其恒在管道最后执行)
#[step_derive::declare_step(
    ctx = crate::repo::photo_repo::PhotoDeleteContext,
    slice = crate::repo::photo_repo::PHOTO_DELETE_STEPS,
    name = "photo_record_delete",
    owns = ["PhotoMapper"],
    is_final = true,
)]
impl PhotoService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut PhotoDeleteContext,
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
    #[common::metered]
    #[tracing::instrument(
        skip_all,
        fields(viewer_id = %token.viewer_id, file_id = %token.file_id)
    )]
    pub async fn download_image(
        state: &PhotoState,
        token: ImageToken,
    ) -> Result<ImageDownloadData> {
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
                            state.repo.get_photo_dimensions(&token.file_id).await?;
                        let (x, y, w, h) = bbox.to_pixel_rect(width as u32, height as u32);
                        format!("image/crop,x_{x},y_{y},w_{w},h_{h}/resize,w_{size}/format,webp")
                    }
                    _ => unreachable!(),
                };
                let bytes = state
                    .s3_client
                    .download_with_process(&token.file_id, &process_param)
                    .timed(metrics_name!("s3_download_process"))
                    .await
                    .into_contextual()?;

                Ok(ImageDownloadData::Processed(bytes))
            }
            ImageTokenType::Original => {
                let stream_resp = state
                    .s3_client
                    .get_download_stream_response(&token.file_id)
                    .timed(metrics_name!("s3_download_stream"))
                    .await
                    .into_contextual()?;

                let stream: Pin<
                    Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>,
                > = Box::pin(stream_resp);

                let content_type = Self::get_image_content_type(&token.file_id);
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
    use crate::repo::photo_repo::PHOTO_DELETE_STEPS;
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
