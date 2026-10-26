use std::pin::Pin;

use audit::{AuditEvent, AuditRecorder};
use bytes::Bytes;
use common_core::{
    Result,
    error::{
        AppError, ContextualError,
        contextual::ext::{IntoContextualExt, OptionExt},
    },
};
use common_metrics::{MetricsTimerExt, metrics_name};
use file_validator::FileValidator;
use futures::Stream;
use oss::OssError;
use types_visual::{
    ImageTokenType, VideoTokenType, VisualToken, VisualTokenKind,
    derivative::{DerivativeKind, DerivativeStatus},
    visual::VisualKind,
};

use crate::{
    mappers::{derivative_mapper::DerivativeMapper, visual_mapper::VisualMapper},
    services::{media, visual_service::VisualService},
    state::VisualState,
};

/// 影像下载结果，Controller 根据此类型构建 HTTP 响应
pub enum ImageDownloadData {
    /// 已完整载入内存的字节(图片缩略图/裁剪、视频缩略片)
    Buffered {
        /// 完整字节
        bytes: Bytes,
        /// 实际 MIME
        content_type: &'static str,
    },
    Streamed {
        /// 对象存储字节流(原图 / 视频衍生片 / 处理后的图片)
        stream: Pin<Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>>,
        /// 对象存储返回的 MIME
        content_type: String,
        /// 对象存储返回的状态码(200 全量 / 206 部分内容 / 416 范围越界)
        status: u16,
        /// `Content-Range` 响应头(仅 206 / 416)
        content_range: Option<String>,
        /// 本次响应体的字节数
        content_length: Option<u64>,
    },
    /// 视频衍生片尚未生成完成(异步转码中), 客户端应稍后重试
    NotReady,
}

