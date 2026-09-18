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
    mappers::visual_mapper::VisualMapper,
    repo::VisualRepo,
    services::visual_service::{
        AfterVisualDelete, AfterVisualUpload, VisualDeleteContext, publish_after_visual_delete,
        publish_after_visual_upload, run_visual_delete_pipeline,
    },
    state::VisualState,
};
use audit::{AuditEvent, AuditRecorder};
use common::Result;
use types::visual::{
    VisualToken, VisualTokenType,
    dto::visual::{VisualCursorParam, VisualView},
    models::{DeleteVisualsParam, ExistsByHashBatchParam, UploadVisualParam},
};

use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    visual::visual::{NewVisualRecord, VisualId, VisualKind, VisualRecord},
};

pub struct VisualService;

// 查询
impl VisualService {
    #[instrument(skip_all)]
    #[common_macros::metered]
    pub async fn get_visual_info(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<VisualView> {
        Self::load_visuals_info(state, user_id, &[visual_id])
            .timed(metrics_name!("load_visuals_info"))
            .await?
            .pop()
            .ok_or_warn(
                "visual_not_exist",
                "用户尝试获取一个不存在的影像的信息",
                AppError::bad_request("影像不存在"),
            )?
            .to_ok()
    }

    /// 查询影像, 并生成包含访问令牌和点赞状态的视图.
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %visual_ids.len())
    )]
    pub async fn load_visuals_info(
        state: &VisualState,
        user_id: UserId,
        visual_ids: &[VisualId],
    ) -> Result<Vec<VisualView>> {
        // 获取影像记录 和 是否喜欢的id
        let (visuals, liked_visual_ids) =
            VisualRepo::load_visual_records(state, user_id, visual_ids).await?;

        // 组装结果
        let views = visuals
            .into_iter()
            .flatten()
            .map(|p| {
                let liked = liked_visual_ids.contains(&p.id);
                Ok(VisualView::from_record_with_tokens(p, user_id)?.with_liked(liked))
            })
            .collect::<contextual::Result<Vec<_>>>()?;
        Ok(views)
    }

    /// 游标获取影像列表.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_visual_cursor_page(
        state: &VisualState,
        user_id: UserId,
        req: VisualCursorParam,
    ) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
        // 获取visual_id
        let page = VisualRepo::query_visual_cursor_ids(state, req)
            .timed(metrics_name!("find_cursor_page_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载信息
        let visual_vos = Self::load_visuals_info(state, user_id, &page.records)
            .timed(metrics_name!("load_visuals_info"))
            .await?;

        // 组装结果
        Ok(page
            .replace_records(visual_vos)
            .with_next_cursor(|last_vo| TimeIdCursor {
                id: last_vo.id,
                time_at: last_vo.created_at,
            }))
    }
}

impl VisualService {
    /// 校验影像, 计算 BLAKE3, 上传文件并写入影像主记录.
    #[common_macros::metered]
    #[instrument(
        skip_all,
        fields(user_id = %user_id, file_name = %req.file_name)
    )]
    pub async fn upload_visual(
        state: Arc<VisualState>,
        user_id: UserId,
        file_data: Bytes,
        req: UploadVisualParam,
    ) -> Result<VisualView> {
        // 效验文件
        let metadata = {
            timed!("validate_visual", {
                FileValidator::validate_visual(&file_data, &req.file_name, &req.content_type)
                    .map_err(|error| {
                        ContextualError::warn_without_source(
                            "file_validation_error",
                            "文件校验失败",
                            AppError::bad_request(error.to_string()),
                        )
                    })?
            })
        };

        // 计算 BLAKE3
        let visual_hash = {
            let file_data_clone = Bytes::clone(&file_data);
            timed!(
                "blake3_hash",
                tokio::task::spawn_blocking(move || {
                    blake3::hash(&file_data_clone).to_hex().to_string()
                })
                .await
                .into_contextual()?
            )
        };
        // BLAKE3 去重校验
        if VisualRepo::exists_by_hash(&state, &visual_hash).await? {
            return Err(ContextualError::warn_without_source(
                "upload_visual:img_exist",
                "影像已存在",
                AppError::bad_request("影像已存在"),
            )
            .emit());
        }

        // 上传文件
        let file_id = Self::get_visual_s3_key(&metadata);
        state
            .s3_client
            .upload(&file_id, &file_data, &metadata.mime_type)
            .timed(metrics_name!("s3_upload"))
            .await
            .into_contextual()?;

        // 更新数据库
        let visual = if metadata.is_video() {
            NewVisualRecord {
                user_id,
                name: metadata.name,
                size: file_data.len() as u64,
                width: metadata.width,
                height: metadata.height,
                kind: VisualKind::Video,
                duration_ms: metadata.duration_ms.ok_or_warn(
                    "video_has_not_duration",
                    "未获取到视频的时长",
                    AppError::bad_request("获取视频时长失败"),
                )?,
                hash: visual_hash.clone(),
                file_id: file_id.clone(),
            }
        } else {
            NewVisualRecord {
                user_id,
                name: metadata.name,
                size: file_data.len() as u64,
                width: metadata.width,
                height: metadata.height,
                kind: VisualKind::Image,
                duration_ms: 0,
                hash: visual_hash.clone(),
                file_id: file_id.clone(),
            }
        };
        let visual = VisualRepo::insert_visual(state.as_ref(), visual)
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
        let visual_record = VisualRecord::from(visual);
        publish_after_visual_upload(
            Arc::clone(&state),
            AfterVisualUpload {
                visual: visual_record.clone(),
                #[cfg(feature = "face")]
                file_data,
            },
        );

        Ok(VisualView::from_record_with_tokens(visual_record, user_id)?)
    }

    /// 批量查询影像哈希值是否已存在.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(count = %req.hashes.len()))]
    pub async fn exists_by_hash_batch(
        state: &VisualState,
        req: ExistsByHashBatchParam,
    ) -> Result<Vec<bool>> {
        Ok(VisualRepo::exists_by_hash_batch(state, &req.hashes).await?)
    }

    /// 删除影像.
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %req.visual_ids.len())
    )]
    pub async fn delete_visuals(
        state: Arc<VisualState>,
        user_id: UserId,
        req: DeleteVisualsParam,
    ) -> Result<()> {
        // 查询属于用户的影像
        let visuals =
            VisualMapper::query_by_user_id_and_ids(&state.db, user_id, &req.visual_ids).await?;
        if visuals.len() != req.visual_ids.len() {
            Err(ContextualError::warn_without_source(
                "user_del_not_belong_visual",
                "用户尝试删除不属于自己的影像 或 影像不存在",
                AppError::bad_request("无法删除不属于自己的影像 或 影像不存在"),
            ))?;
        }
        let mut ctx = VisualDeleteContext { user_id, visuals };
        run_visual_delete_pipeline(&state.db, &mut ctx)
            .await
            .map_err(|error| {
                ContextualError::error(
                    "visual_delete_pipeline",
                    "执行影像删除事务失败",
                    error.to_string(),
                    error,
                )
            })?;

        // 发布删除后事件，缓存失效等后续操作不影响删除结果。
        publish_after_visual_delete(
            Arc::clone(&state),
            AfterVisualDelete {
                visuals: ctx.visuals,
            },
        );

        Ok(())
    }
}

