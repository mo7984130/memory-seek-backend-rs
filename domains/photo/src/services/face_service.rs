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
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use tracing::{info, warn};

use crate::PhotoState;

pub(crate) struct FaceService;

// 创建
impl FaceService {
    pub async fn full_compute(state: &PhotoState, user_id: UserId) -> Result<()> {
        if user_id.0 != 1 {
            warn!("非管理员用户尝试全量计算人脸, id = {}", user_id);
            return Err(AppError::Forbidden("非管理员无法访问".into()));
        }

        info!("开始人脸全量计算, 计算前会清除face和person表, 谨慎运行");
        info!("开始保存face和person表");
        let backup_dir = std::env::temp_dir().join("memory-seek-full-compute");
        std::fs::create_dir_all(&backup_dir)?;
        let (face_path, face_hash) =
            CsvExporter::export(&state.db, face::TABLE_NAME, &backup_dir)
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
                .save(person::TABLE_NAME, &person_path, BackupType::Manual, &run_id)
                .await
                .trace_internal_err(
                    "photo:face:full_compute:save_person_storage:err",
                    "保存person表到备份存储失败",
                )?;
            info!(run_id = %run_id, "face和person表已保存到备份存储");
        }

        let _ = std::fs::remove_file(&face_path);
        let _ = std::fs::remove_file(&person_path);

        // 清除face和person库
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

        info!("开始分批计算人脸");
        let total = photo::Entity::find()
            .count(&state.db)
            .await
            .trace_internal_err(
                "photo:face:full_compute:find_photo_total:err",
                "人脸全量计算时获取照片数量错误",
            )?;
        let batch_size = 1024;
        let batch_num = total / batch_size + 1;
        info!(
            "共{}, batchsize: {}, 轮数: {}",
            total, batch_size, batch_num
        );

        let mut previous_id = -1;
        for i in 0..batch_num {
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

            if photos.is_empty() {
                break;
            }

            if let Some(last) = photos.last() {
                previous_id = last.0;
            }

            for (photo_id, file_id) in photos {
                let photo_id = PhotoId(photo_id);
                let img = state.s3_client.download(&file_id).await?;
                let img = image::load_from_memory(img.iter().as_slice())
                    .trace_internal_err(
                        "photo:face:full_compute:load_from_memory:err",
                        "从Bytes转换为image错误",
                    )?
                    .to_rgb8();

                let faces = {
                    let mut engine = state.face_engine.lock().trace_internal_err(
                        "photo:face:full_compute:face_engine_lock:err",
                        "获取人脸引擎锁失败",
                    )?;
                    engine.run(&img).trace_internal_err(
                        "photo:face:full_compute:face_engine_run:err",
                        "人脸模型运行错误",
                    )?
                };

                if faces.is_empty() {
                    continue;
                }

                let models: Vec<face::ActiveModel> = faces
                    .into_iter()
                    .map(|f| NewFaceRecord::from_detected(photo_id, f))
                    .map(|face| face::ActiveModel::try_from(face))
                    .collect::<Result<Vec<face::ActiveModel>>>()
                    .trace_internal_err("db:photo:face:convert_err", "转换人脸记录失败")?;

                face::Entity::insert_many(models)
                    .exec(&state.db)
                    .await
                    .trace_internal_err("db:photo:face:insert_many:err", "批量插入人脸记录失败")?;
            }
            info!("已完成{} / {}", i, batch_size);
        }

        Ok(())
    }
}

// 修改
impl FaceService {}

// 查询
impl FaceService {}

// 删除
impl FaceService {}