impl VisualService {
    /// 依 VisualToken 下载影像，返回处理后的数据或原始流；视频衍生片转码中返回 `NotReady`
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(viewer_id = %token.viewer_id, file_id = %token.file_id, range = ?range)
    )]
    pub async fn download_visual(
        state: &VisualState,
        token: VisualToken,
        range: Option<&str>,
    ) -> Result<ImageDownloadData> {
        // 浏览埋点:仅预览/原图访问计入,缩略图/裁剪不计入
        if token.kind.is_view_event() {
            let db = state.db.clone();
            let token = token.clone();
            tokio::spawn(async move {
                common_db::db_transaction!(&db, |txn| {
                    let Some(visual_id) =
                        VisualMapper::query_visual_id_by_file_id(txn, &token.file_id).await?
                    else {
                        ContextualError::warn_without_source(
                            "nonexist_file_id",
                            "用户尝试访问一个不存在的file_id",
                            (),
                        )
                        .emit();
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

        match token.kind {
            // 图片缩略图: 直接字节; 开启持久化时优先读缓存对象
            VisualTokenKind::Image(op @ ImageTokenType::Thumbnail) => {
                if !state.config.processed_image.store_in_oss {
                    let processed = Self::process_visual_image(state, &token.file_id, &op).await?;
                    return Ok(ImageDownloadData::Buffered {
                        bytes: processed.bytes,
                        content_type: processed.format.to_mime_type(),
                    });
                }

                let object_key = Self::processed_key(state, &token.file_id, &op);
                match state.s3_client.download(&object_key).await {
                    Ok(bytes) => Ok(ImageDownloadData::Buffered {
                        content_type: media::sniff_image_content_type(&bytes),
                        bytes,
                    }),
                    Err(error) if error.is_not_found() => {
                        let processed =
                            Self::process_and_store(state, &token.file_id, &op, &object_key)
                                .await?;
                        Ok(ImageDownloadData::Buffered {
                            bytes: processed.bytes,
                            content_type: processed.format.to_mime_type(),
                        })
                    }
                    Err(error) => Err(ContextualError::from(error).emit()),
                }
            }
            // 图片裁剪: 始终直接处理, 不参与持久化
            VisualTokenKind::Image(op @ ImageTokenType::Crop { .. }) => {
                let processed = Self::process_visual_image(state, &token.file_id, &op).await?;
                Ok(ImageDownloadData::Buffered {
                    bytes: processed.bytes,
                    content_type: processed.format.to_mime_type(),
                })
            }
            // 图片预览: 始终流式; 开启持久化时优先读缓存对象
            VisualTokenKind::Image(op @ ImageTokenType::Preview) => {
                if !state.config.processed_image.store_in_oss {
                    let streamed = media::stream_processed_image(
                        &state.s3_client,
                        state.image_backend,
                        &token.file_id,
                        &op,
                    )
                    .timed(metrics_name!("s3_download_process"))
                    .await?;
                    return Ok(ImageDownloadData::Streamed {
                        stream: streamed.stream,
                        content_type: streamed.content_type,
                        status: 200,
                        content_range: None,
                        content_length: streamed.content_length,
                    });
                }

                let object_key = Self::processed_key(state, &token.file_id, &op);
                match state
                    .s3_client
                    .get_download_stream_range_response(&object_key, None)
                    .timed(metrics_name!("s3_download_stream"))
                    .await
                {
                    Ok(response) => Ok(ImageDownloadData::Streamed {
                        stream: response.stream,
                        content_type: response
                            .content_type
                            .unwrap_or_else(|| "image/webp".to_string()),
                        status: response.status,
                        content_range: None,
                        content_length: response.content_length,
                    }),
                    // 产物尚未生成: 处理并写入对象存储, 本次以单块流返回
                    Err(error) if error.is_not_found() => {
                        let processed =
                            Self::process_and_store(state, &token.file_id, &op, &object_key)
                                .await?;
                        let content_type = processed.format.to_mime_type();
                        let content_length = processed.bytes.len() as u64;
                        Ok(ImageDownloadData::Streamed {
                            stream: media::single_chunk_stream(processed.bytes),
                            content_type: content_type.to_string(),
                            status: 200,
                            content_range: None,
                            content_length: Some(content_length),
                        })
                    }
                    Err(error) => Err(ContextualError::from(error).emit()),
                }
            }
            VisualTokenKind::Image(ImageTokenType::Original) => {
                Self::stream_original(state, &token, range).await
            }
            // 视频缩略片: 直接字节
            VisualTokenKind::Video(VideoTokenType::Thumbnail) => {
                Self::download_video_derivative(state, &token, DerivativeKind::Thumbnail, range)
                    .await
            }
            // 视频预览片: 流式, 支持 Range
            VisualTokenKind::Video(VideoTokenType::Preview) => {
                Self::download_video_derivative(state, &token, DerivativeKind::Preview, range).await
            }
            VisualTokenKind::Video(VideoTokenType::Original) => {
                Self::stream_original(state, &token, range).await
            }
        }
    }

    /// 下载视频衍生片(缩略片直接字节, 预览片流式)
    async fn download_video_derivative(
        state: &VisualState,
        token: &VisualToken,
        kind: DerivativeKind,
        range: Option<&str>,
    ) -> Result<ImageDownloadData> {
        let visual_id = VisualMapper::query_visual_id_by_file_id(&state.db, &token.file_id)
            .await?
            .ok_or_warn(
                "video_file_id_not_found",
                "视频文件不存在",
                AppError::not_found("视频文件不存在"),
            )?;
        let derivative = DerivativeMapper::query_one(&state.db, visual_id, kind).await?;
        let object_key = match derivative {
            Some(record) if record.status == DerivativeStatus::Ready => record.object_key,
            // 已失败: 不再让客户端轮询
            Some(record) if record.status == DerivativeStatus::Failed => {
                return Err(ContextualError::warn_without_source(
                    "video_derivative_failed",
                    "视频衍生片生成失败",
                    AppError::not_found("视频衍生片生成失败"),
                )
                .emit());
            }
            // 仍在转码中
            _ => None,
        };
        let Some(object_key) = object_key else {
            return Ok(ImageDownloadData::NotReady);
        };
        match kind {
            // 视频缩略片较小, 直接返回完整字节
            DerivativeKind::Thumbnail => {
                let bytes = state
                    .s3_client
                    .download(&object_key)
                    .timed(metrics_name!("s3_download"))
                    .await
                    .into_contextual()?;
                Ok(ImageDownloadData::Buffered {
                    bytes,
                    content_type: kind.mime_type(),
                })
            }
            DerivativeKind::Preview => {
                Self::stream_key(state, &object_key, kind.mime_type(), range).await
            }
        }
    }

    /// 处理图片并记录处理耗时
    async fn process_visual_image(
        state: &VisualState,
        file_id: &str,
        op: &ImageTokenType,
    ) -> Result<media::ProcessedImage> {
        media::process_image(&state.s3_client, state.image_backend, file_id, op)
            .timed(metrics_name!("s3_download_process"))
            .await
            .map_err(|error| error.emit())
    }

    /// 处理图片并尽力写入对象存储
    async fn process_and_store(
        state: &VisualState,
        file_id: &str,
        op: &ImageTokenType,
        object_key: &str,
    ) -> Result<media::ProcessedImage> {
        let processed = Self::process_visual_image(state, file_id, op).await?;
        if let Err(error) = state
            .s3_client
            .upload(
                object_key,
                &processed.bytes,
                processed.format.to_mime_type(),
            )
            .timed(metrics_name!("s3_upload_processed"))
            .await
        {
            tracing::warn!(%object_key, %error, "处理后的图片写入对象存储失败");
        }
        Ok(processed)
    }

    /// 处理产物在对象存储中的 key
    fn processed_key(state: &VisualState, file_id: &str, op: &ImageTokenType) -> String {
        let op_key = media::op_key(op, state.image_backend);
        media::processed_object_key(file_id, &op_key)
    }

    /// 流式下载原始影像(原图 / 原视频)
    async fn stream_original(
        state: &VisualState,
        token: &VisualToken,
        range: Option<&str>,
    ) -> Result<ImageDownloadData> {
        let content_type = match token.kind.visual_kind() {
            VisualKind::Image => {
                FileValidator::image_content_type(&token.file_id).unwrap_or("image/jpeg")
            }
            VisualKind::Video => {
                FileValidator::video_content_type(&token.file_id).unwrap_or("video/mp4")
            }
        };
        Self::stream_key(state, &token.file_id, content_type, range).await
    }

    /// 从对象存储流式读取指定 key, 透传单段 Range
    async fn stream_key(
        state: &VisualState,
        object_key: &str,
        fallback_content_type: &'static str,
        range: Option<&str>,
    ) -> Result<ImageDownloadData> {
        let response = state
            .s3_client
            .get_download_stream_range_response(object_key, range)
            .timed(metrics_name!("s3_download_stream"))
            .await
            .into_contextual()?;
        let content_type = response
            .content_type
            .filter(|content_type| content_type != "application/octet-stream")
            .unwrap_or_else(|| fallback_content_type.to_string());
        Ok(ImageDownloadData::Streamed {
            stream: response.stream,
            content_type,
            status: response.status,
            content_range: response.content_range,
            content_length: response.content_length,
        })
    }
}