/// 删除影像主表记录(受外键约束,`is_final` 使其恒在管道最后执行)
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "visual_record_delete",
    owns = ["VisualMapper"],
    is_final = true,
    method = on_visual_delete,
)]
impl VisualService {
    /// 执行影像删除管道的最后一步, 删除影像主表记录.
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut VisualDeleteContext,
    ) -> common::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();
        VisualMapper::delete_by_ids(txn, &visual_ids).await?;
        AuditRecorder::append(
            txn,
            AuditEvent::new("delete_visuals")
                .with_actor(ctx.user_id.0)
                .with_detail(serde_json::json!({
                    "visualIds": visual_ids.iter().map(|id| id.0).collect::<Vec<_>>()
                })),
        )
        .await?;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualDelete,
    slice = crate::services::visual_service::AFTER_MEDIA_DELETE_CONSUMERS,
    name = "visual_delete_cache_invalidation",
)]
impl VisualService {
    /// 删除影像后。
    #[instrument(name = "delete_visuals", skip_all)]
    async fn on_after_visual_delete(
        &self,
        state: Arc<VisualState>,
        event: Arc<AfterVisualDelete>,
    ) -> common::Result<()> {
        // 删除影像文件
        let file_ids = event
            .visuals
            .iter()
            .map(|visual| &visual.file_id)
            .collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("s3_delete_batch"))
            .await
            .into_contextual()?;

        VisualRepo::invalidate_deleted_visuals(state.as_ref(), &event.visuals).await;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualUpload,
    slice = crate::services::visual_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "visual_cursor_cache_invalidation",
)]
impl VisualService {
    /// 发布影像上传后的缓存失效事件.
    #[instrument(name = "upload_visual", skip_all)]
    async fn on_after_visual_upload(
        &self,
        state: Arc<VisualState>,
        _event: Arc<AfterVisualUpload>,
    ) -> common::Result<()> {
        VisualRepo::after_visual_upload(&state).await;
        Ok(())
    }
}

/// 影像下载结果，Controller 根据此类型构建 HTTP 响应
pub enum ImageDownloadData {
    /// 处理后的影像(缩略图/预览/裁剪/视频截帧),MIME 随处理方式变化
    Processed {
        /// 处理后的影像字节
        bytes: Bytes,
        /// 处理产物格式:图片处理为 `image/webp`,视频截帧为 `image/jpeg`
        content_type: &'static str,
    },
    /// 原始影像,以流式返回,动态内容类型
    Original {
        /// 影像字节流
        stream: Pin<Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>>,
        /// 根据已验证文件格式推断的 MIME 类型
        content_type: &'static str,
    },
}

