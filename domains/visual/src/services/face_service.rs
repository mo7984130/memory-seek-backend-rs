use audit::{AuditEvent, AuditRecorder};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use common_core::{
    Result,
    error::contextual::ext::{IntoContextualExt, OptionExt, ResultContextualExt},
    error::{AppError, ContextualError, contextual},
    ext::ToOk,
    types::CursorPage,
};
use common_metrics::{
    GaugeGuard, MetricsTimer, MetricsTimerExt, inc_counter, metrics_name, set_gauge,
};
use image::{ImageBuffer, Rgb};
use insight_face_rs::Face;
use tokio::{spawn, sync::mpsc, task::spawn_blocking};
use tracing::{debug, info, warn};
use types_core::cursor::TimeIdCursor;
use types_identity::auth::user::{AdminId, UserId};
use types_visual::{
    FaceView, dto::face::FaceDeleteBatchResult, dto::face::UnassignedFaceVisualCursorParam,
    dto::visual::VisualView, face, face::FaceId, face::FaceRecord, face_task::FaceTaskStatus,
    models::FaceIds, person::PersonId, visual::VisualId, visual::VisualKind,
};

use crate::{
    VisualState,
    mappers::{
        face_mapper::FaceMapper, face_task_mapper::FaceTaskMapper, person_mapper::PersonMapper,
        visual_mapper::VisualMapper,
    },
    repo::FaceRepo,
    services::visual_service::{AfterVisualUpload, VisualService},
};

pub(crate) struct FaceService;

type Img = ImageBuffer<Rgb<u8>, Vec<u8>>;

/// 人脸检测取图的最大边长(限制尺寸, 减少解码与检测开销)
const DETECT_IMAGE_MAX_EDGE: u32 = 1920;

// 创建
impl FaceService {
    /// 人脸计算.
    /// 全量时, 备份并且清空表, 从头开始
    /// 增量时, 备份表, 靠是否有visual无face来判断
    pub async fn compute(state: Arc<VisualState>, admin: AdminId, full: bool) -> Result<()> {
        let user_id = admin.into_inner();
        let admin = AdminId::new(user_id)?;
        common_db::db_transaction!(scoped & state.db, |txn| {
            AuditRecorder::append(
                txn,
                AuditEvent::new("face_compute")
                    .with_actor(user_id.0)
                    .with_detail(serde_json::json!({ "full": full })),
            )
            .await?;
            Ok(())
        })
        .await?;
        spawn(async move { Self::compute_inner(state, admin, full).await });
        Ok(())
    }

