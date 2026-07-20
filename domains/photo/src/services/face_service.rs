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
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, Query},
};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::PhotoState;

pub(crate) struct FaceService;

// 创建
impl FaceService {
    pub async fn compute(state: &PhotoState, user_id: UserId, full: bool) -> Result<()> {
        if user_id.0 != 1 {
            warn!("非管理员用户尝试人脸计算, id = {}", user_id);
            return Err(AppError::Forbidden("非管理员无法访问".into()));
        }

        let overall_start = std::time::Instant::now();

        let (backup_time, cleanup_time) = if full {
            info!("开始人脸全量计算, 计算前会清除face和person表, 谨慎运行");

            let backup_start = std::time::Instant::now();
            info!("开始备份face和person表");
            if let Some(storage) = &state.backup_storage {
                storage
                    .backup_tables(&state.db, &[face::TABLE_NAME, person::TABLE_NAME], BackupType::Manual)
                    .await
                    .trace_internal_err("photo:face:full_compute:backup_tables:err", "备份face和person表失败")?;
            }
            let backup_time = backup_start.elapsed();
            info!("备份阶段耗时: {:?}", backup_time);

            let cleanup_start = std::time::Instant::now();
            info!("开始清除face和person表");
            face::Entity::delete_many()
                .exec(&state.db)
                .await
                .trace_internal_err("photo:face:full_compute:delete_face:err", "全量计算人脸时清除face表错误")?;
            person::Entity::delete_many()
                .exec(&state.db)
                .await
                .trace_internal_err("photo:face:full_compute:delete_person:err", "全量计算人脸时清除person表错误")?;
            let cleanup_time = cleanup_start.elapsed();
            info!("清除阶段耗时: {:?}", cleanup_time);

            (backup_time, cleanup_time)
        } else {
            info!("开始人脸增量计算");
            (std::time::Duration::ZERO, std::time::Duration::ZERO)
        };

        info!("开始分批计算人脸");

        let batch_size = 128;
        let subquery = Query::select()
            .expr(Expr::col(face::Column::PhotoId))
            .from(face::Entity)
            .to_owned();

        let mut previous_id = -1i64;
        let mut total_faces = 0usize;
        let mut no_face_count = 0usize;
        let pipeline_start = std::time::Instant::now();
        let mut batch_idx = 0u32;

        loop {
            batch_idx += 1;
            let batch_start = std::time::Instant::now();

            let tq = std::time::Instant::now();
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
                .await
                .trace_internal_err("photo:face:compute:find_photo:err", "人脸计算时获取照片错误")?;
            let query_time = tq.elapsed();

            if photos.is_empty() {
                break;
            }

            if let Some(last) = photos.last() {
                previous_id = last.0;
            }

            let photo_count = photos.len();
            let (dltx, mut dlrx) = mpsc::channel::<(PhotoId, image::RgbImage)>(2);
            let (dettx, mut detrx) = mpsc::channel::<Vec<face::ActiveModel>>(2);

            let s3 = state.s3_client.clone();
            let dltx_clone = dltx.clone();
            let download_handle = tokio::spawn(async move {
                for (photo_id, file_id) in photos {
                    let img_bytes = s3.download(&file_id).await?;
                    let img = image::load_from_memory(&img_bytes)
                        .trace_internal_err("photo:face:compute:load_from_memory:err", "从Bytes转换为image错误")?
                        .to_rgb8();
                    if dltx_clone.send((PhotoId(photo_id), img)).await.is_err() {
                        break;
                    }
                }
                Ok::<_, AppError>(())
            });
            drop(dltx);

            let engine = state.face_engine.clone();
            let dettx_clone = dettx.clone();
            let detect_handle = tokio::spawn(async move {
                while let Some((photo_id, img)) = dlrx.recv().await {
                    let faces = {
                        let mut engine = engine.lock()
                            .trace_internal_err("photo:face:compute:face_engine_lock:err", "获取人脸引擎锁失败")?;
                        engine.run(&img)
                            .trace_internal_err("photo:face:compute:face_engine_run:err", "人脸模型运行错误")?
                    };
                    let models = if faces.is_empty() {
                        Vec::new()
                    } else {
                        faces.into_iter()
                            .map(|f| NewFaceRecord::from_detected(photo_id, f))
                            .map(|f| face::ActiveModel::try_from(f))
                            .collect::<Result<Vec<face::ActiveModel>>>()
                            .trace_internal_err("db:photo:face:convert_err", "转换人脸记录失败")?
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
                    .trace_internal_err("db:photo:face:insert_many:err", "批量插入人脸记录失败")?;
            }

            download_handle.await.expect("download task panicked")?;
            detect_handle.await.expect("detect task panicked")?;

            total_faces += batch_faces;
            no_face_count += photo_count - batch_with_faces;

            info!(
                "第{}批完成 ({:?}), query={:?}, 照片数={}, 含人脸照片={}, 人脸数={}, 累计人脸={}, 无人脸={}",
                batch_idx, batch_start.elapsed(), query_time,
                photo_count, batch_with_faces, batch_faces,
                total_faces, no_face_count,
            );
        }

        let mode = if full { "全量" } else { "增量" };
        info!(
            "{}计算完成: 总耗时={:?}, backup={:?}, 清除={:?}, pipeline={:?}, 处理照片数={}, 总人脸数={}, 无人脸照片数={}",
            mode, overall_start.elapsed(), backup_time, cleanup_time, pipeline_start.elapsed(),
            no_face_count + total_faces, total_faces, no_face_count,
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
