use std::{pin::Pin, sync::Arc};

use bytes::Bytes;
use common::{
    error::{
        AppError, ContextualError,
        contextual::{
            self,
            ext::{ContextualResultExt, IntoContextualExt, OptionExt},
        },
    },
    ext::{ResultInspectErrAsync, ToOk},
    metrics_name, timed,
    types::CursorPage,
    utils::MetricsTimerExt,
};
use file_validator::{FileMetaData, FileValidator};
use futures::Stream;
use oss::OssError;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    mappers::media_mapper::MediaMapper,
    repo::MediaRepo,
    services::media_service::{
        AfterMediaDelete, AfterMediaUpload, MediaDeleteContext, publish_after_media_delete,
        publish_after_media_upload, run_media_delete_pipeline,
    },
    state::MediaState,
};
use audit::{AuditEvent, AuditRecorder};
use common::Result;
use types::media::{
    ImageToken, ImageTokenType,
    dto::media::{MediaCursorParam, MediaView},
    models::{DeleteMediasParam, ExistsByMd5BatchParam, UploadMediaParam},
};

use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    media::media::{MediaId, MediaKind, MediaRecord, NewMediaRecord},
};

pub struct MediaService;

// 查询
impl MediaService {
    #[instrument(skip_all)]
    #[common_macros::metered]
    pub async fn get_media_info(
        state: &MediaState,
        user_id: UserId,
        media_id: MediaId,
    ) -> Result<MediaView> {
        Self::load_medias_info(state, user_id, &[media_id])
            .timed(metrics_name!("load_medias_info"))
            .await?
            .pop()
            .ok_or_warn(
                "media_not_exist",
                "用户尝试获取一个不存在的媒体的信息",
                AppError::bad_request("媒体不存在"),
            )?
            .to_ok()
    }

    /// 查询媒体, 并生成包含访问令牌和点赞状态的视图.
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %media_ids.len())
    )]
    pub async fn load_medias_info(
        state: &MediaState,
        user_id: UserId,
        media_ids: &[MediaId],
    ) -> Result<Vec<MediaView>> {
        // 获取媒体记录 和 是否喜欢的id
        let (medias, liked_media_ids) =
            MediaRepo::load_media_records(state, user_id, media_ids).await?;

        // 组装结果
        let views = medias
            .into_iter()
            .flatten()
            .map(|p| {
                let liked = liked_media_ids.contains(&p.id);
                Ok(MediaView::from_record_with_tokens(p, user_id)?.with_liked(liked))
            })
            .collect::<contextual::Result<Vec<_>>>()?;
        Ok(views)
    }

    /// 游标获取媒体列表.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_media_cursor_page(
        state: &MediaState,
        user_id: UserId,
        req: MediaCursorParam,
    ) -> Result<CursorPage<MediaView, TimeIdCursor<MediaId>>> {
        // 获取media_id
        let page = MediaRepo::query_media_cursor_ids(state, req)
            .timed(metrics_name!("find_cursor_page_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载信息
        let media_vos = Self::load_medias_info(state, user_id, &page.records)
            .timed(metrics_name!("load_medias_info"))
            .await?;

        // 组装结果
        Ok(page
            .replace_records(media_vos)
            .with_next_cursor(|last_vo| TimeIdCursor {
                id: last_vo.id,
                time_at: last_vo.created_at,
            }))
    }
}

impl MediaService {
    /// 校验媒体, 计算 MD5, 上传文件并写入媒体主记录.
    #[common_macros::metered]
    #[instrument(
        skip_all,
        fields(user_id = %user_id, file_name = %req.file_name)
    )]
    pub async fn upload_media(
        state: Arc<MediaState>,
        user_id: UserId,
        file_data: Bytes,
        req: UploadMediaParam,
    ) -> Result<MediaView> {
        // 效验文件
        let metadata = {
            timed!("validate_media", {
                FileValidator::validate_media(&file_data, &req.file_name, &req.content_type)
                    .map_err(|error| {
                        ContextualError::warn_without_source(
                            "file_validation_error",
                            "文件校验失败",
                            AppError::bad_request(error.to_string()),
                        )
                    })?
            })
        };

        // 计算md5
        let md5_hash = {
            let file_data_clone = Bytes::clone(&file_data);
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
        // MD5 去重校验
        if MediaRepo::exists_by_md5(state.as_ref(), &md5_hash).await? {
            return Err(ContextualError::warn_without_source(
                "upload_media:img_exist",
                "图片已存在",
                AppError::bad_request("图片已存在"),
            )
            .emit());
        }

        // 上传文件
        let file_id = Self::get_media_s3_key(&metadata);
        state
            .s3_client
            .upload(&file_id, &file_data, &metadata.mime_type)
            .timed(metrics_name!("s3_upload"))
            .await
            .into_contextual()?;

        // 更新数据库
        let media = if metadata.is_video() {
            NewMediaRecord {
                user_id,
                name: metadata.name,
                size: file_data.len() as u64,
                width: metadata.width,
                height: metadata.height,
                kind: MediaKind::Video,
                duration_ms: metadata.duration_ms.ok_or_warn(
                    "video_has_not_duration",
                    "未获取到视频的时长",
                    AppError::bad_request("获取视频时长失败"),
                )?,
                md5: md5_hash.clone(),
                file_id: file_id.clone(),
            }
        } else {
            NewMediaRecord {
                user_id,
                name: metadata.name,
                size: file_data.len() as u64,
                width: metadata.width,
                height: metadata.height,
                kind: MediaKind::Image,
                duration_ms: 0,
                md5: md5_hash.clone(),
                file_id: file_id.clone(),
            }
        };
        let media = MediaRepo::insert_media(state.as_ref(), media)
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
            .into_contextual()?;

        // 发布事件
        let media_record = MediaRecord::from(media);
        publish_after_media_upload(
            Arc::clone(&state),
            AfterMediaUpload {
                media: media_record.clone(),
                #[cfg(feature = "face")]
                file_data,
            },
        );

        Ok(MediaView::from_record_with_tokens(media_record, user_id)?)
    }

    /// 批量查询图片 MD5 是否已存在.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(count = %req.md5s.len()))]
    pub async fn exists_by_md5_batch(
        state: &MediaState,
        req: ExistsByMd5BatchParam,
    ) -> Result<Vec<bool>> {
        Ok(MediaRepo::exists_by_md5_batch(state, &req.md5s).await?)
    }

    /// 删除媒体.
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %req.media_ids.len())
    )]
    pub async fn delete_medias(
        state: Arc<MediaState>,
        user_id: UserId,
        req: DeleteMediasParam,
    ) -> Result<()> {
        // 查询属于用户的媒体
        let medias =
            MediaMapper::query_by_user_id_and_ids(&state.db, user_id, &req.media_ids).await?;
        if medias.len() != req.media_ids.len() {
            Err(ContextualError::warn_without_source(
                "user_del_not_belong_media",
                "用户尝试删除不属于自己的媒体 或 媒体不存在",
                AppError::bad_request("无法删除不属于自己的媒体 或 媒体不存在"),
            ))?;
        }
        let mut ctx = MediaDeleteContext { user_id, medias };
        run_media_delete_pipeline(&state.db, &mut ctx)
            .await
            .map_err(|error| {
                ContextualError::error(
                    "media_delete_pipeline",
                    "执行媒体删除事务失败",
                    error.to_string(),
                    error,
                )
            })?;

        // 发布删除后事件，缓存失效等后续操作不影响删除结果。
        publish_after_media_delete(Arc::clone(&state), AfterMediaDelete { medias: ctx.medias });

        Ok(())
    }
}

