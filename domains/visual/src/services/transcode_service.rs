//! 视频衍生片转码服务.
//!
//! 视频上传后, 由事件消费者登记待生成的衍生片并投入队列, 常驻单 worker 串行消费:
//! 从对象存储下载原片, 调用 ffmpeg 生成缩略片(前 N 秒)与预览片(整片压缩),
//! 上传为独立对象, 并更新生成状态。

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use common_core::error::contextual::Result;
use common_core::error::contextual::ext::{IntoContextualExt, OptionExt, ResultContextualExt};
use common_core::{AppError, ContextualError, TempFile};
use futures::StreamExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{info, warn};
use types_visual::derivative::{
    DerivativeKind, DerivativeStatus, VisualDerivativeId, derivative_key,
};
use types_visual::visual::{VisualId, VisualKind, VisualRecord};
use uuid::Uuid;

use crate::config::TranscodeConfig;
use crate::mappers::{derivative_mapper::DerivativeMapper, visual_mapper::VisualMapper};
use crate::services::visual_service::{AfterVisualDelete, AfterVisualUpload};
use crate::state::VisualState;

/// 单次 ffmpeg 执行超时
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(600);

/// 待转码任务
struct TranscodeJob {
    id: VisualDerivativeId,
    visual_id: VisualId,
    kind: DerivativeKind,
    file_id: String,
}

/// 转码队列: 消费者仅入队, 由常驻单 worker 串行消费, 避免上传请求阻塞在转码上
static TRANSCODE_QUEUE: OnceLock<mpsc::UnboundedSender<TranscodeJob>> = OnceLock::new();

/// 视频衍生片转码服务
pub struct TranscodeService;

// 视频上传后登记并入队
#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualUpload,
    slice = crate::services::visual_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "video_transcode_enqueue",
)]
impl TranscodeService {
    /// 仅处理视频; 登记缩略/预览两条衍生记录并入队。
    #[tracing::instrument(name = "transcode_enqueue", skip_all)]
    async fn on_after_visual_upload(
        &self,
        state: Arc<VisualState>,
        event: Arc<AfterVisualUpload>,
    ) -> common_core::Result<()> {
        Self::enqueue(&state, &event.visual).await
    }
}

impl TranscodeService {
    /// 登记待生成的衍生片并入队(非视频 / 未启用时跳过)。
    async fn enqueue(state: &VisualState, visual: &VisualRecord) -> common_core::Result<()> {
        if visual.kind != VisualKind::Video || !state.config.transcode.enabled {
            return Ok(());
        }
        let kinds = [DerivativeKind::Thumbnail, DerivativeKind::Preview];
        DerivativeMapper::insert_pending(&state.db, visual.id, &kinds).await?;
        let records = DerivativeMapper::query_by_visual_id(&state.db, visual.id).await?;
        let tx = TRANSCODE_QUEUE.get().ok_or_error(
            "transcode_queue_not_ready",
            "转码队列未初始化",
            AppError::InternalServerError,
        )?;
        for record in records {
            // 已就绪的无需重做(幂等)
            if record.status == DerivativeStatus::Ready {
                continue;
            }
            tx.send(TranscodeJob {
                id: record.id,
                visual_id: visual.id,
                kind: record.kind,
                file_id: visual.file_id.clone(),
            })
            .context_err(
                "transcode_queue_closed",
                "转码队列已关闭",
                AppError::InternalServerError,
            )?;
        }
        Ok(())
    }

    /// 启动常驻转码 worker, 由 [`crate::VisualState::new`] 调用; 重复调用直接 panic。
    pub(crate) fn start_worker(state: Arc<VisualState>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        TRANSCODE_QUEUE
            .set(tx)
            .unwrap_or_else(|_| panic!("TRANSCODE_QUEUE 重复初始化"));
        let task_manager = state.task_manager.clone();
        task_manager.spawn("video_transcode_worker", async move {
            Self::recover_pending(&state).await;
            while let Some(job) = rx.recv().await {
                if let Err(error) = Self::process_job(&state, job).await {
                    ContextualError::warn(
                        "transcode_job_failed",
                        "视频转码任务失败",
                        error,
                        AppError::InternalServerError,
                    )
                    .emit();
                }
            }
        });
    }