    /// 人脸计算.
    #[common_macros::metered(name = "face_compute")]
    #[tracing::instrument(
        name = "face_compute",
        skip_all,
        fields(user_id = %admin, full = %full)
    )]
    async fn compute_inner(state: Arc<VisualState>, admin: AdminId, full: bool) -> Result<()> {
        let user_id = admin.into_inner();
        info!(user_id = %user_id, "人脸计算触发, full: {}", full);

        let _running_guard = GaugeGuard::start(metrics_name!("running"));
        set_gauge!("mode", 1.0, "mode" => if full { "full" } else { "incremental" });

        // 如果是全量计算的话
        // 备份并且清空表
        if full {
            FaceRepo::backup_and_truncate(&state).await?;
        }

        let batch_size = 128;
        let mut previous_id = VisualId(0);
        let mut batch_idx = 0i64;
        let mut total_visuals = 0u64;
        let mut total_faces = 0u64;
        let mut total_no_face = 0u64;
        loop {
            batch_idx += 1;
            set_gauge!("batch", batch_idx as f64);

            let visuals =
                FaceRepo::query_face_compute_visuals(&state, full, batch_size, previous_id)
                    .timed(metrics_name!("query"))
                    .await?;
            if visuals.is_empty() {
                info!("第{}批DB查询结果为空, 计算结束", batch_idx);
                break;
            }
            // 刷新previous_id
            if let Some(last) = visuals.last() {
                previous_id = last.0;
            }
            let visual_count = visuals.len();

            let mut new_faces: Vec<face::NewFaceRecord> = Vec::with_capacity(visual_count * 4);
            let _download_batch_timer = MetricsTimer::start(metrics_name!("download_batch"));
            for (visual_id, file_id) in visuals {
                debug!("照片流程开始: visual_id: {visual_id}");
                let _ = async {
                    let img = Self::download_visual(&state, &file_id)
                        .timed(metrics_name!("visual_download"))
                        .await?;
                    let faces = Self::detect_visual(&state, img)
                        .timed(metrics_name!("visual_detect"))
                        .await?;
                    let face_count = faces.len();
                    new_faces.extend(
                        faces
                            .into_iter()
                            .map(|face| face::NewFaceRecord::from_detected(visual_id, face)),
                    );
                    Ok::<usize, AppError>(face_count)
                }
                .await
                .inspect(|&face_count| {
                    inc_counter!("visuals_processed", 1);
                    inc_counter!("faces_detected", face_count as u64);
                    total_visuals += 1;
                    total_faces += face_count as u64;
                    if face_count == 0 {
                        inc_counter!("no_face_visuals", 1);
                        total_no_face += 1;
                    }
                })
                .inspect_err(|_| {
                    tracing::warn!(visual_id = %visual_id, %file_id, "照片流程错误, 跳过");
                });
            }
            drop(_download_batch_timer);

            Self::insert_faces(&state, new_faces)
                .timed(metrics_name!("insert"))
                .await?;

            info!(
                "第{}批插入完成, 现共{}",
                batch_idx,
                batch_size * batch_idx as u64
            );
        }

        set_gauge!("total_visuals", total_visuals as f64);
        set_gauge!("total_faces", total_faces as f64);
        set_gauge!("total_no_face", total_no_face as f64);

        Ok(())
    }

    /// 从对象存储取图并解码为图像缓冲区(按 `image_processor` 选择处理后端).
    async fn download_visual(state: &VisualState, file_id: &String) -> Result<Img> {
        debug!("下载照片{}", file_id);
        let image = crate::media::fetch_for_detection(
            &state.s3_client,
            state.image_backend,
            file_id,
            DETECT_IMAGE_MAX_EDGE,
            DETECT_IMAGE_MAX_EDGE,
        )
        .await?;

        // 转 RGB 的 CPU 工作放到阻塞线程, 并保留 visual_decode 步骤指标
        let img = spawn_blocking(move || image.into_rgb8())
            .timed(metrics_name!("visual_decode"))
            .await
            .into_contextual()?;

        debug!("下载完成");
        Ok(img)
    }

    /// 在阻塞线程中直接对落盘图片文件执行人脸检测, 避免占用异步执行器.
    ///
    /// 引擎内部(`run_from_file`)负责解码, 调用方无需持有解码缓冲。
    async fn detect_visual_from_file(state: &VisualState, path: &Path) -> Result<Vec<Face>> {
        let face_engine_clone = Arc::clone(&state.face_engine);
        let path = path.to_owned();
        let detect_result = spawn_blocking(move || -> contextual::Result<Vec<Face>> {
            let result = face_engine_clone.run_from_file(&path);
            if let Err(ref error) = result {
                // 诊断: 打印文件状态(大小/头部字节), 便于定位 image::open 失败原因
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let head = std::fs::read(&path)
                    .map(|b| b[..b.len().min(16)].to_vec())
                    .unwrap_or_default();
                warn!(
                    path = %path.display(),
                    size,
                    head = ?head,
                    error = %error,
                    "run_from_file 解码失败诊断"
                );
            }
            result.context_err(
                "face-engine_run_error",
                "人脸检测模型运行失败",
                AppError::InternalServerError,
            )
        })
        .await
        .into_contextual()?;
        Ok(detect_result?)
    }

    /// 在阻塞线程中执行人脸检测并返回检测结果.
    async fn detect_visual(state: &VisualState, img: Img) -> Result<Vec<Face>> {
        debug!("检测照片中");
        let face_engine_clone = Arc::clone(&state.face_engine);
        let detect_result = spawn_blocking(move || -> contextual::Result<Vec<Face>> {
            let faces = face_engine_clone.run(&img).context_err(
                "face-engine_run_error",
                "人脸检测模型运行失败",
                AppError::InternalServerError,
            )?;
            debug!("转换成功");
            Ok(faces)
        })
        .await
        .into_contextual()?;
        Ok(detect_result?)
    }

    /// 批量写入检测到的人脸记录.
    async fn insert_faces(state: &VisualState, faces: Vec<face::NewFaceRecord>) -> Result<()> {
        debug!("插入人脸到数据库中");
        if faces.is_empty() {
            debug!("faces为空, 跳过");
        } else {
            FaceRepo::insert_faces(state, faces)
                .await
                .into_contextual()?;
        }
        debug!("插入完成");
        Ok(())
    }
}