// 影像下载
impl VisualService {
    /// 根据 VisualToken 下载影像,返回处理后的数据或原始流
    ///
    /// - 图片:缩略图/预览/人脸裁剪走 OSS 图片处理,原图走流式下载
    /// - 视频:缩略图/预览为按时长中点截取的封面帧(OSS `video/snapshot`),原视频走流式下载
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(viewer_id = %token.viewer_id, file_id = %token.file_id)
    )]
    pub async fn download_visual(
        state: &VisualState,
        token: VisualToken,
    ) -> Result<ImageDownloadData> {
        // 浏览埋点:仅预览/原图访问计入,缩略图/裁剪不计入
        if matches!(
            token.token_type,
            VisualTokenType::Preview | VisualTokenType::Original
        ) {
            let db = state.db.clone();
            let token = token.clone();
            tokio::spawn(async move {
                common::db_transaction!(contextual & db, |txn| {
                    let Some(visual_id) =
                        VisualMapper::query_visual_id_by_file_id(txn, &token.file_id).await?
                    else {
                        return Ok(());
                    };
                    AuditRecorder::append(
                        txn,
                        AuditEvent::new("view")
                            .with_actor(token.viewer_id.0)
                            .with_target("visual", visual_id.0),
                    )
                    .await?;
                    Ok(())
                })
                .await
            });
        }

        match (token.kind, &token.token_type) {
            // 图片:缩略图 / 预览 / 人脸裁剪,统一走 OSS 图片处理
            (
                VisualKind::Image,
                VisualTokenType::Thumbnail
                | VisualTokenType::Preview
                | VisualTokenType::Crop { .. },
            ) => {
                let process_param: String = match &token.token_type {
                    VisualTokenType::Thumbnail => "image/resize,w_300/format,webp".to_string(),
                    VisualTokenType::Preview => "image/resize,w_1920/format,webp".to_string(),
                    VisualTokenType::Crop {
                        bbox,
                        source_dimensions,
                    } => {
                        let size = 200;
                        let (x, y, w, h) =
                            bbox.to_pixel_rect(source_dimensions.width, source_dimensions.height);
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

                Ok(ImageDownloadData::Processed {
                    bytes,
                    content_type: "image/webp",
                })
            }
            // 视频:缩略图 / 预览为封面截帧,按时长中点取帧避免片头黑屏
            (VisualKind::Video, VisualTokenType::Thumbnail | VisualTokenType::Preview) => {
                let db = state.db.clone();
                let duration_ms = VisualMapper::query_duration_by_file_id(&db, &token.file_id)
                    .await?
                    .ok_or_warn(
                        "video_file_id_not_found",
                        "视频文件不存在",
                        AppError::not_found("视频文件不存在"),
                    )?;
                let t_ms = (duration_ms / 2).max(1);
                let process_param = format!("video/snapshot,t_{t_ms},f_jpg,w_640,m_fast");
                let bytes = state
                    .s3_client
                    .download_with_process(&token.file_id, &process_param)
                    .timed(metrics_name!("s3_download_process"))
                    .await
                    .into_contextual()?;

                Ok(ImageDownloadData::Processed {
                    bytes,
                    content_type: "image/jpeg",
                })
            }
            // 原图 / 原视频:流式下载,MIME 按文件扩展名推断
            (_, VisualTokenType::Original) => {
                let stream_resp = state
                    .s3_client
                    .get_download_stream_response(&token.file_id)
                    .timed(metrics_name!("s3_download_stream"))
                    .await
                    .into_contextual()?;

                let stream: Pin<
                    Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>,
                > = Box::pin(stream_resp);

                let content_type = match token.kind {
                    VisualKind::Image => {
                        FileValidator::image_content_type(&token.file_id).unwrap_or("image/jpeg")
                    }
                    VisualKind::Video => {
                        FileValidator::video_content_type(&token.file_id).unwrap_or("video/mp4")
                    }
                };
                Ok(ImageDownloadData::Original {
                    stream,
                    content_type,
                })
            }
            // 视频不支持人脸裁剪
            (VisualKind::Video, VisualTokenType::Crop { .. }) => {
                Err(AppError::bad_request("视频不支持裁剪 token"))
            }
        }
    }

    #[inline]
    fn get_visual_s3_key(metadata: &FileMetaData) -> String {
        let date_path = common::time::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        format!("visuals/{}/{}.{}", date_path, uuid, metadata.format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::visual_service::{
        AFTER_MEDIA_DELETE_CONSUMERS, AFTER_MEDIA_UPLOAD_CONSUMERS, MEDIA_DELETE_STEPS,
    };
    use common::pipeline::Step;

    /// 验证 `linkme` 定义即注册:全部清理步骤均被收集,且存在唯一的 final 步骤(主表删除)
    #[test]
    fn step_registry_collects_all_steps() {
        let steps: Vec<&'static dyn Step<VisualDeleteContext>> = MEDIA_DELETE_STEPS.to_vec();

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
                .any(|consumer| consumer.name() == "visual_cursor_cache_invalidation")
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
                .any(|consumer| consumer.name() == "visual_delete_cache_invalidation")
        );
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "timeline_stat_cache_invalidation")
        );
    }
}
