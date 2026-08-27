use audit::{AuditEvent, AuditRecorder};
use std::collections::HashMap;
use std::sync::Arc;

use common::{
    Result,
    error::{AppError, contextual},
    ext::{ContextResultExt, IntoContextualExt, OptionExt, ToOk},
    inc_counter, inc_error, metrics_name,
    models::CursorPage,
    set_gauge,
    utils::{GaugeGuard, MetricsTimer, MetricsTimerExt},
};
use image::{ImageBuffer, Rgb};
use insight_face_rs::Face;
use tokio::{spawn, task::spawn_blocking};
use tracing::{debug, info};
use types::{
    auth::user::{AdminId, UserId},
    cursor::TimeIdCursor,
    photo::{
        FaceView,
        dto::face::{FaceDeleteBatchResult, UnassignedFacePhotoCursorParam},
        dto::photo::PhotoView,
        face::{self, FaceId, FaceRecord},
        models::FaceIds,
        person::PersonId,
        photo::PhotoId,
    },
};

use crate::{
    PhotoState,
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper},
    repo::FaceRepo,
    services::photo_service::{AfterPhotoUpload, PhotoService},
};

pub(crate) struct FaceService;

type Img = ImageBuffer<Rgb<u8>, Vec<u8>>;
// 创建
impl FaceService {
    /// 人脸计算.
    /// 全量时, 备份并且清空表, 从头开始
    /// 增量时, 备份表, 靠是否有photo无face来判断
    #[tracing::instrument(
        skip_all,
        fields(
            user_id = %admin,
            full = %full
        )
    )]
    pub async fn compute(state: Arc<PhotoState>, admin: AdminId, full: bool) -> Result<()> {
        let user_id = admin.into_inner();
        let admin = AdminId::new(user_id)?;
        common::db_transaction!(scoped & state.db, |txn| {
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
    #[common::metered(name = "face_compute")]
    #[tracing::instrument(
        name = "face_compute",
        skip_all,
        fields(user_id = %admin, full = %full)
    )]
    async fn compute_inner(state: Arc<PhotoState>, admin: AdminId, full: bool) -> Result<()> {
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
        let mut previous_id = PhotoId(0);
        let mut batch_idx = 0i64;
        let mut total_photos = 0u64;
        let mut total_faces = 0u64;
        let mut total_no_face = 0u64;
        loop {
            batch_idx += 1;
            set_gauge!("batch", batch_idx as f64);

            let photos = FaceRepo::query_face_compute_photos(&state, full, batch_size, previous_id)
                .timed(metrics_name!("query"))
                .await?;
            if photos.is_empty() {
                info!("第{}批DB查询结果为空, 计算结束", batch_idx);
                break;
            }
            // 刷新previous_id
            if let Some(last) = photos.last() {
                previous_id = last.0;
            }
            let photo_count = photos.len();

            let mut new_faces: Vec<face::NewFaceRecord> = Vec::with_capacity(photo_count * 4);
            let _download_batch_timer = MetricsTimer::start(metrics_name!("download_batch"));
            for (photo_id, file_id) in photos {
                debug!("照片流程开始: photo_id: {photo_id}");
                let _ = async {
                    let img = Self::download_photo(&state, &file_id)
                        .timed(metrics_name!("photo_download"))
                        .await?;
                    let faces = Self::detect_photo(&state, img)
                        .timed(metrics_name!("photo_detect"))
                        .await?;
                    let face_count = faces.len();
                    new_faces.extend(
                        faces
                            .into_iter()
                            .map(|face| face::NewFaceRecord::from_detected(photo_id, face)),
                    );
                    Ok::<usize, AppError>(face_count)
                }
                .await
                .inspect(|&face_count| {
                    inc_counter!("photos_processed", 1);
                    inc_counter!("faces_detected", face_count as u64);
                    total_photos += 1;
                    total_faces += face_count as u64;
                    if face_count == 0 {
                        inc_counter!("no_face_photos", 1);
                        total_no_face += 1;
                    }
                })
                .inspect_err(|_| {
                    common::caller_warn!(photo_id = %photo_id, %file_id, "照片流程错误, 跳过");
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

        set_gauge!("total_photos", total_photos as f64);
        set_gauge!("total_faces", total_faces as f64);
        set_gauge!("total_no_face", total_no_face as f64);

        Ok(())
    }

    /// 从对象存储下载照片并解码为图像缓冲区.
    async fn download_photo(state: &PhotoState, file_id: &String) -> Result<Img> {
        debug!("下载照片{}", file_id);
        let bytes = state
            .s3_client
            .download_with_process(file_id, "image/resize,m_lfit,w_1920,h_1920")
            .await
            .inspect_err(|_| inc_error!("download"))
            .into_contextual()?;

        let img = Self::decode_photo(bytes).await?;

        debug!("下载完成");
        Ok(img)
    }

    /// 在阻塞线程中解码图片字节, 避免占用异步执行器.
    async fn decode_photo(bytes: bytes::Bytes) -> Result<Img> {
        let _decode_timer = MetricsTimer::start(metrics_name!("photo_decode"));
        let decode_result = tokio::task::spawn_blocking(move || -> contextual::Result<Img> {
            image::load_from_memory(&bytes)
                .map(|img| img.into_rgb8())
                .context_error(
                    "decode_image_error",
                    "解码图片失败",
                    AppError::InternalServerError,
                )
        })
        .await
        .inspect_err(|_| inc_error!("decode"))
        .into_contextual()?;
        Ok(decode_result.inspect_err(|_| inc_error!("decode"))?)
    }

    /// 在阻塞线程中执行人脸检测并返回检测结果.
    async fn detect_photo(state: &PhotoState, img: Img) -> Result<Vec<Face>> {
        debug!("检测照片中");
        let face_engine_clone = Arc::clone(&state.face_engine);
        let detect_result = spawn_blocking(move || -> contextual::Result<Vec<Face>> {
            let faces = face_engine_clone.run(&img).context_error(
                "face-engine_run_error",
                "人脸检测模型运行失败",
                AppError::InternalServerError,
            )?;
            debug!("转换成功");
            Ok(faces)
        })
        .await
        .inspect_err(|_| inc_error!("detect"))
        .into_contextual()?;
        Ok(detect_result.inspect_err(|_| inc_error!("detect"))?)
    }

    /// 批量写入检测到的人脸记录.
    async fn insert_faces(state: &PhotoState, faces: Vec<face::NewFaceRecord>) -> Result<()> {
        debug!("插入人脸到数据库中");
        if faces.is_empty() {
            debug!("faces为空, 跳过");
        } else {
            FaceRepo::insert_faces(state, faces)
                .await
                .inspect_err(|_| inc_error!("insert"))
                .into_contextual()?;
        }
        debug!("插入完成");
        Ok(())
    }
}

// 修改
impl FaceService {
    #[common::metered]
    #[tracing::instrument(
        skip_all,
        fields(face_id = %face_id, person_id = ?person_id)
    )]
    /// 修改人脸归属
    pub async fn change_face_belonging(
        state: &PhotoState,
        face_id: FaceId,
        person_id: Option<PersonId>,
        user_id: UserId,
    ) -> Result<()> {
        FaceRepo::change_face_belonging(state, face_id, person_id, user_id).await?;
        Ok(())
    }
}

// 查询
impl FaceService {
    /// 查询指定照片的人脸.
    #[common::metered]
    #[tracing::instrument(skip_all, fields(photo_id = %photo_id))]
    pub async fn get_faces_by_photo_id(
        state: &PhotoState,
        photo_id: PhotoId,
    ) -> Result<Vec<FaceView>> {
        let (faces, person_names) =
            FaceRepo::query_faces_with_person_names(state, photo_id).await?;

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
    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    /// 分页查询包含未分配人脸的照片.
    pub async fn get_unassigned_face_photos(
        state: &PhotoState,
        user_id: UserId,
        req: UnassignedFacePhotoCursorParam,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        let page = FaceRepo::query_unassigned_face_photo_ids(state, &req)
            .timed(metrics_name!("query_unassigned_face_photo_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        let photos = PhotoService::load_photos_info(state, user_id, &page.records)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        page.replace_records(photos)
            .with_next_cursor(|photo| TimeIdCursor {
                time_at: photo.created_at,
                id: photo.id,
            })
            .to_ok()
    }
}

// 删除
impl FaceService {
    /// 删除单张人脸
    /// 仅可以删除无归属的人脸
    #[common::metered]
    #[tracing::instrument(skip_all, fields(face_id = %face_id))]
    pub async fn delete_face(state: &PhotoState, face_id: FaceId, user_id: UserId) -> Result<()> {
        FaceRepo::delete_faces(state, vec![face_id], user_id).await?;

        Ok(())
    }

    /// 批量删除人脸
    /// 仅可以删除无归属的人脸
    #[common::metered]
    #[tracing::instrument(skip_all, fields(count = %face_ids.len()))]
    pub async fn delete_faces(
        state: &PhotoState,
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
    state = crate::state::PhotoState,
    event = crate::services::photo_service::AfterPhotoUpload,
    slice = crate::services::photo_service::AFTER_PHOTO_UPLOAD_CONSUMERS,
    name = "face_recognition",
)]
impl FaceService {
    async fn on_after_photo_upload(
        &self,
        state: Arc<PhotoState>,
        event: Arc<AfterPhotoUpload>,
    ) -> common::Result<()> {
        let image = Self::decode_photo(event.file_data.clone()).await?;
        let faces = Self::detect_photo(&state, image)
            .timed(metrics_name!("photo_detect"))
            .await?;
        let faces = faces
            .into_iter()
            .map(|face| face::NewFaceRecord::from_detected(event.photo.id, face))
            .collect();
        Self::insert_faces(&state, faces)
            .timed(metrics_name!("insert"))
            .await?;
        Ok(())
    }
}

// 当照片删除时
// 删除人脸 和 对应的人物
#[step_derive::declare_transaction_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "face_cleanup",
    owns = ["FaceMapper", "PersonMapper"],
)]
impl FaceService {
    async fn on_photo_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        let photo_ids = ctx.photo_ids();

        // 加行锁读取待删照片的全部人脸, 阻止并发转移归属读到将删人脸
        let faces = FaceMapper::lock_by_photo_ids(txn, &photo_ids).await?;
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
        FaceMapper::delete_by_photo_ids(txn, &photo_ids).await?;

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