// 修改
impl FaceService {
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(face_id = %face_id, person_id = ?person_id)
    )]
    /// 修改人脸归属
    pub async fn change_face_belonging(
        state: &VisualState,
        face_id: FaceId,
        person_id: Option<PersonId>,
        user_id: UserId,
    ) -> Result<()> {
        // 事务耗时由 FaceRepo 内的 `.timed(metrics_name!("db_transaction"))` 记录
        // （其当前 span 即本函数，指标为 visual:change_face_belonging:db_transaction），此处不再重复计时。
        FaceRepo::change_face_belonging(state, face_id, person_id, user_id).await?;
        Ok(())
    }
}

// 查询
impl FaceService {
    /// 查询指定照片的人脸.
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(visual_id = %visual_id))]
    pub async fn get_faces_by_visual_id(
        state: &VisualState,
        visual_id: VisualId,
    ) -> Result<Vec<FaceView>> {
        let (faces, person_names) =
            FaceRepo::query_faces_with_person_names(state, visual_id).await?;

        let views = faces
            .into_iter()
            .map(|face| FaceView {
                id: face.id,
                bbox: face.bbox.into(),
                person_id: face.person_id,
                person_name: face.person_id.and_then(|id| person_names.get(&id).cloned()),
            })
            .collect::<Vec<FaceView>>();

        views.to_ok()
    }

    /// 游标获取"包含未分配人脸"的照片列表
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    /// 分页查询包含未分配人脸的照片.
    pub async fn get_unassigned_face_visuals(
        state: &VisualState,
        user_id: UserId,
        req: UnassignedFaceVisualCursorParam,
    ) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
        let page = FaceRepo::query_unassigned_face_visual_ids(state, &req)
            .timed(metrics_name!("query_unassigned_face_visual_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        let visuals = VisualService::load_visuals_info(state, user_id, &page.records)
            .timed(metrics_name!("load_visuals_info"))
            .await?;

        page.replace_records(visuals)
            .with_next_cursor(|visual| TimeIdCursor {
                time_at: visual.created_at,
                id: visual.id,
            })
            .to_ok()
    }
}

// 删除
impl FaceService {
    /// 删除单张人脸
    /// 仅可以删除无归属的人脸
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(face_id = %face_id))]
    pub async fn delete_face(state: &VisualState, face_id: FaceId, user_id: UserId) -> Result<()> {
        FaceRepo::delete_faces(state, vec![face_id], user_id).await?;

        Ok(())
    }

    /// 批量删除人脸
    /// 仅可以删除无归属的人脸
    #[common_macros::metered(name = "delete_faces_batch")]
    #[tracing::instrument(name = "delete_faces_batch", skip_all, fields(count = %face_ids.len()))]
    pub async fn delete_faces(
        state: &VisualState,
        face_ids: FaceIds,
        user_id: UserId,
    ) -> Result<FaceDeleteBatchResult> {
        let deleted_face_count =
            FaceRepo::delete_faces(state, face_ids.into_inner(), user_id).await?;

        Ok(FaceDeleteBatchResult { deleted_face_count })
    }
}

