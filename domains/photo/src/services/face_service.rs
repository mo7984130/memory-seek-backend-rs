use backup::storage::BackupType;
use common::{
    Result,
    error::AppError,
    ext::ResultErrExt,
    inc_counter, metrics_group, metrics_name, metrics_success, set_gauge, timed,
    utils::{GaugeGuard, MetricsTimer, MetricsTimerExt},
};
use entities::{
    auth::user::UserId,
    photo::{
        face::{self, NewFaceRecord},
        person,
        photo::{self, PhotoId},
    },
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
    sea_query::{Expr, Query},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::PhotoState;

pub(crate) struct FaceService;

// 创建
impl FaceService {
    #[tracing::instrument(skip_all, fields(user_id = %user_id, full = %full))]
    pub async fn compute(state: &PhotoState, user_id: UserId, full: bool) -> Result<()> {
        metrics_group!("face:compute");

        // 鉴权
        if user_id != UserId(1) {
            warn!("非管理员用户尝试人脸计算, id = {}", user_id);
            return Err(AppError::Forbidden("非管理员无法访问".into()));
        }

        let _running = GaugeGuard::start("photo:face:compute:running");
        set_gauge!("face:compute", "mode", if full { 1.0 } else { 0.0 });

        // 如果是全量计算的话
        // 备份并且清空表
        if full {
            info!("开始人脸全量计算");

            timed!("face:compute", "backup", {
                info!("开始备份face和person表");
                state
                    .backup_storage
                    .backup_tables(
                        &state.db,
                        &[face::TABLE_NAME, person::TABLE_NAME],
                        BackupType::Manual,
                    )
                    .await
                    .trace_internal_err(
                        "photo:face:full_compute:backup_tables:err",
                        "备份face和person表失败",
                    )?;
            });

            timed!("face:compute", "cleanup", {
                info!("开始清除face和person表");
                let txn = state
                    .db
                    .begin()
                    .await
                    .trace_internal_err("photo:face:full_compute:begin_txn:err", "开启事务失败")?;
                face::Entity::delete_many()
                    .exec(&txn)
                    .await
                    .trace_internal_err(
                        "photo:face:full_compute:delete_face:err",
                        "全量计算人脸时清除face表错误",
                    )?;
                person::Entity::delete_many()
                    .exec(&txn)
                    .await
                    .trace_internal_err(
                        "photo:face:full_compute:delete_person:err",
                        "全量计算人脸时清除person表错误",
                    )?;
                txn.commit().await.trace_internal_err(
                    "photo:face:full_compute:commit_txn:err",
                    "提交清除事务失败",
                )?;
            });
        } else {
            info!("开始人脸增量计算");
        }

        info!("开始分批计算人脸");

        let batch_size = 128;
        let subquery = Query::select()
            .expr(Expr::col(face::Column::PhotoId))
            .from(face::Entity)
            .to_owned();

        let mut previous_id = -1i64;
        let mut total_faces = 0usize;
        let mut no_face_count = 0usize;
        let mut total_photos = 0usize;
        let mut batch_idx = 0u32;

        let _batch_timer = MetricsTimer::start(metrics_name!("face:compute", "batch_loop"));
        loop {
            batch_idx += 1;
            set_gauge!("face:compute", "batch", batch_idx as f64);

            let condition = if full {
                Condition::all().add(photo::Column::Id.gt(previous_id))
            } else {
                Condition::all()
                    .add(photo::Column::Id.gt(previous_id))
                    .add(Expr::col(photo::Column::Id).not_in_subquery(subquery.clone()))
            };
            let photos: Vec<(i64, String)> = photo::Entity::find()
                .select_only()
                .column(photo::Column::Id)
                .column(photo::Column::FileId)
                .filter(condition)
                .order_by(photo::Column::Id, sea_orm::Order::Asc)
                .limit(batch_size)
                .into_tuple()
                .all(&state.db)
                .timed(metrics_name!("face:compute", "query"))
                .await
                .trace_internal_err(
                    "photo:face:compute:find_photo:err",
                    "人脸计算时获取照片错误",
                )?;

            if photos.is_empty() {
                break;
            }

            if let Some(last) = photos.last() {
                previous_id = last.0;
            }

            let photo_count = photos.len();
            info!(
                "第{}批查询到{}张照片, 起始ID={}",
                batch_idx, photo_count, previous_id
            );
            let (download_tx, mut download_rx) = mpsc::channel::<(PhotoId, image::RgbImage)>(2);
            let (detect_tx, mut detect_rx) = mpsc::channel::<Vec<face::ActiveModel>>(2);

            let s3 = state.s3_client.clone();
            let download_errs = Arc::new(AtomicU32::new(0));
            let dlerr = download_errs.clone();
            let download_tx_clone = download_tx.clone();
            let download_handle = tokio::spawn(async move {
                let _timer = MetricsTimer::start("photo:face:compute:download:duration_quantile");
                for (photo_id, file_id) in photos {
                    let img_bytes = match s3
                        .download_with_process(&file_id, "image/resize,m_lfit,w_1920,h_1920")
                        .await
                    {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(pid = photo_id, "下载失败: {}", e);
                            dlerr.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    };
                    let img = match image::load_from_memory(&img_bytes) {
                        Ok(img) => img.to_rgb8(),
                        Err(e) => {
                            warn!(pid = photo_id, "解码失败: {}", e);
                            dlerr.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    };
                    // download_rx 被 drop 时下游已退出，正常结束
                    if download_tx_clone
                        .send((PhotoId(photo_id), img))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok::<(), AppError>(())
            });
            drop(download_tx);

            let engine = state.face_engine.clone();
            let detect_errs = Arc::new(AtomicU32::new(0));
            let deerr = detect_errs.clone();
            let detect_tx_clone = detect_tx.clone();
            let detect_handle = tokio::spawn(async move {
                let _timer = MetricsTimer::start("photo:face:compute:detect:duration_quantile");
                while let Some((photo_id, img)) = download_rx.recv().await {
                    let eng = engine.clone();
                    let run_result = tokio::task::spawn_blocking(move || {
                        let mut eng = eng.lock().map_err(|_| AppError::InternalServerError)?;
                        eng.run(&img).map_err(|_| AppError::InternalServerError)
                    })
                    .await
                    .map_err(|_| AppError::InternalServerError)?;

                    let models = match run_result {
                        Ok(faces) if !faces.is_empty() => {
                            match faces
                                .into_iter()
                                .map(|f| NewFaceRecord::from_detected(photo_id, f))
                                .map(|f| face::ActiveModel::try_from(f))
                                .collect::<Result<Vec<face::ActiveModel>>>()
                            {
                                Ok(m) => m,
                                Err(e) => {
                                    warn!(pid = ?photo_id, "转换人脸记录失败: {}", e);
                                    deerr.fetch_add(1, Ordering::Relaxed);
                                    Vec::new()
                                }
                            }
                        }
                        Ok(_) => Vec::new(),
                        Err(e) => {
                            warn!(pid = ?photo_id, "人脸检测失败: {:?}", e);
                            deerr.fetch_add(1, Ordering::Relaxed);
                            Vec::new()
                        }
                    };
                    // detect_rx 被 drop 时 main task 已退出，正常结束
                    let _ = detect_tx_clone.send(models).await;
                }
                Ok::<(), AppError>(())
            });
            drop(detect_tx);

            info!("第{}批开始下载+检测, 照片数={}", batch_idx, photo_count);

            let mut batch_faces = 0usize;
            let mut batch_with_faces = 0usize;
            let mut batch_insert_errs = 0u32;
            while let Some(models) = detect_rx.recv().await {
                if models.is_empty() {
                    continue;
                }
                batch_with_faces += 1;
                batch_faces += models.len();
                let insert_result = timed!("face:compute", "insert", {
                    face::Entity::insert_many(models).exec(&state.db).await
                });
                if let Err(e) = insert_result {
                    warn!("批量插入人脸记录失败: {}", e);
                    batch_insert_errs += 1;
                }
            }

            info!(
                "第{}批写入完成, 含人脸照片={}, 人脸数={}",
                batch_idx, batch_with_faces, batch_faces
            );
            download_handle.await.expect("download task panicked")?;
            detect_handle.await.expect("detect task panicked")?;
            let batch_errors = download_errs.load(Ordering::Relaxed)
                + detect_errs.load(Ordering::Relaxed)
                + batch_insert_errs;

            total_faces += batch_faces;
            no_face_count += photo_count - batch_with_faces;
            total_photos += photo_count;

            inc_counter!("photos_processed", photo_count as u64);
            inc_counter!("faces_detected", batch_faces as u64);
            inc_counter!("no_face_photos", (photo_count - batch_with_faces) as u64);
            inc_counter!("errors", batch_errors as u64);

            info!(
                "第{}批完成, 照片数={}, 含人脸={}, 人脸数={}, 失败={}, 累计照片={}, 累计人脸={}, 累计无人脸={}",
                batch_idx,
                photo_count,
                batch_with_faces,
                batch_faces,
                batch_errors,
                total_photos,
                total_faces,
                no_face_count,
            );
        }

        let mode = if full { "全量" } else { "增量" };
        info!(
            "{}计算完成: 处理照片数={}, 总人脸数={}, 无人脸照片数={}",
            mode, total_photos, total_faces, no_face_count,
        );

        metrics_success!("face:compute");
        Ok(())
    }
}

// 修改
impl FaceService {}

// 查询
impl FaceService {}

// 删除
impl FaceService {}
