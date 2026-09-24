use std::io::Write;
use std::sync::Arc;

use axum::body::Body;
use blake3::Hasher;
use common_core::{
    AppError, ContextualError, Result, TempFile,
    error::contextual::ext::{ContextualResultExt, IntoContextualExt, OptionExt},
    ext::ResultInspectErrAsync,
};
use common_metrics::{MetricsTimerExt, metrics_name, timed};
use file_validator::{FileValidator, MediaKind};
use futures::StreamExt;
use tracing::{info, instrument};
use types::{
    auth::user::{AdminId, UserId},
    visual::{
        UploadVisualParam, VisualView,
        visual::{NewVisualRecord, VisualKind, VisualRecord},
    },
};
use uuid::Uuid;

use crate::{
    VisualRepo, VisualState,
    services::visual_service::{AfterVisualUpload, VisualService, publish_after_visual_upload},
};

impl VisualService {
    /// 接收原始字节流影像: 并发限流、流式落盘并计算 BLAKE3、校验、上传对象存储并落库。
    ///
    /// 请求体即文件字节(不再使用 multipart); 临时文件由守卫保证任何路径下清理:
    /// 成功且启用 `face` 时移交给上传事件供人脸识别消费, 否则在函数结束时删除。
    #[common_macros::metered]
    #[instrument(skip_all, fields(user_id = %user_id))]
    pub async fn upload_from_body(
        state: Arc<VisualState>,
        user_id: UserId,
        req: UploadVisualParam,
        body: Body,
    ) -> Result<VisualView> {
        // 上传并发信号量: 超出并发上限直接拒绝, 避免大文件并发打满内存/磁盘
        let _permit = state
            .upload_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                ContextualError::warn_without_source(
                    "upload_busy",
                    "上传服务繁忙, 请稍后重试",
                    AppError::ServiceUnavailable,
                )
                .emit()
            })?;

        // 流式落盘临时文件: 以本次上传的 uuid 命名(校验后补真实格式后缀,
        // 与最终 S3 key 的文件名一致); 所有错误路径由 Drop 自动清理
        let file_uuid = Uuid::new_v4().to_string();
        let mut temp_file = TempFile::create_in(&state.tmp_dir, &file_uuid).map_err(|error| {
            ContextualError::error(
                "create_temp_file_err",
                "创建临时文件失败",
                error,
                AppError::InternalServerError,
            )
            .emit()
        })?;

        // 流式读取请求体: 首块按魔数嗅探媒体类型(决定写盘上限并拦截非法文件),
        // 边写盘边计算 BLAKE3, 不依赖文件名
        let mut stream = body.into_data_stream();
        let mut hasher = Hasher::new();
        let mut total: u64 = 0;
        let mut write_limit: Option<u64> = None;
        let mut first_chunk = true;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                // 请求体超限(DefaultBodyLimit 流式限制): 返回 413 而非 500
                if common_web::body_util::is_body_limit_error(&error) {
                    return ContextualError::warn_without_source(
                        "upload_file_too_large",
                        "上传文件大小超过服务器限制",
                        AppError::PayloadTooLarge,
                    )
                    .emit();
                }
                ContextualError::error(
                    "read_body_err",
                    "读取请求体失败",
                    error,
                    AppError::InternalServerError,
                )
                .emit()
            })?;

            if first_chunk {
                first_chunk = false;
                let kind = FileValidator::sniff_media(&chunk).ok_or_else(|| {
                    ContextualError::warn_without_source(
                        "unsupported_file_type",
                        "不支持的文件类型",
                        AppError::bad_request("不支持的文件类型"),
                    )
                    .emit()
                })?;
                write_limit = Some(match kind {
                    MediaKind::Image => FileValidator::ALLOW_IMAGE_MAX_SIZE,
                    MediaKind::Video => FileValidator::ALLOW_VIDEO_MAX_SIZE,
                });
            }

            total += chunk.len() as u64;
            if write_limit.is_some_and(|limit| total > limit) {
                return Err(ContextualError::warn_without_source(
                    "upload_file_too_large",
                    "上传文件大小超过服务器限制",
                    AppError::PayloadTooLarge,
                )
                .emit());
            }

            // 边写盘边计算 BLAKE3
            hasher.update(&chunk);
            temp_file.file().write_all(&chunk).map_err(|error| {
                ContextualError::error(
                    "write_temp_file_err",
                    "写入临时文件失败",
                    error,
                    AppError::InternalServerError,
                )
                .emit()
            })?;
        }

        if total == 0 {
            return Err(ContextualError::warn_without_source(
                "upload_body_empty",
                "未找到上传文件",
                AppError::bad_request("未找到上传文件"),
            )
            .emit());
        }
        let hash = hasher.finalize().to_hex().to_string();

        Self::upload_visual(state, user_id, temp_file, file_uuid, hash, req).await
    }

    /// 校验影像, 上传文件并写入影像主记录.
    ///
    /// `temp_file` 为上传流式落盘的临时文件, 由守卫保证任何路径下清理:
    /// 成功且启用 `face` 时移交给上传事件供人脸识别消费, 否则在函数结束时删除。
    #[common_macros::metered]
    #[instrument(
        skip_all,
        fields(user_id = %user_id)
    )]
    pub async fn upload_visual(
        state: Arc<VisualState>,
        user_id: UserId,
        mut temp_file: TempFile,
        file_uuid: String,
        visual_hash: String,
        req: UploadVisualParam,
    ) -> Result<VisualView> {
        // 仅管理员可指定 created_at
        if req.created_at.is_some() {
            AdminId::new(user_id)?;
        }

        // 效验文件（MIME 类型由文件头魔数嗅探确定，不信任客户端声明）
        let metadata = {
            timed!("validate_visual", {
                FileValidator::validate_visual(temp_file.path()).map_err(|error| {
                    ContextualError::warn_without_source(
                        "file_validation_error",
                        "文件校验失败",
                        AppError::bad_request(error.to_string()),
                    )
                })?
            })
        };

        // 按真实格式补扩展名: 后续 face 事件经 image::open(按路径扩展名解码)消费临时文件,
        // 无扩展名会导致其无法识别格式(临时文件名前缀统一, 不依赖客户端文件名)
        temp_file.set_extension(&metadata.format).map_err(|error| {
            ContextualError::error(
                "rename_temp_file_err",
                "重命名临时文件失败",
                error,
                AppError::InternalServerError,
            )
            .emit()
        })?;

        // BLAKE3 去重校验(controller 写盘时已同步计算)
        if VisualRepo::exists_by_hash(&state, &visual_hash).await? {
            return Err(ContextualError::warn_without_source(
                "upload_visual:img_exist",
                "影像已存在",
                AppError::bad_request("影像已存在"),
            )
            .emit());
        }

        // 流式上传文件到对象存储(小文件直传, 大文件自动分片);
        // 文件名为与临时文件一致的 uuid + 真实格式后缀
        let file_id = Self::get_visual_s3_key(&file_uuid, &metadata);
        state
            .s3_client
            .upload_file(&file_id, temp_file.path(), &metadata.mime_type)
            .timed(metrics_name!("s3_upload"))
            .await
            .into_contextual()?;

        // 更新数据库
        let visual = if metadata.is_video() {
            NewVisualRecord {
                user_id,
                name: file_uuid.clone(),
                size: metadata.size,
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
                created_at: req.created_at,
            }
        } else {
            NewVisualRecord {
                user_id,
                name: file_uuid.clone(),
                size: metadata.size,
                width: metadata.width,
                height: metadata.height,
                kind: VisualKind::Image,
                duration_ms: 0,
                hash: visual_hash.clone(),
                file_id: file_id.clone(),
                created_at: req.created_at,
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

        info!("影像上传成功: {}: {}", visual.id, visual.name);

        // 发布事件
        let visual_record = VisualRecord::from(visual);
        publish_after_visual_upload(
            Arc::clone(&state),
            AfterVisualUpload {
                visual: visual_record.clone(),
                #[cfg(feature = "face")]
                temp_file,
            },
        );

        Ok(VisualView::from_record_with_tokens(visual_record, user_id)?)
    }
}
