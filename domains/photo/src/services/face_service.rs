use backup::exporter::CsvExporter;
use backup::storage::BackupType;
use common::{Result, error::AppError, ext::ResultErrExt};
use entities::{
    auth::user::UserId,
    photo::{
        face::{self, NewFaceRecord},
        person,
        photo::{self, PhotoId},
    },
};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, Query},
};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::PhotoState;

pub(crate) struct FaceService;

// 创建
impl FaceService {
    pub async fn full_compute(state: &PhotoState, user_id: UserId) -> Result<()> {
        let overall_start = std::time::Instant::now();

        if user_id.0 != 1 {
            warn!("非管理员用户尝试全量计算人脸, id = {}", user_id);
            return Err(AppError::Forbidden("非管理员无法访问".into()));
        }

        info!("开始人脸全量计算, 计算前会清除face和person表, 谨慎运行");

        let backup_start = std::time::Instant::now();
        info!("开始保存face和person表");
        let backup_dir = std::env::temp_dir().join("memory-seek-full-compute");
        std::fs::create_dir_all(&backup_dir)
            .trace_internal_err("photo:face:full_compute:create_backup_dir:err", "创建备份临时目录失败")?;
        let (face_path, face_hash) = CsvExporter::export(&state.db, face::TABLE_NAME, &backup_dir)
            .await
            .trace_internal_err(
                "photo:face:full_compute:save_face:err",
                "人脸全量计算时, 保存face表错误",
            )?;
        info!("face表已导出, hash={}", face_hash);
        let (person_path, person_hash) =
            CsvExporter::export(&state.db, person::TABLE_NAME, &backup_dir)
                .await
                .trace_internal_err(
                    "photo:face:full_compute:save_person:err",
                    "人脸全量计算时, 保存person表错误",
                )?;
        info!("person表已导出, hash={}", person_hash);

        if let Some(storage) = &state.backup_storage {
            let run_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            storage
                .save(face::TABLE_NAME, &face_path, BackupType::Manual, &run_id)
                .await
                .trace_internal_err(
                    "photo:face:full_compute:save_face_storage:err",
                    "保存face表到备份存储失败",
                )?;
            storage
                .save(
                    person::TABLE_NAME,
                    &person_path,
                    BackupType::Manual,
                    &run_id,
                )
                .await
                .trace_internal_err(
                    "photo:face:full_compute:save_person_storage:err",
                    "保存person表到备份存储失败",
                )?;
            info!(run_id = %run_id, "face和person表已保存到备份存储");
        }

        let _ = std::fs::remove_file(&face_path);
        let _ = std::fs::remove_file(&person_path);
        let backup_time = backup_start.elapsed();
        info!("备份阶段耗时: {:?}", backup_time);

        let cleanup_start = std::time::Instant::now();
        info!("开始清除face和person表");
        face::Entity::delete_many()
            .exec(&state.db)
            .await
            .trace_internal_err(
                "photo:face:full_compute:delete_face:err",
                "全量计算人脸时清除face表错误",
            )?;
        person::Entity::delete_many()
            .exec(&state.db)
            .await
            .trace_internal_err(
                "photo:face:full_compute:delete_person:err",
                "全量计算人脸时清除person表错误",
            )?;
        let cleanup_time = cleanup_start.elapsed();
        info!("清除阶段耗时: {:?}", cleanup_time);

        info!("开始分批计算人脸");
        let total = photo::Entity::find()
            .count(&state.db)
            .await
            .trace_internal_err(
                "photo:face:full_compute:find_photo_total:err",
                "人脸全量计算时获取照片数量错误",
            )?;

        let batch_size = 128;
        let batch_num = total / batch_size + 1;
        info!(
            "共{}张照片, batch_size: {}, 轮数: {}",
            total, batch_size, batch_num
        );

        let mut previous_id = -1;
        let mut total_faces = 0usize;
        let mut no_face_count = 0usize;
        let pipeline_start = std::time::Instant::now();

        let concurrency = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(1))
            .unwrap_or(4);

