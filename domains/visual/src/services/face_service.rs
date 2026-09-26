use audit::{AuditEvent, AuditRecorder};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

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
    dto::visual::VisualView, face, face::FaceId, face::FaceRecord, models::FaceIds,
    person::PersonId, visual::VisualId, visual::VisualKind,
};

use crate::{
    VisualState,
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper},
    repo::FaceRepo,
    services::visual_service::{AfterVisualUpload, VisualService},
};

pub(crate) struct FaceService;

type Img = ImageBuffer<Rgb<u8>, Vec<u8>>;

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

    /// 从对象存储下载照片并解码为图像缓冲区.
    async fn download_visual(state: &VisualState, file_id: &String) -> Result<Img> {
        debug!("下载照片{}", file_id);
        let bytes = state
            .s3_client
            .download_with_process(file_id, "image/resize,m_lfit,w_1920,h_1920")
            .await
            .into_contextual()?;

        let img = Self::decode_visual(bytes).await?;

        debug!("下载完成");
        Ok(img)
    }

    /// 在阻塞线程中解码图片字节, 避免占用异步执行器.
    async fn decode_visual(bytes: bytes::Bytes) -> Result<Img> {
        let _decode_timer = MetricsTimer::start(metrics_name!("visual_decode"));
        let decode_result = tokio::task::spawn_blocking(move || -> contextual::Result<Img> {
            image::load_from_memory(&bytes)
                .map(|img| img.into_rgb8())
                .context_err(
                    "decode_image_error",
                    "解码图片失败",
                    AppError::bad_request("解码图片失败, 请上传正确的照片"),
                )
        })
        .await
        .into_contextual()?;
        Ok(decode_result?)
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
        _state: Arc<VisualState>,
        event: Arc<AfterVisualUpload>,
    ) -> common_core::Result<()> {
        if event.visual.kind != VisualKind::Image {
            return Ok(());
        }
        FACE_QUEUE
            .get()
            .ok_or_error(
                "face_queue_not_ready",
                "人脸检测队列未初始化",
                AppError::InternalServerError,
            )?
            .send(event)
            .context_err(
                "face_queue_closed",
                "人脸检测队列已关闭",
                AppError::InternalServerError,
            )?;
        Ok(())
    }
}

/// 上传后待做人脸检测的事件队列: 消费者仅入队, 由常驻单 worker 串行消费,
/// 避免大量上传时无限 spawn 任务造成内存压力。
static FACE_QUEUE: OnceLock<mpsc::UnboundedSender<Arc<AfterVisualUpload>>> = OnceLock::new();

// 常驻 worker: 消费上传事件队列, 串行执行人脸检测
impl FaceService {
    /// 启动单线程人脸检测 worker, 由 [`crate::VisualState::new`] 调用;
    /// 重复调用直接 panic。
    pub(crate) fn start_face_consumer(state: Arc<VisualState>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        FACE_QUEUE
            .set(tx)
            .unwrap_or_else(|_| panic!("FACE_QUEUE 重复初始化"));
        let task_manager = state.task_manager.clone();
        task_manager.spawn("face_consumer_worker", async move {
            while let Some(event) = rx.recv().await {
                if let Err(error) = Self::process_face_event(&state, event).await {
                    ContextualError::warn(
                        "face_consume_failed",
                        "人脸检测消费上传事件失败",
                        error,
                        AppError::InternalServerError,
                    )
                    .emit();
                }
            }
        });
    }

    /// 单张照片的人脸检测与入库, 由 worker 串行调用。
    /// 直接对落盘临时文件执行人脸检测(引擎内部解码), 不再持有解码缓冲;
    /// 守卫在消费结束后删除文件。
    #[tracing::instrument(name = "face_compute", skip_all)]
    async fn process_face_event(
        state: &Arc<VisualState>,
        event: Arc<AfterVisualUpload>,
    ) -> common_core::Result<()> {
        if event.visual.kind != VisualKind::Image {
            return Ok(());
        }
        let faces = Self::detect_visual_from_file(state, event.temp_file.path())
            .timed(metrics_name!("visual_detect"))
            .await?;
        let faces = faces
            .into_iter()
            .map(|face| face::NewFaceRecord::from_detected(event.visual.id, face))
            .collect();
        Self::insert_faces(state, faces)
            .timed(metrics_name!("insert"))
            .await?;
        info!("照片 id: {} 人脸检测完成", event.visual.id);
        Ok(())
    }
}

// 当照片删除时
// 删除人脸 和 对应的人物
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "face_cleanup",
    owns = ["FaceMapper", "PersonMapper"],
    method = on_visual_delete,
)]
impl FaceService {
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::visual_service::VisualDeleteContext,
    ) -> common_core::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();

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
