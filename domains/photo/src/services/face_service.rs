use audit::{AuditEvent, AuditService};
use std::collections::HashMap;
use std::sync::Arc;

use common::{
    Result,
    error::{AppError, ContextualError, contextual},
    ext::{ContextResultExt, IntoContextualExt, OptionExt, ToOk, UintExt},
    inc_counter, inc_error, metrics_name,
    models::CursorPage,
    set_gauge,
    utils::{DbUtils, GaugeGuard, MetricsTimer, MetricsTimerExt},
};
use image::{ImageBuffer, Rgb};
use insight_face_rs::Face;
use sea_orm::{DatabaseTransaction, EntityTrait};
use tokio::{spawn, task::spawn_blocking};
use tracing::{debug, info};
use types::{
    auth::user::{AdminId, UserId},
    cursor::TimeIdCursor,
    photo::{
        FaceView,
        dto::face::{FaceDeleteBatchResult, UnassignedFacePhotoCursorParam, bbox_from_insight},
        dto::photo::PhotoView,
        face::{self, FaceId, FaceRecord},
        models::FaceIds,
        person::{self, PersonId, PersonRecord},
        photo::PhotoId,
    },
};

use crate::{
    PhotoState,
    mappers::{
        face_mapper::FaceMapper,
        person_mapper::{PersonCoverUpdate, PersonMapper},
        photo_mapper::PhotoMapper,
    },
    repo::FaceRepo,
    services::photo_service::{AfterPhotoUpload, PhotoService},
};

pub(crate) struct FaceService;