        for i in 0..batch_num {
            let batch_start = std::time::Instant::now();

            let tq = std::time::Instant::now();
            let photos: Vec<(i64, String)> = photo::Entity::find()
                .select_only()
                .column(photo::Column::Id)
                .column(photo::Column::FileId)
                .filter(photo::Column::Id.gt(previous_id))
                .order_by(photo::Column::Id, sea_orm::Order::Asc)
                .limit(batch_size)
                .into_tuple()
                .all(&state.db)
                .await
                .trace_internal_err(
                    "photo:face:full_compute:find_photo:err",
                    "人脸全量计算时获取照片错误",
                )?;
            let query_time = tq.elapsed();

            if photos.is_empty() {
                break;
            }

            if let Some(last) = photos.last() {
                previous_id = last.0;
            }

            let photo_count = photos.len();
            let (dltx, mut dlrx) =
                mpsc::channel::<(PhotoId, image::RgbImage)>(concurrency * 2);
            let (dettx, mut detrx) =
                mpsc::channel::<Vec<face::ActiveModel>>(concurrency * 2);

            // Stage 1: Download + decode (N concurrent)
            let s3 = state.s3_client.clone();
            let mut dl_handles = Vec::with_capacity(photo_count);
            for (photo_id, file_id) in &photos {
                let tx = dltx.clone();
                let s3 = s3.clone();
                let pid = PhotoId(*photo_id);
                let fid = file_id.clone();
                dl_handles.push(tokio::spawn(async move {
                    let img_bytes = s3.download(&fid).await?;
                    let img = image::load_from_memory(&img_bytes)
                        .trace_internal_err(
                            "photo:face:full_compute:load_from_memory:err",
                            "从Bytes转换为image错误",
                        )?
                        .to_rgb8();
                    let _ = tx.send((pid, img)).await;
                    Ok::<_, AppError>(())
                }));
            }
            drop(dltx);

            // Stage 2: Face detection (single-threaded, holds engine lock)
            let engine = state.face_engine.clone();
            let dettx_clone = dettx.clone();
            let detect_handle = tokio::spawn(async move {
                while let Some((photo_id, img)) = dlrx.recv().await {
                    let faces = {
                        let mut engine = engine.lock().trace_internal_err(
                            "photo:face:full_compute:face_engine_lock:err",
                            "获取人脸引擎锁失败",
                        )?;
                        engine.run(&img).trace_internal_err(
                            "photo:face:full_compute:face_engine_run:err",
                            "人脸模型运行错误",
                        )?
                    };
                    let models = if faces.is_empty() {
                        Vec::new()
                    } else {
                        faces
                            .into_iter()
                            .map(|f| NewFaceRecord::from_detected(photo_id, f))
                            .map(|f| face::ActiveModel::try_from(f))
                            .collect::<Result<Vec<face::ActiveModel>>>()
                            .trace_internal_err(
                                "db:photo:face:convert_err",
                                "转换人脸记录失败",
                            )?
                    };
                    let _ = dettx_clone.send(models).await;
                }
                Ok::<_, AppError>(())
            });
            drop(dettx);

            // Stage 3: Batch insert (main task)
            let mut batch_faces = 0usize;
            let mut batch_with_faces = 0usize;
            while let Some(models) = detrx.recv().await {
                if models.is_empty() {
                    continue;
                }
                batch_with_faces += 1;
                batch_faces += models.len();
                face::Entity::insert_many(models)
                    .exec(&state.db)
                    .await
                    .trace_internal_err(
                        "db:photo:face:insert_many:err",
                        "批量插入人脸记录失败",
                    )?;
            }

            for h in dl_handles {
                h.await.expect("download task panicked")?;
            }
            detect_handle.await.expect("detect task panicked")?;

            total_faces += batch_faces;
            no_face_count += photo_count - batch_with_faces;

            info!(
                "第{}/{}批完成 ({:?}), query={:?}, 照片数={}, 含人脸照片={}, 总人脸数={}",
                i + 1,
                batch_num,
                batch_start.elapsed(),
                query_time,
                photo_count,
                batch_with_faces,
                batch_faces,
            );
        }

        info!(
            "全量计算完成: 总耗时={:?}, backup={:?}, 清除={:?}, pipeline={:?}, 总人脸数={}, 无人脸照片数={}",
            overall_start.elapsed(),
            backup_time,
            cleanup_time,
            pipeline_start.elapsed(),
            total_faces,
            no_face_count,
        );