/// 当照片上传后
/// 计算人脸
#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualUpload,
    slice = crate::services::visual_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "face_recognition",
)]
impl FaceService {
    /// 消费者仅将事件投入队列, 由单 worker 串行处理;
    /// 检测延迟从上传响应转移到队列积压。
    #[tracing::instrument(name = "face_enqueue", skip_all)]
    async fn on_after_visual_upload(
        &self,
        state: Arc<VisualState>,
        event: Arc<AfterVisualUpload>,
    ) -> common_core::Result<()> {
        if event.visual.kind != VisualKind::Image {
            return Ok(());
        }
        let visual_id = event.visual.id;
        // 先登记持久化任务(outbox), 再入队: 进程崩溃后由启动恢复兜底
        FaceTaskMapper::insert_pending(&state.db, visual_id)
            .timed(metrics_name!("db_insert"))
            .await?;
        FACE_QUEUE
            .get()
            .ok_or_error(
                "face_queue_not_ready",
                "人脸检测队列未初始化",
                AppError::InternalServerError,
            )?
            .send(FaceJob {
                visual_id,
                source: FaceJobSource::Uploaded(event),
            })
            .context_err(
                "face_queue_closed",
                "人脸检测队列已关闭",
                AppError::InternalServerError,
            )?;
        Ok(())
    }
}

/// 单次人脸检测的最大尝试次数(含首次); 超过后置为终态 failed
const FACE_MAX_ATTEMPTS: u32 = 3;

/// 检测失败后的重试退避(秒)
const FACE_RETRY_BACKOFF_SECS: u64 = 30;

/// 待检测任务。
#[derive(Clone)]
struct FaceJob {
    visual_id: VisualId,
    source: FaceJobSource,
}

/// 任务取图来源。
#[derive(Clone)]
enum FaceJobSource {
    /// 热路径: 上传落盘的临时文件(随事件 Arc 保活, 消费后由守卫删除)
    Uploaded(Arc<AfterVisualUpload>),
    /// 恢复路径: 按 file_id 从对象存储重新下载
    Stored { file_id: String },
}

/// 上传后待做人脸检测的任务队列: 消费者仅入队, 由常驻单 worker 串行消费,
/// 避免大量上传时无限 spawn 任务造成内存压力。
///
/// 队列仅作进程内唤醒; 持久化状态见 `visual_face_task`, 崩溃后由启动恢复重新入队。
static FACE_QUEUE: OnceLock<mpsc::UnboundedSender<FaceJob>> = OnceLock::new();

/// 是否应重试: 已尝试次数(含本次)未达上限。
fn should_retry(attempts: i32, max_attempts: u32) -> bool {
    attempts > 0 && (attempts as u32) < max_attempts
}

// 常驻 worker: 消费任务队列, 串行执行人脸检测
impl FaceService {
    /// 启动单线程人脸检测 worker, 由 [`crate::VisualState::new`] 调用;
    /// 重复调用直接 panic。
    pub(crate) fn start_face_consumer(state: Arc<VisualState>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        FACE_QUEUE
            .set(tx)
            .unwrap_or_else(|_| panic!("FACE_QUEUE 重复初始化"));
        let task_manager = state.task_manager.clone();
        task_manager.spawn("face_consumer_worker", move |token| async move {
            Self::recover_pending(&state).await;
            loop {
                tokio::select! {
                    // 取消优先: 收到关闭信号后不再领取新任务
                    biased;
                    _ = token.cancelled() => {
                        info!("人脸检测 worker 收到关闭信号, 完成当前任务后退出");
                        break;
                    }
                    job = rx.recv() => {
                        let Some(job) = job else { break };
                        // 执行阶段不参与取消竞速: 当前任务一定跑完
                        if let Err(error) = Self::process_job(&state, job).await {
                            ContextualError::warn(
                                "face_consume_failed",
                                "人脸检测消费任务失败",
                                error,
                                AppError::InternalServerError,
                            )
                            .emit();
                        }
                    }
                }
            }
        });
    }