type Img = ImageBuffer<Rgb<u8>, Vec<u8>>;
// 创建
impl FaceService {
    /// 异步启动人脸计算任务; 任务失败通过后台日志记录, 不阻塞请求返回.
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
            AuditService::append(
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

    /// 批量下载照片, 检测人脸并写入人脸记录.
    #[common::metered(name = "face_compute")]
    #[tracing::instrument(
        name = "face_compute",
        skip_all,
        fields(user_id = %admin, full = %full)
    )]
    async fn compute_inner(state: Arc<PhotoState>, admin: AdminId, full: bool) -> Result<()> {
        let user_id = admin.into_inner();
        info!(user_id = %user_id, "管理员触发人脸计算");

        // 并发度守卫：进入 +1，退出时 -1
        let _running_guard = GaugeGuard::start(metrics_name!("running"));
        // mode 为 category gauge，以标签区分 full / incremental
        #[cfg(feature = "metrics")]
        {
            metrics::gauge!("photo:face_compute:mode", "mode" => "full").set(if full {
                1.0
            } else {
                0.0
            });
            metrics::gauge!("photo:face_compute:mode", "mode" => "incremental").set(if full {
                0.0
            } else {
                1.0
            });
        }

        // 如果是全量计算的话
        // 备份并且清空表
        if full {
            Self::backup_tables(&state).await?;
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

            let photos = Self::query_photos(&state, full, batch_size, previous_id)
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

            let _insert_phase_timer = MetricsTimer::start(metrics_name!("insert_phase"));
            Self::insert_faces(&state, new_faces)
                .timed(metrics_name!("insert"))
                .await?;
            drop(_insert_phase_timer);

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

    /// 备份并清空人脸相关表, 为全量重算准备一致的起始状态.
    async fn backup_tables(state: &PhotoState) -> Result<()> {
        FaceRepo::backup_face_tables(state)
            .timed(metrics_name!("cleanup:backup"))
            .await?;

        common::db_transaction!(scoped & state.db, |txn| {
            face::Entity::delete_many()
                .exec(txn)
                .await
                .into_contextual()?;
            person::Entity::delete_many()
                .exec(txn)
                .await
                .into_contextual()?;
            Ok(())
        })
        .await
        .map_err(|error| error.emit())
    }

    /// 按 ID 游标批量查询待处理照片.
    async fn query_photos(
        state: &PhotoState,
        full: bool,
        size: u64,
        previous_id: PhotoId,
    ) -> Result<Vec<(PhotoId, String)>> {
        debug!("开始查询照片");

        let photos = FaceRepo::query_face_compute_photos(state, full, size, previous_id).await?;

        debug!("查询成功");
        Ok(photos)
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
            debug!("获取face-engine 锁");
            let mut eng = face_engine_clone.lock().map_err(|error| {
                ContextualError::error(
                    "poison_error",
                    "人脸引擎锁中毒",
                    error.to_string(),
                    AppError::InternalServerError,
                )
            })?;
            debug!("获取成功");
            let faces = eng.run(&img).context_error(
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
    /// 修改人脸归属: 将单张人脸移动到指定人物, `person_id` 为 `None` 时取消归属
    ///
    /// 在同一事务内维护全部不变量(见 `docs/change-face-belonging-plan.md`):
    /// - 新旧人物 `face_count` 增量维护, 旧人物无人脸后删除;
    /// - 新旧人物 `weight`/`centroid` 增量维护(±score / ±score×embedding);
    /// - 封面按「score 最高 = 封面」规则维护: 移入可能替换新人物封面,
    ///   移出若曾是旧人物封面则回退到剩余 score 最高人脸;
    /// - 涉及的行加锁且按 id 升序, 避免并发丢失更新与死锁。
    #[common::metered]
    #[tracing::instrument(
        skip_all,
        fields(face_id = %face_id, person_id = ?person_id)
    )]
    /// 将人脸转移到指定人物或取消其人物归属, 并维护双方统计和封面.
    pub async fn change_face_belonging(
        state: &PhotoState,
        face_id: FaceId,
        person_id: Option<PersonId>,
        user_id: UserId,
    ) -> Result<()> {
        let affected_person_ids: Vec<PersonId> =
            common::db_transaction!(scoped & state.db, |txn| {
                // 加行锁读取人脸(读-改-写流程, 避免并发转移丢更新)
                let face = FaceMapper::lock_by_id(txn, face_id).await?.ok_or_error(
                    "face_not_found",
                    "人脸不存在",
                    AppError::not_found("人脸不存在"),
                )?;

                // 归属未变化(均为 None 或同一人物), 直接返回
                let old_person_id = face.person_id;
                if person_id == old_person_id {
                    return Ok(Vec::new());
                }

                match person_id {
                    Some(new_person_id) => {
                        // 按 id 升序加锁涉及的两个人物行, 避免并发操作互相死锁(旧人物可能不存在)
                        let (new_person, old_person) = DbUtils::ensure_lock_two_optional_ordered(
                            txn,
                            new_person_id,
                            old_person_id,
                            |db, id| async move { Ok(PersonMapper::lock_by_id(db, id).await?) },
                            |person| {
                                person.ok_or_error(
                                    "person_not_found",
                                    "人物不存在",
                                    AppError::not_found("人物不存在"),
                                )
                            },
                        )
                        .await?;

                        // 移动人脸归属
                        FaceMapper::update_face_person_id(txn, face_id, new_person_id)
                            .await?
                            .no_zero_or_warn(
                                "face_belonging_change_fail",
                                "修改人脸归属失败",
                                AppError::bad_request("修改人脸归属失败"),
                            )?;

                        // 新人物: 数量/权重/质心增量, 封面按 score 规则可能替换
                        let new_cover =
                            Self::resolve_cover_after_add(txn, &new_person, &face).await?;
                        PersonMapper::update_stats(
                            txn,
                            new_person_id,
                            new_person.face_count + 1,
                            new_person.weight + face.score as f64,
                            new_person.centroid.add_scaled(&face.embedding, face.score),
                            new_cover,
                        )
                        .await?;

                        // 旧人物: 减量维护(无人脸则删除)
                        if let Some(old_person) = old_person {
                            Self::remove_face_from_person(txn, &old_person, &face).await?;
                        }
                    }
                    // 取消归属: 仅需处理旧人物减量维护
                    None => {
                        let old_person = PersonMapper::lock_by_id(
                            txn,
                            old_person_id.ok_or_error(
                                "face_belonging_change_fail",
                                "取消人脸归属失败",
                                AppError::InternalServerError,
                            )?,
                        )
                        .await?
                        .ok_or_error(
                            "person_not_found",
                            "人物不存在",
                            AppError::not_found("人物不存在"),
                        )?;

                        FaceMapper::clear_face_person_id(txn, face_id)
                            .await?
                            .no_zero_or_warn(
                                "face_belonging_change_fail",
                                "取消人脸归属失败",
                                AppError::bad_request("取消人脸归属失败"),
                            )?;

                        Self::remove_face_from_person(txn, &old_person, &face).await?;
                    }
                }

                // 返回受影响人物 ID（新旧人物）, 事务提交后用于失效人物缓存
                let mut affected = Vec::with_capacity(2);
                if let Some(pid) = old_person_id {
                    affected.push(pid);
                }
                if let Some(pid) = person_id {
                    affected.push(pid);
                }
                let event = AuditEvent::new(if person_id.is_some() {
                    "face_change_belonging"
                } else {
                    "face_unassign"
                })
                .with_actor(user_id.0)
                .with_target("face", face_id.0);
                let event = person_id.map_or(event.clone(), |id| {
                    event.with_detail(serde_json::json!({ "toPersonId": id.0 }))
                });
                AuditService::append(txn, event).await?;
                Ok(affected)
            })
            .await?;

        // 失效受影响人物缓存（L1 + L2）：新旧人物 face_count/封面/质心均已变化
        // 错误不返回
        crate::repo::PersonRepo::invalidate_persons(state, &affected_person_ids).await;

        Ok(())
    }

    /// 旧人物减量维护: 无人脸则删除; 否则数量/权重/质心减量, 封面可能回退
    async fn remove_face_from_person(
        txn: &DatabaseTransaction,
        person: &PersonRecord,
        face: &FaceRecord,
    ) -> Result<()> {
        Self::remove_faces_from_person(txn, person, std::slice::from_ref(face)).await
    }

    /// 批量减量维护: 将一组人脸从人物中移除(删除照片场景, 语义与单张移除一致)
    ///
    /// - 剩余人脸为 0 则删除人物;
    /// - 否则 `face_count` 减量 / `weight` 减 Σscore / `centroid` 减 Σ(score×embedding);
    /// - 被移除人脸含封面人脸时, 封面回退到剩余 score 最高人脸
    ///   (调用方需先删除该组人脸或转移其归属, 使回退查询到的即为剩余人脸)。
    async fn remove_faces_from_person(
        txn: &DatabaseTransaction,
        person: &PersonRecord,
        faces: &[FaceRecord],
    ) -> Result<()> {
        let removed = faces.len() as u64;
        if person.face_count <= removed {
            PersonMapper::delete_by_id(txn, person.id).await?;
            return Ok(());
        }

        let mut weight = person.weight;
        let mut centroid = person.centroid;
        for face in faces {
            weight -= face.score as f64;
            centroid = centroid.sub_scaled(&face.embedding, face.score);
        }

        // 封面回退: 仅当被移除人脸含封面人脸时才需重选
        let cover = if let Some(cover_face) = faces.iter().find(|f| f.id == person.cover_face_id) {
            Self::resolve_cover_after_remove(txn, person, cover_face).await?
        } else {
            None
        };

        PersonMapper::update_stats(
            txn,
            person.id,
            person.face_count - removed,
            weight,
            centroid,
            cover,
        )
        .await?;

        Ok(())
    }

    /// 封面决策: 人脸移入后的新人物封面
    ///
    /// 集合只新增一个元素, 封面要么不变, 要么是这张被移入的人脸
    /// (score 严格更高才替换, 相等保持现状保证封面稳定)。
    async fn resolve_cover_after_add(
        txn: &DatabaseTransaction,
        person: &PersonRecord,
        face: &FaceRecord,
    ) -> Result<Option<PersonCoverUpdate>> {
        let cover_score = FaceMapper::query_by_id(txn, person.cover_face_id)
            .await?
            .ok_or_error(
                "person_cover_face_not_found",
                "人物封面人脸不存在",
                AppError::InternalServerError,
            )?
            .score;
        if face.score > cover_score {
            let file_id = PhotoMapper::query_file_id_by_id(txn, face.photo_id)
                .await?
                .ok_or_error(
                    "person_cover_photo_not_found",
                    "封面人脸所属照片不存在",
                    AppError::InternalServerError,
                )?;
            Ok(Some(PersonCoverUpdate {
                cover_face_id: face.id,
                cover_photo_id: face.photo_id,
                cover_file_id: file_id,
                cover_bbox: bbox_from_insight(face.bbox),
            }))
        } else {
            Ok(None)
        }
    }

    /// 封面决策: 人脸移出后的旧人物封面回退
    ///
    /// 仅当被移人脸曾是旧人物封面时才需要回退, 取剩余 score 最高人脸;
    /// 其余情况封面不变。
    async fn resolve_cover_after_remove(
        txn: &DatabaseTransaction,
        person: &PersonRecord,
        face: &FaceRecord,
    ) -> Result<Option<PersonCoverUpdate>> {
        if person.cover_face_id != face.id {
            return Ok(None);
        }
        let top = FaceMapper::query_top_score_by_person_id(txn, person.id)
            .await?
            .ok_or_error(
                "person_cover_face_not_found",
                "人物封面人脸不存在",
                AppError::InternalServerError,
            )?;
        let file_id = PhotoMapper::query_file_id_by_id(txn, top.photo_id)
            .await?
            .ok_or_error(
                "person_cover_photo_not_found",
                "封面人脸所属照片不存在",
                AppError::InternalServerError,
            )?;
        Ok(Some(PersonCoverUpdate {
            cover_face_id: top.id,
            cover_photo_id: top.photo_id,
            cover_file_id: file_id,
            cover_bbox: bbox_from_insight(top.bbox),
        }))
    }
}

// 查询
impl FaceService {
    /// 查询指定照片的人脸视图.
    #[common::metered]
    #[tracing::instrument(skip_all, fields(photo_id = %photo_id))]
    pub async fn get_faces_by_photo_id(
        state: &PhotoState,
        photo_id: PhotoId,
    ) -> Result<Vec<FaceView>> {
        let (faces, person_names) =
            FaceRepo::query_faces_with_person_names(state, photo_id).await?;

        // 批量加载归属人物名称
        let person_names: HashMap<PersonId, String> = person_names.into_iter().collect();

        let views = faces
            .into_iter()
            .map(|face| FaceView {
                id: face.id,
                bbox: bbox_from_insight(face.bbox),
                person_id: face.person_id,
                person_name: face.person_id.and_then(|id| person_names.get(&id).cloned()),
            })
            .collect::<Vec<FaceView>>();

        views.to_ok()
    }

    /// 获取"包含未分配人脸"的照片列表(游标分页, 不区分照片归属者)
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

        Ok(page
            .replace_records(photos)
            .with_next_cursor(|photo| TimeIdCursor {
                time_at: photo.created_at,
                id: photo.id,
            }))
    }
}