        Ok(())
    }

    pub async fn incremental_compute(state: &PhotoState, user_id: UserId) -> Result<()> {
        if user_id.0 != 1 {
            warn!("非管理员用户尝试增量计算人脸, id = {}", user_id);
            return Err(AppError::Forbidden("非管理员无法访问".into()));
        }

        let overall_start = std::time::Instant::now();
        info!("开始人脸增量计算");

        let concurrency = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(1))
            .unwrap_or(4);
        let batch_size = 128;

        let mut previous_id = -1i64;
        let mut total_photos = 0usize;
        let mut total_faces = 0usize;

        loop {
            let batch_start = std::time::Instant::now();

            let tq = std::time::Instant::now();
            let subquery = Query::select()
                .expr(Expr::col(face::Column::PhotoId))
                .from(face::Entity)
                .to_owned();
            let photos: Vec<(i64, String)> = photo::Entity::find()
                .select_only()
                .column(photo::Column::Id)
                .column(photo::Column::FileId)
                .filter(photo::Column::Id.gt(previous_id))
                .filter(Expr::col(photo::Column::Id).not_in_subquery(subquery))
                .order_by(photo::Column::Id, sea_orm::Order::Asc)
                .limit(batch_size)
                .into_tuple()
                .all(&state.db)
                .await
                .trace_internal_err(
                    "photo:face:incremental:query:err",
                    "增量计算时查询待处理照片失败",
                )?;
            let query_time = tq.elapsed();

            if photos.is_empty() {
                break;
            }

            if let Some(last) = photos.last() {
                previous_id = last.0;
            }
            let photo_count = photos.len();

            let (dltx, mut dlrx) =
                mpsc::channel::<(PhotoId, image::RgbImage)>(concurrency * 2);
            let (dettx, mut detrx) =
                mpsc::channel::<Vec<face::ActiveModel>>(concurrency * 2);

            let s3 = state.s3_client.clone();
            let mut dl_handles = Vec::with_capacity(photo_count);
            for (photo_id, file_id) in &photos {
                let tx = dltx.clone();
                let s3 = s3.clone();
                let pid = PhotoId(*photo_id);
                let fid = file_id.clone();
                dl_handles.push(tokio::spawn(async move {
                    let img_bytes = s3.download(&fid).await?;
                    let img = image::load_from_memory(&img_bytes)
                        .trace_internal_err(
                            "photo:face:incremental:load_from_memory:err",
                            "从Bytes转换为image错误",
                        )?
                        .to_rgb8();
                    let _ = tx.send((pid, img)).await;
                    Ok::<_, AppError>(())
                }));
            }
            drop(dltx);

            let engine = state.face_engine.clone();
            let dettx_clone = dettx.clone();
            let detect_handle = tokio::spawn(async move {
                while let Some((photo_id, img)) = dlrx.recv().await {
                    let faces = {
                        let mut engine = engine.lock().trace_internal_err(
                            "photo:face:incremental:face_engine_lock:err",
                            "获取人脸引擎锁失败",
                        )?;
                        engine.run(&img).trace_internal_err(
                            "photo:face:incremental:face_engine_run:err",
                            "人脸模型运行错误",
                        )?
                    };
                    let models = if faces.is_empty() {
                        Vec::new()
                    } else {
                        faces
                            .into_iter()
                            .map(|f| NewFaceRecord::from_detected(photo_id, f))
                            .map(|f| face::ActiveModel::try_from(f))
                            .collect::<Result<Vec<face::ActiveModel>>>()
                            .trace_internal_err(
                                "photo:face:incremental:convert_err",
                                "转换人脸记录失败",
                            )?
                    };
                    let _ = dettx_clone.send(models).await;
                }
                Ok::<_, AppError>(())
            });
            drop(dettx);

            let mut batch_faces = 0usize;
            let mut batch_with_faces = 0usize;
            while let Some(models) = detrx.recv().await {
                if models.is_empty() {
                    continue;
                }
                batch_with_faces += 1;
                batch_faces += models.len();
                face::Entity::insert_many(models)
                    .exec(&state.db)
                    .await
                    .trace_internal_err(
                        "photo:face:incremental:insert_many:err",
                        "批量插入人脸记录失败",
                    )?;
            }

            for h in dl_handles {
                h.await.expect("download task panicked")?;
            }
            detect_handle.await.expect("detect task panicked")?;

            total_photos += photo_count;
            total_faces += batch_faces;

            info!(
                "增量批次完成 ({:?}), query={:?}, 照片数={}, 含人脸照片={}, 人脸数={}, 累计处理={}, 累计人脸={}",
                batch_start.elapsed(),
                query_time,
                photo_count,
                batch_with_faces,
                batch_faces,
                total_photos,
                total_faces,
            );
        }

        info!(
            "增量计算完成: 总耗时={:?}, 处理照片数={}, 总人脸数={}",
            overall_start.elapsed(),
            total_photos,
            total_faces,
        );

        Ok(())
    }
}

// 修改
impl FaceService {}

// 查询
impl FaceService {}

// 删除
impl FaceService {}
