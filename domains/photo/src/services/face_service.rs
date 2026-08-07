use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use backup::storage::BackupType;
use common::{
    Result,
    error::AppError,
    ext::{OptionExt, ResultErrExt, ToOk, UintExt},
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    utils::{DbUtils, MetricsTimerExt},
};
use image::{ImageBuffer, Rgb};
use insight_face_rs::Face;
use sea_orm::{
    ColumnTrait, Condition, DatabaseTransaction, EntityName, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
    sea_query::{Expr, Query},
};
use tokio::{spawn, task::spawn_blocking};
use tracing::{debug, info, warn};
use types::{
    auth::user::{AdminId, UserId},
    cursor::TimeIdCursor,
    photo::{
        FaceView,
        dto::face::{UnassignedFacePhotoCursorParam, bbox_from_insight},
        dto::photo::PhotoView,
        face::{self, FaceId, FaceRecord},
        person::{self, PersonId, PersonRecord},
        photo::{self, PhotoId},
    },
};

use crate::{
    PhotoState,
    mappers::{
        face_mapper::FaceMapper,
        person_mapper::{PersonCoverUpdate, PersonMapper},
        photo_mapper::PhotoMapper,
    },
    services::photo_service::PhotoService,
};

pub(crate) struct FaceService;

type Img = ImageBuffer<Rgb<u8>, Vec<u8>>;
// 创建
impl FaceService {
    #[tracing::instrument(
        skip_all,
        fields(
            user_id = %admin,
            full = %full
        )
    )]
    pub async fn compute(state: Arc<PhotoState>, admin: AdminId, full: bool) -> Result<()> {
        spawn(async move { Self::compute_inner(state, admin, full).await });
        Ok(())
    }

    async fn compute_inner(state: Arc<PhotoState>, admin: AdminId, full: bool) -> Result<()> {
        metrics_group!();

        let user_id = admin.into_inner();
        info!(user_id = %user_id, "管理员触发人脸计算");

        // 如果是全量计算的话
        // 备份并且清空表
        if full {
            Self::backup_tables(&state).await?;
        }

        let batch_size = 128;
        let mut previous_id = PhotoId(0);
        let mut batch_idx = 0i64;
        loop {
            batch_idx += 1;

            let photos = Self::query_photos(&state, full, batch_size, previous_id).await?;
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
            for (photo_id, file_id) in photos {
                debug!("照片流程开始: photo_id: {photo_id}");
                let _ = async {
                    let img = Self::download_photo(&state, &file_id).await?;
                    let faces = Self::detect_photo(&state, img).await?;

                    for face in faces {
                        let new_face = face::NewFaceRecord::from_detected(photo_id, face);
                        new_faces.push(new_face);
                    }

                    Ok::<(), AppError>(())
                }
                .await
                .inspect_err(|_| {
                    warn!(photo_id = %photo_id, %file_id, "照片流程错误, 跳过");
                });
            }
            Self::insert_faces(&state, new_faces).await?;

            info!(
                "第{}批插入完成, 现共{}",
                batch_idx,
                batch_size * batch_idx as u64
            );
        }

        metrics_success!();
        Ok(())
    }

    async fn backup_tables(state: &PhotoState) -> Result<()> {
        state
            .backup_storage
            .backup_tables(
                &state.db,
                &[face::Entity.table_name(), person::Entity.table_name()],
                BackupType::Manual,
            )
            .timed(metrics_name!("cleanup:backup"))
            .await?;

        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                face::Entity::delete_many().exec(txn).await?;
                person::Entity::delete_many().exec(txn).await?;
                Ok(())
            })
        })
        .await
    }

    async fn query_photos(
        state: &PhotoState,
        full: bool,
        size: u64,
        previous_id: PhotoId,
    ) -> Result<Vec<(PhotoId, String)>> {
        debug!("开始查询照片");

        let condition = if full {
            Condition::all().add(photo::Column::Id.gt(previous_id))
        } else {
            // 增量:仅处理还没有人脸记录的照片
            let subquery = Query::select()
                .expr(Expr::val(1))
                .from(face::Entity)
                .and_where(
                    Expr::col((face::Entity, face::Column::PhotoId))
                        .equals((photo::Entity, photo::Column::Id)),
                )
                .to_owned();

            Condition::all()
                .add(photo::Column::Id.gt(previous_id))
                .add(Expr::exists(subquery).not())
        };

        let photos: Vec<(PhotoId, String)> = photo::Entity::find()
            .select_only()
            .column(photo::Column::Id)
            .column(photo::Column::FileId)
            .filter(condition)
            .order_by(photo::Column::Id, sea_orm::Order::Asc)
            .limit(size)
            .into_tuple::<(PhotoId, String)>()
            .all(&state.db)
            .await?
            .into_iter()
            .collect();

        debug!("查询成功");
        Ok(photos)
    }

    async fn download_photo(state: &PhotoState, file_id: &String) -> Result<Img> {
        debug!("下载照片{}", file_id);
        let bytes = state
            .s3_client
            .download_with_process(file_id, "image/resize,m_lfit,w_1920,h_1920")
            .await?;

        let img = tokio::task::spawn_blocking(move || {
            image::load_from_memory(&bytes)
                .map(|img| img.into_rgb8())
                .trace_internal_err("decode_image_error", "解码图片失败")
        })
        .await?;

        debug!("下载完成");
        img
    }

    async fn detect_photo(state: &PhotoState, img: Img) -> Result<Vec<Face>> {
        debug!("检测照片中");
        let face_engine_clone = Arc::clone(&state.face_engine);
        spawn_blocking(move || {
            debug!("获取face-engine 锁");
            let mut eng = face_engine_clone.lock()?;
            debug!("获取成功");
            let faces = eng
                .run(&img)
                .trace_internal_err("face-engine_run_error", "人脸检测模型运行失败")?;
            debug!("转换成功");
            Ok(faces)
        })
        .await?
    }

    async fn insert_faces(state: &PhotoState, faces: Vec<face::NewFaceRecord>) -> Result<()> {
        debug!("插入人脸到数据库中");
        if faces.is_empty() {
            debug!("faces为空, 跳过");
        } else {
            face::Entity::insert_many(faces.into_iter().map(face::ActiveModel::from))
                .exec_without_returning(&state.db)
                .await?;
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
    #[tracing::instrument(skip_all)]
    pub async fn change_face_belonging(
        state: &PhotoState,
        face_id: FaceId,
        person_id: Option<PersonId>,
    ) -> Result<()> {
        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 加行锁读取人脸(读-改-写流程, 避免并发转移丢更新)
                let face = FaceMapper::lock_by_id(txn, face_id).await?.ok_or_error(
                    "face_not_found",
                    "人脸不存在",
                    AppError::not_found("人脸不存在"),
                )?;

                // 归属未变化(均为 None 或同一人物), 直接返回
                let old_person_id = face.person_id;
                if person_id == old_person_id {
                    return Ok(());
                }

                match person_id {
                    Some(new_person_id) => {
                        // 按 id 升序加锁涉及的两个人物行, 避免并发操作互相死锁(旧人物可能不存在)
                        let (new_person, old_person) = DbUtils::ensure_lock_two_optional_ordered(
                            txn,
                            new_person_id,
                            old_person_id,
                            PersonMapper::lock_by_id,
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

                Ok(())
            })
        })
        .await?;

        Ok(())
    }

    /// 旧人物减量维护: 无人脸则删除; 否则数量/权重/质心减量, 封面可能回退
    async fn remove_face_from_person(
        txn: &DatabaseTransaction,
        person: &PersonRecord,
        face: &FaceRecord,
    ) -> Result<()> {
        if person.face_count == 1 {
            PersonMapper::delete_by_id(txn, person.id).await?;
        } else {
            let old_cover = Self::resolve_cover_after_remove(txn, person, face).await?;
            PersonMapper::update_stats(
                txn,
                person.id,
                person.face_count - 1,
                person.weight - face.score as f64,
                person.centroid.sub_scaled(&face.embedding, face.score),
                old_cover,
            )
            .await?;
        }
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
    pub async fn get_faces_by_photo_id(
        state: &PhotoState,
        photo_id: PhotoId,
    ) -> Result<Vec<FaceView>> {
        let faces = FaceMapper::query_by_photo_id(&state.db, photo_id).await?;

        // 批量加载归属人物名称
        let person_ids: HashSet<PersonId> = faces.iter().filter_map(|f| f.person_id).collect();
        let person_names: HashMap<PersonId, String> = PersonMapper::query_id_and_name_by_ids(
            &state.db,
            &person_ids.iter().copied().collect::<Vec<_>>(),
        )
        .await?
        .into_iter()
        .collect();

        faces
            .into_iter()
            .map(|face| FaceView {
                id: face.id,
                bbox: bbox_from_insight(face.bbox),
                person_id: face.person_id,
                person_name: face.person_id.and_then(|id| person_names.get(&id).cloned()),
            })
            .collect::<Vec<FaceView>>()
            .to_ok()
    }

    /// 获取"包含未分配人脸"的照片列表(游标分页, 不区分照片归属者)
    #[tracing::instrument(skip_all)]
    pub async fn get_unassigned_face_photos(
        state: &PhotoState,
        viewer_user_id: UserId,
        param: UnassignedFacePhotoCursorParam,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        metrics_group!();

        let photo_ids = FaceMapper::query_unassigned_face_photo_ids_cursor_page(
            &state.db,
            param.cursor,
            param.size,
        )
        .timed(metrics_name!("query_unassigned_face_photo_ids"))
        .await?;
        if photo_ids.is_empty() {
            metrics_success!();
            return Ok(CursorPage::empty());
        }

        let photos = PhotoService::load_photos_info(state, viewer_user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        let page = CursorPage::from_oversize_fn(photos, param.size, |photo| {
            TimeIdCursor {
                created_at: photo.created_at,
                id: photo.id,
            }
            .to_ok()
        })?;

        metrics_success!();
        Ok(page)
    }
}

// 删除
impl FaceService {
    /// 删除单张人脸(仅限未归属人物的人脸, 避免破坏人物统计不变量)
    #[tracing::instrument(skip_all)]
    pub async fn delete_face(state: &PhotoState, face_id: FaceId) -> Result<()> {
        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 加行锁读取人脸(读-改-写流程, 防止并发转移归属后误删)
                let face = FaceMapper::lock_by_id(txn, face_id).await?.ok_or_error(
                    "face_not_found",
                    "人脸不存在",
                    AppError::not_found("人脸不存在"),
                )?;

                // 已归属人物的人脸禁止直接删除(需先取消归属), 防止人物统计悬空
                if face.person_id.is_some() {
                    return Err(AppError::bad_request(
                        "人脸已归属人物, 请先取消归属后再删除",
                    ));
                }

                FaceMapper::delete_by_id(txn, face_id)
                    .await?
                    .no_zero_or_warn(
                        "face_delete_fail",
                        "删除人脸失败",
                        AppError::bad_request("删除人脸失败"),
                    )?;
                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}

// 照片删除步骤:人脸清理(占位)
#[step_derive::declare_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
    name = "face_cleanup",
    owns = ["FaceMapper", "PersonMapper"],
)]
impl FaceService {
    /// 需删除 `photo_face` 并维护 `photo_person` 的
    /// `face_count / weight / centroid / cover` 统计,逻辑参照
    /// `FaceService::change_face_belonging`(见 `docs/change-face-belonging-plan.md`)。
    /// 本次仅注册占位,保持与现状一致的"人脸暂不清理"行为。
    async fn on_photo_delete(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _ctx: &mut crate::services::photo_service::PhotoDeleteContext,
    ) -> common::Result<()> {
        // TODO: 删除照片的人脸记录并维护人物统计
        Ok(())
    }
}