    /// 启动恢复: 将未完成的记录重新入队(卡在 running 的先重置为 pending)。
    async fn recover_pending(state: &VisualState) {
        let records = match DerivativeMapper::query_recoverable(&state.db).await {
            Ok(records) => records,
            Err(error) => {
                ContextualError::warn(
                    "transcode_recover_query_failed",
                    "查询待恢复转码任务失败",
                    error,
                    AppError::InternalServerError,
                )
                .emit();
                return;
            }
        };
        if records.is_empty() {
            return;
        }
        info!("恢复 {} 个未完成的视频转码任务", records.len());

        let mut by_visual: HashMap<VisualId, Vec<_>> = HashMap::new();
        for record in records {
            by_visual.entry(record.visual_id).or_default().push(record);
        }
        for (visual_id, records) in by_visual {
            let file_id = match VisualMapper::query_file_id_by_id(&state.db, visual_id).await {
                Ok(file_id) => file_id,
                Err(error) => {
                    warn!(visual_id = %visual_id, error = ?error, "恢复转码任务时查询 file_id 失败, 跳过");
                    continue;
                }
            };
            let Some(tx) = TRANSCODE_QUEUE.get() else {
                return;
            };
            for record in records {
                if record.status == DerivativeStatus::Running
                    && let Err(error) = DerivativeMapper::mark_pending(&state.db, record.id).await
                {
                    warn!(derivative_id = %record.id, error = ?error, "重置转码任务状态失败");
                }
                let _ = tx.send(TranscodeJob {
                    id: record.id,
                    visual_id,
                    kind: record.kind,
                    file_id: file_id.clone(),
                });
            }
        }
    }

    /// 生成单个衍生片: 下载原片 -> ffmpeg -> 上传 -> 更新状态。
    async fn process_job(state: &Arc<VisualState>, job: TranscodeJob) -> Result<()> {
        DerivativeMapper::mark_running(&state.db, job.id).await?;
        match Self::generate(state, &job).await {
            Ok(object_key) => {
                DerivativeMapper::mark_ready(&state.db, job.id, &object_key).await?;
                info!(visual_id = %job.visual_id, kind = ?job.kind, "视频衍生片生成完成");
                Ok(())
            }
            Err(error) => {
                if let Err(mark_error) =
                    DerivativeMapper::mark_failed(&state.db, job.id, &error.to_string()).await
                {
                    warn!(derivative_id = %job.id, error = ?mark_error, "记录转码失败状态时出错");
                }
                Err(error)
            }
        }
    }

    /// 下载原片、调用 ffmpeg 生成衍生片并上传, 返回衍生对象 key。
    async fn generate(state: &VisualState, job: &TranscodeJob) -> Result<String> {
        let input = Self::download_to_temp(state, &job.file_id).await?;

        let mut output = TempFile::create_in(&state.tmp_dir, &Uuid::new_v4().to_string())
            .context_err(
                "transcode_output_temp_err",
                "创建转码输出临时文件失败",
                AppError::InternalServerError,
            )?;
        output.set_extension("mp4").context_err(
            "transcode_output_ext_err",
            "设置转码输出扩展名失败",
            AppError::InternalServerError,
        )?;

        Self::run_ffmpeg(
            &state.config.transcode,
            job.kind,
            input.path(),
            output.path(),
        )
        .await?;

        let object_key = derivative_key(&job.file_id, job.kind);
        state
            .s3_client
            .upload_file(&object_key, output.path(), job.kind.mime_type())
            .await
            .into_contextual()?;
        Ok(object_key)
    }