    /// 启动恢复: 将未完成的任务重新入队(卡在 running 的先重置为 pending),
    /// 恢复路径改从对象存储下载原图。任务随影像删除已连带清理, 不会出现孤儿。
    async fn recover_pending(state: &Arc<VisualState>) {
        let records = match FaceTaskMapper::query_recoverable(&state.db).await {
            Ok(records) => records,
            Err(error) => {
                ContextualError::warn(
                    "face_recover_query_failed",
                    "查询待恢复人脸检测任务失败",
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
        info!("恢复 {} 个未完成的人脸检测任务", records.len());

        for record in records {
            if record.status == FaceTaskStatus::Running
                && let Err(error) = FaceTaskMapper::mark_pending(&state.db, record.visual_id).await
            {
                warn!(visual_id = %record.visual_id, error = ?error, "重置人脸检测任务状态失败");
            }
            let file_id = match VisualMapper::query_file_id_by_id(&state.db, record.visual_id).await
            {
                Ok(file_id) => file_id,
                Err(error) => {
                    warn!(visual_id = %record.visual_id, error = ?error, "恢复人脸检测任务时查询 file_id 失败, 跳过");
                    continue;
                }
            };
            let Some(tx) = FACE_QUEUE.get() else {
                return;
            };
            let _ = tx.send(FaceJob {
                visual_id: record.visual_id,
                source: FaceJobSource::Stored { file_id },
            });
        }
    }

    /// 处理单个任务: 标记进行中 -> 检测 -> 原子落库(人脸 + 完成任务)。
    ///
    /// 检测期间影像被删时任务记录会连带删除, `mark_running` / `mark_done` 均不命中,
    /// 因此不会留下孤儿人脸; 已完成任务被重复入队也会直接跳过。
    #[tracing::instrument(name = "face_compute", skip_all)]
    async fn process_job(state: &Arc<VisualState>, job: FaceJob) -> common_core::Result<()> {
        let visual_id = job.visual_id;
        // 仅消费待检测任务(已完成/已删除则跳过, 保证重复入队幂等)
        if !FaceTaskMapper::mark_running(&state.db, visual_id).await? {
            return Ok(());
        }

        match Self::detect_for_job(state, &job).await {
            Ok(faces) => {
                let faces = faces
                    .into_iter()
                    .map(|face| face::NewFaceRecord::from_detected(visual_id, face))
                    .collect();
                Self::save_detection(state, visual_id, faces)
                    .timed(metrics_name!("insert"))
                    .await?;
                info!("照片 id: {} 人脸检测完成", visual_id);
                Ok(())
            }
            Err(error) => {
                let retry = match FaceTaskMapper::bump_attempt(&state.db, visual_id).await? {
                    Some(attempts) => should_retry(attempts, FACE_MAX_ATTEMPTS),
                    None => false,
                };
                if retry {
                    // 回退为待检测并退避重入队(不阻塞 worker)
                    FaceTaskMapper::mark_pending(&state.db, visual_id).await?;
                    warn!(
                        visual_id = %visual_id,
                        backoff_secs = FACE_RETRY_BACKOFF_SECS,
                        error = %error,
                        "人脸检测失败, 稍后重试"
                    );
                    Self::schedule_retry(state, job, FACE_RETRY_BACKOFF_SECS);
                } else {
                    let _ =
                        FaceTaskMapper::mark_failed(&state.db, visual_id, &error.to_string()).await;
                    warn!(
                        visual_id = %visual_id,
                        error = %error,
                        "人脸检测失败, 已达最大尝试次数"
                    );
                }
                Err(error)
            }
        }
    }

    /// 依任务来源取图并检测: 热路径直接读临时文件, 恢复路径从对象存储下载后检测。
    async fn detect_for_job(
        state: &Arc<VisualState>,
        job: &FaceJob,
    ) -> common_core::Result<Vec<Face>> {
        match &job.source {
            FaceJobSource::Uploaded(event) => {
                Self::detect_visual_from_file(state, event.temp_file.path())
                    .timed(metrics_name!("visual_detect"))
                    .await
            }
            FaceJobSource::Stored { file_id } => {
                let img = Self::download_visual(state, file_id).await?;
                Self::detect_visual(state, img)
                    .timed(metrics_name!("visual_detect"))
                    .await
            }
        }
    }

    /// 原子落库: 命中检测中任务则插入人脸并置为完成, 二者同事务。
    async fn save_detection(
        state: &Arc<VisualState>,
        visual_id: VisualId,
        faces: Vec<face::NewFaceRecord>,
    ) -> common_core::Result<()> {
        common_db::db_transaction!(contextual & state.db, |txn| {
            // 任务在检测期间被删(影像被删)则不落库
            if !FaceTaskMapper::mark_done(txn, visual_id).await? {
                return Ok(());
            }
            if !faces.is_empty() {
                FaceMapper::inserts(txn, faces).await?;
            }
            Ok(())
        })
        .await?;
        Ok(())
    }

    /// 退避后重新入队(不阻塞当前 worker)。
    fn schedule_retry(state: &Arc<VisualState>, job: FaceJob, backoff_secs: u64) {
        let task_manager = state.task_manager.clone();
        task_manager.spawn("face_retry", move |token| async move {
            // 退避期间收到关闭信号直接放弃重试, 避免拖住关闭
            tokio::select! {
                biased;
                _ = token.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
            }
            if let Some(tx) = FACE_QUEUE.get() {
                let _ = tx.send(job);
            }
        });
    }
}

// 当照片删除时
// 删除人脸 和 对应的人物
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "face_cleanup",
    owns = ["FaceMapper", "PersonMapper", "FaceTaskMapper"],
    method = on_visual_delete,
)]
impl FaceService {
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common_core::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();