// 删除
impl FaceService {
    /// 删除单张人脸(仅限未归属人物的人脸, 避免破坏人物统计不变量)
    #[common::metered]
    #[tracing::instrument(skip_all, fields(face_id = %face_id))]
    /// 删除一张未归属人物的人脸.
    pub async fn delete_face(state: &PhotoState, face_id: FaceId, user_id: UserId) -> Result<()> {
        common::db_transaction!(scoped & state.db, |txn| {
            // 加行锁读取人脸(读-改-写流程, 防止并发转移归属后误删)
            let face = FaceMapper::lock_by_id(txn, face_id).await?.ok_or_error(
                "face_not_found",
                "人脸不存在",
                AppError::not_found("人脸不存在"),
            )?;

            // 已归属人物的人脸禁止直接删除(需先取消归属), 防止人物统计悬空
            if face.person_id.is_some() {
                inc_error!("conflict");
                return Err(ContextualError::warn_without_source(
                    "face_delete_conflict",
                    "人脸已归属人物, 请先取消归属后再删除",
                    AppError::bad_request("人脸已归属人物, 请先取消归属后再删除"),
                ));
            }

            FaceMapper::delete_by_id(txn, face_id)
                .await?
                .no_zero_or_warn(
                    "face_delete_fail",
                    "删除人脸失败",
                    AppError::bad_request("删除人脸失败"),
                )?;
            AuditService::append(
                txn,
                AuditEvent::new("face_delete")
                    .with_actor(user_id.0)
                    .with_target("face", face_id.0),
            )
            .await?;
            Ok(())
        })
        .await?;

        Ok(())
    }