    /// 将原片从对象存储流式下载到临时文件(按原扩展名命名, 便于 ffmpeg 探测)。
    async fn download_to_temp(state: &VisualState, file_id: &str) -> Result<TempFile> {
        let mut stream = state
            .s3_client
            .get_download_stream_response(file_id)
            .await
            .into_contextual()?;

        let mut temp = TempFile::create_in(&state.tmp_dir, &Uuid::new_v4().to_string())
            .context_err(
                "transcode_input_temp_err",
                "创建转码输入临时文件失败",
                AppError::InternalServerError,
            )?;

        let ext = file_id
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("mp4");
        temp.set_extension(ext).context_err(
            "transcode_input_ext_err",
            "设置转码输入扩展名失败",
            AppError::InternalServerError,
        )?;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.into_contextual()?;
            temp.file().write_all(&chunk).context_err(
                "transcode_write_temp_err",
                "写入转码输入临时文件失败",
                AppError::InternalServerError,
            )?;
        }
        Ok(temp)
    }

    /// 调用 ffmpeg 执行转码, 失败/超时返回错误。
    async fn run_ffmpeg(
        cfg: &TranscodeConfig,
        kind: DerivativeKind,
        input: &Path,
        output: &Path,
    ) -> Result<()> {
        let mut cmd = Command::new(&cfg.ffmpeg_path);
        cmd.args(Self::build_args(cfg, kind, input, output));

        let output = match timeout(FFMPEG_TIMEOUT, cmd.output()).await {
            Err(_elapsed) => {
                return Err(ContextualError::warn_without_source(
                    "ffmpeg_timeout",
                    "视频转码超时",
                    AppError::InternalServerError,
                ));
            }
            Ok(result) => result.context_err(
                "ffmpeg_spawn_err",
                "启动 ffmpeg 失败",
                AppError::InternalServerError,
            )?,
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ContextualError::warn_without_source(
                "ffmpeg_failed",
                format!("视频转码失败: {}", stderr.trim()),
                AppError::InternalServerError,
            ));
        }
        Ok(())
    }

    /// 构造 ffmpeg 参数(纯函数, 便于单测)。
    ///
    /// 统一: H.264 high / yuv420p / faststart; 仅缩不放。
    /// 缩略片: 前 N 秒、无音轨、宽 `min(w, iw)`。
    /// 预览片: 整片、AAC 立体声、高 `min(h, ih)`。
    fn build_args(
        cfg: &TranscodeConfig,
        kind: DerivativeKind,
        input: &Path,
        output: &Path,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec!["-y".to_string()];

        match kind {
            DerivativeKind::Thumbnail => {
                args.extend([
                    "-ss".to_string(),
                    "0".to_string(),
                    "-t".to_string(),
                    cfg.thumb_duration_secs.to_string(),
                ]);
            }
            DerivativeKind::Preview => {}
        }

        args.extend(["-i".to_string(), input.display().to_string()]);

        match kind {
            DerivativeKind::Thumbnail => {
                args.extend([
                    "-an".to_string(),
                    "-vf".to_string(),
                    // 逗号在 ffmpeg 过滤器表达式内需转义
                    format!("scale=min({}\\,iw):-2", cfg.thumb_max_width),
                    "-b:v".to_string(),
                    cfg.thumb_bitrate.clone(),
                    "-level".to_string(),
                    "3.1".to_string(),
                ]);
            }
            DerivativeKind::Preview => {
                args.extend([
                    "-vf".to_string(),
                    format!("scale=-2:min({}\\,ih)", cfg.preview_max_height),
                    "-b:v".to_string(),
                    cfg.preview_bitrate.clone(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "128k".to_string(),
                    "-ac".to_string(),
                    "2".to_string(),
                    "-level".to_string(),
                    "4.2".to_string(),
                ]);
            }
        }

        args.extend([
            "-c:v".to_string(),
            "libx264".to_string(),
            "-profile:v".to_string(),
            "high".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-r".to_string(),
            "30".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            output.display().to_string(),
        ]);

        args
    }
}

// 视频衍生片: 随影像删除清理 DB 记录(事务步骤)与对象(删除后消费者)
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "derivative_cleanup",
    owns = ["DerivativeMapper"],
    method = on_visual_delete,
)]
impl TranscodeService {
    /// 删除影像事务中, 清理其衍生片 DB 记录。
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common_core::error::contextual::Result<()> {
        DerivativeMapper::delete_by_visual_ids(txn, &ctx.visual_ids()).await?;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualDelete,
    slice = crate::services::visual_service::AFTER_MEDIA_DELETE_CONSUMERS,
    name = "derivative_delete_cleanup",
)]
impl TranscodeService {
    /// 删除影像后, 清理其视频衍生片对象(键由 file_id 推导, 删除幂等)。
    async fn on_after_visual_delete(
        &self,
        state: Arc<VisualState>,
        event: Arc<AfterVisualDelete>,
    ) -> common_core::Result<()> {
        let keys = event
            .visuals
            .iter()
            .filter(|visual| visual.kind == VisualKind::Video)
            .flat_map(|visual| {
                [
                    derivative_key(&visual.file_id, DerivativeKind::Thumbnail),
                    derivative_key(&visual.file_id, DerivativeKind::Preview),
                ]
            })
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return Ok(());
        }
        state.s3_client.delete_batch(keys).await.into_contextual()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumb_args_take_first_seconds_without_audio() {
        let cfg = TranscodeConfig::default();
        let args = TranscodeService::build_args(
            &cfg,
            DerivativeKind::Thumbnail,
            Path::new("/tmp/in.mp4"),
            Path::new("/tmp/out.mp4"),
        );
        let joined = args.join(" ");
        assert!(joined.contains("-ss 0 -t 4"));
        assert!(joined.contains("-an"));
        assert!(joined.contains("scale=min(640\\,iw):-2"));
        assert!(joined.contains("-pix_fmt yuv420p"));
        assert!(joined.contains("-movflags +faststart"));
    }

    #[test]
    fn preview_args_keep_full_length_with_audio() {
        let cfg = TranscodeConfig::default();
        let args = TranscodeService::build_args(
            &cfg,
            DerivativeKind::Preview,
            Path::new("/tmp/in.mp4"),
            Path::new("/tmp/out.mp4"),
        );
        let joined = args.join(" ");
        assert!(!joined.contains("-ss"));
        assert!(!joined.contains("-t "));
        assert!(joined.contains("scale=-2:min(1080\\,ih)"));
        assert!(joined.contains("-c:a aac"));
        assert!(joined.contains("-b:a 128k"));
    }
}