/// 删除媒体主表记录(受外键约束,`is_final` 使其恒在管道最后执行)
#[step_derive::declare_transaction_step(
    ctx = crate::services::media_service::MediaDeleteContext,
    slice = crate::services::media_service::MEDIA_DELETE_STEPS,
    name = "media_record_delete",
    owns = ["MediaMapper"],
    is_final = true,
    method = on_media_delete,
)]
impl MediaService {
    /// 执行媒体删除管道的最后一步, 删除媒体主表记录.
    async fn on_media_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut MediaDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let media_ids = ctx.media_ids();
        MediaMapper::delete_by_ids(txn, &media_ids).await?;
        AuditRecorder::append(
            txn,
            AuditEvent::new("delete_medias")
                .with_actor(ctx.user_id.0)
                .with_detail(serde_json::json!({
                    "mediaIds": media_ids.iter().map(|id| id.0).collect::<Vec<_>>()
                })),
        )
        .await?;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::MediaState,
    event = crate::services::media_service::AfterMediaDelete,
    slice = crate::services::media_service::AFTER_MEDIA_DELETE_CONSUMERS,
    name = "media_delete_cache_invalidation",
)]
impl MediaService {
    /// 删除媒体后。
    #[instrument(name = "delete_medias", skip_all)]
    async fn on_after_media_delete(
        &self,
        state: Arc<MediaState>,
        event: Arc<AfterMediaDelete>,
    ) -> common::Result<()> {
        // 删除媒体文件
        let file_ids = event
            .medias
            .iter()
            .map(|media| &media.file_id)
            .collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("s3_delete_batch"))
            .await
            .into_contextual()?;

        MediaRepo::invalidate_deleted_medias(state.as_ref(), &event.medias).await;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::MediaState,
    event = crate::services::media_service::AfterMediaUpload,
    slice = crate::services::media_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "media_cursor_cache_invalidation",
)]
impl MediaService {
    /// 发布媒体上传后的缓存失效事件.
    #[instrument(name = "upload_media", skip_all)]
    async fn on_after_media_upload(
        &self,
        state: Arc<MediaState>,
        _event: Arc<AfterMediaUpload>,
    ) -> common::Result<()> {
        MediaRepo::after_media_upload(&state).await;
        Ok(())
    }
}