        // 连带清理待检测任务(outbox), 避免影像删除后任务被启动恢复重新入队
        FaceTaskMapper::delete_by_visual_ids(txn, &visual_ids).await?;

        // 加行锁读取待删照片的全部人脸, 阻止并发转移归属读到将删人脸
        let faces = FaceMapper::lock_by_visual_ids(txn, &visual_ids).await?;
        if faces.is_empty() {
            return Ok(());
        }

        // 按归属人物分组(未归属人脸不涉及人物统计, 直接删除)
        let mut by_person: HashMap<PersonId, Vec<FaceRecord>> = HashMap::new();
        for face in faces {
            if let Some(person_id) = face.person_id {
                by_person.entry(person_id).or_default().push(face);
            }
        }

        // 删除照片的全部人脸记录(先删后维护, 封面回退查询到的即为剩余人脸)
        FaceMapper::delete_by_visual_ids(txn, &visual_ids).await?;

        // 涉及人物按 id 升序加锁, 批量减量维护统计与封面
        let mut person_ids: Vec<PersonId> = by_person.keys().copied().collect();
        person_ids.sort();
        for person in PersonMapper::lock_by_ids(txn, &person_ids).await? {
            let faces = by_person.remove(&person.id).ok_or_error(
                "get_person_error",
                "获取人物的人脸错误",
                AppError::InternalServerError,
            )?;

            PersonMapper::remove_faces(txn, person, &faces).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::should_retry;

    #[test]
    fn should_retry_until_max_attempts() {
        // max=3: 第1、2 次失败后重试, 第3 次不再重试
        assert!(should_retry(1, 3));
        assert!(should_retry(2, 3));
        assert!(!should_retry(3, 3));
        // max=1: 只有一次机会
        assert!(!should_retry(1, 1));
        // 非法计数
        assert!(!should_retry(0, 3));
    }
}