    /// 批量删除未归属人脸(仅限未归属人物的人脸, 避免破坏人物统计不变量)
    ///
    /// 与单张删除一致, 已归属人脸会被跳过, 不参与删除;
    /// 通过 SQL 条件 `person_id IS NULL` 原子过滤, 无需逐张加锁。
    #[common::metered]
    #[tracing::instrument(skip_all, fields(count = %face_ids.len()))]
    /// 批量删除未归属人物的人脸, 并返回实际删除数量.
    pub async fn delete_faces_batch(
        state: &PhotoState,
        face_ids: &FaceIds,
        user_id: UserId,
    ) -> Result<FaceDeleteBatchResult> {
        let deleted_face_count =
            FaceRepo::delete_unassigned_faces(state, face_ids, user_id).await?;

        Ok(FaceDeleteBatchResult { deleted_face_count })
    }
}

/// 新上传照片的人脸检测。事件在后台分发，检测或写入失败只记录日志，不影响上传结果。
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

// 照片删除步骤:人脸清理
#[step_derive::declare_transaction_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "face_cleanup",
    owns = ["FaceMapper", "PersonMapper"],
)]
impl FaceService {
    /// 删除照片的人脸记录, 并维护受影响 `photo_person` 的
    /// `face_count / weight / centroid / cover` 统计(减量维护语义参照
    /// `FaceService::change_face_belonging`, 见 `docs/change-face-belonging-plan.md`)。
    ///
    /// 并发安全: 先按 `photo_id` 加行锁锁定全部人脸, 再按人物 id 升序锁定
    /// 受影响人物 —— 与转移归属的「先人脸后人物」加锁顺序一致, 避免互死锁与丢失更新。
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
        for person_id in person_ids {
            let person = PersonMapper::lock_by_id(txn, person_id)
                .await?
                .ok_or_error(
                    "person_not_found",
                    "人物不存在",
                    AppError::not_found("人物不存在"),
                )?;
            let faces = &by_person[&person_id];
            Self::remove_faces_from_person(txn, &person, faces).await?;
        }

        // 记录受影响人物 ID, 供 delete_photos 在事务提交后失效人物缓存
        ctx.affected_person_ids = by_person.keys().copied().collect();

        Ok(())
    }
}