/// 图片下载结果，Controller 根据此类型构建 HTTP 响应
pub enum ImageDownloadData {
    /// 处理后的图片（缩略图/预览/裁剪），始终为 webp 格式
    Processed(Bytes),
    /// 原始图片，以流式返回，动态内容类型
    Original {
        /// 图片字节流
        stream: Pin<Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>>,
        /// 根据已验证文件格式推断的 MIME 类型
        content_type: &'static str,
    },
}

// 图片下载
impl MediaService {
    /// 根据 ImageToken 下载图片，返回处理后的数据或原始流
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(viewer_id = %token.viewer_id, file_id = %token.file_id)
    )]
    pub async fn download_image(
        state: &MediaState,
        token: ImageToken,
    ) -> Result<ImageDownloadData> {
        // 浏览埋点：仅预览/原图访问计入，缩略图/裁剪不计入
        if matches!(
            token.token_type,
            ImageTokenType::Preview | ImageTokenType::Original
        ) {
            let db = state.db.clone();
            let token = token.clone();
            tokio::spawn(async move {
                common::db_transaction!(contextual & db, |txn| {
                    let Some(media_id) =
                        MediaMapper::query_media_id_by_file_id(txn, &token.file_id).await?
                    else {
                        return Ok(());
                    };
                    AuditRecorder::append(
                        txn,
                        AuditEvent::new("view")
                            .with_actor(token.viewer_id.0)
                            .with_target("media", media_id.0),
                    )
                    .await?;
                    Ok(())
                })
                .await
            });
        }

        match token.token_type {
            ImageTokenType::Thumbnail | ImageTokenType::Preview | ImageTokenType::Crop => {
                let process_param: String = match token.token_type {
                    ImageTokenType::Thumbnail => "image/resize,w_300/format,webp".to_string(),
                    ImageTokenType::Preview => "image/resize,w_1920/format,webp".to_string(),
                    ImageTokenType::Crop => {
                        let bbox = token.bbox.ok_or_warn(
                            "image_token_crop_info_not_found",
                            "token里面没有包含裁剪信息",
                            AppError::bad_request("token不包含裁剪信息"),
                        )?;
                        let size = 200;
                        let dimensions = token.source_dimensions.ok_or_warn(
                            "image_token_dimensions_not_found",
                            "裁剪 token 中没有包含原图尺寸",
                            AppError::bad_request("裁剪 token 缺少原图尺寸"),
                        )?;
                        let (x, y, w, h) = bbox.to_pixel_rect(dimensions.width, dimensions.height);
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

                let content_type =
                    FileValidator::image_content_type(&token.file_id).unwrap_or("image/jpeg");
                Ok(ImageDownloadData::Original {
                    stream,
                    content_type,
                })
            }
        }
    }

    #[inline]
    fn get_media_s3_key(metadata: &FileMetaData) -> String {
        let date_path = common::time::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        format!("medias/{}/{}.{}", date_path, uuid, metadata.format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::media_service::{
        AFTER_MEDIA_DELETE_CONSUMERS, AFTER_MEDIA_UPLOAD_CONSUMERS, MEDIA_DELETE_STEPS,
    };
    use common::pipeline::Step;

    /// 验证 `linkme` 定义即注册:全部清理步骤均被收集,且存在唯一的 final 步骤(主表删除)
    #[test]
    fn step_registry_collects_all_steps() {
        let steps: Vec<&'static dyn Step<MediaDeleteContext>> = MEDIA_DELETE_STEPS.to_vec();

        #[cfg(feature = "face")]
        assert_eq!(steps.len(), 6);
        #[cfg(not(feature = "face"))]
        assert_eq!(steps.len(), 5);

        let finals: Vec<_> = steps.iter().filter(|step| step.is_final()).collect();
        assert_eq!(finals.len(), 1);
    }

    #[test]
    fn after_upload_registry_collects_all_consumers() {
        let consumers = AFTER_MEDIA_UPLOAD_CONSUMERS.to_vec();

        #[cfg(feature = "face")]
        assert_eq!(consumers.len(), 3);
        #[cfg(not(feature = "face"))]
        assert_eq!(consumers.len(), 2);
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "media_cursor_cache_invalidation")
        );
        #[cfg(feature = "face")]
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "face_recognition")
        );
    }

    #[test]
    fn after_delete_registry_collects_cache_consumers() {
        let consumers = AFTER_MEDIA_DELETE_CONSUMERS.to_vec();

        assert_eq!(consumers.len(), 2);
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "media_delete_cache_invalidation")
        );
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "timeline_stat_cache_invalidation")
        );
    }
}
