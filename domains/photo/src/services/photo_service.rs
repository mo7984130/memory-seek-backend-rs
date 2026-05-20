use crate::mappers::{
    CollectionPhotoMapper, CommentLikeMapper, CommentMapper, PhotoMapper, TimelineStatMapper,
};
use crate::models::photo::{CursorPageVO, PhotoCursor, PhotoCursorQuery, PhotoVO};
use crate::photo::{PhotoInfo, TimeRange};
use crate::services::CollectionService;
use crate::services::timeline_stat_service::TimelineStatService;
use crate::state::PhotoState;
use bytes::Bytes;
use chrono::Utc;
use common::constants::RedisKeys;
use common::error::AppError;
use common::ext::{CacheExtension, ResultErrExt};
use common::models::UserId;
use common::utils::{FileValidator, MetricsTimerExt};
use common::{metrics_group, metrics_success, metrics_timer_name, timed};
use const_format::formatcp;
use entities::*;
use sea_orm::prelude::*;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use sea_orm::{ColumnTrait, EntityTrait};
use serde::Deserialize;
use std::collections::HashSet;
use tracing::{instrument, warn};
use uuid::Uuid;

/// 人脸检测任务，包含照片ID、图片字节数据和元数据
#[cfg(feature = "face_recognition")]
pub struct FaceTask {
    pub photo_id: i64,
    pub image_bytes: Bytes,
    pub img_width: u32,
    pub img_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageDirection {
    Next,
    Prev,
}

/// 数据库 IN 子句单批最大条数
const DB_BATCH_SIZE: usize = 512;

const HIT_MAX_SIZE_MSG: &str = formatcp!("查询参数长度不可以超过 {}", DB_BATCH_SIZE);

pub struct PhotoService;

impl PhotoService {
    /// 上传照片
    ///
    /// 执行以下步骤：
    /// 1. 计算MD5检查是否重复
    /// 2. 验证图片格式和尺寸
    /// 3. 上传到OSS存储
    /// 4. 保存照片记录到数据库
    /// 5. 更新时间线统计
    /// 6. 发送人脸检测任务（如果启用了face_recognition feature）
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `_redis`: Redis连接池（暂未使用）
    /// - `s3`: OSS客户端
    /// - `face_tx`: 人脸检测任务发送通道（仅face_recognition feature）
    /// - `user_id`: 用户ID
    /// - `file_data`: 文件字节数据
    /// - `file_name`: 原始文件名
    /// - `content_type`: 文件MIME类型
    /// - `created_at`: 自定义创建时间（可选）
    /// - `token_cipher`: 加密密钥
    ///
    /// # 返回
    /// 返回上传成功的照片VO
    #[instrument(name = "photo_upload", skip_all, fields(user_id, file_name))]
    pub async fn upload_photo(
        state: &PhotoState,
        user_id: i64,
        file_data: Bytes,
        file_name: String,
        content_type: String,
        created_at: Option<DateTimeUtc>,
    ) -> Result<PhotoVO, AppError> {
        metrics_group!("upload_photo");

        // 计算md5
        let md5_hash = {
            let file_data_clone = file_data.clone();
            timed!(
                "upload_photo",
                "md5_hash",
                tokio::task::spawn_blocking(move || format!(
                    "{:x}",
                    md5::compute(&file_data_clone)
                ))
                .await
                .trace_to_internal_err(
                    "spawn_blocking_md5_compute_err",
                    "tokio spawn_blocking join err"
                )?
            )
        };
        let existing_md5s = PhotoMapper::exists_by_md5_batch(&state.db, &[&md5_hash]).await?;
        if existing_md5s.contains(&md5_hash) {
            warn!(md5=%md5_hash, "图片已存在");
            return Err(AppError::bad_request("图片已存在"));
        }

        // 效验文件
        let metadata = {
            let file_data_clone = file_data.clone();
            timed!(
                "upload_photo",
                "validate_photo",
                tokio::task::spawn_blocking(move || FileValidator::validate_image(
                    &file_data_clone,
                    &file_name,
                    &content_type
                )
                .trace_to_bad_request_warn("photo::upload:invaild_photo", "图片效验不通过"))
                .await
                .trace_to_internal_err(
                    "spawn_blocking_validate_photo_err",
                    "tokio spawn_blocking join err"
                )??
            )
        };

        // 上传文件
        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);
        state
            .s3_client
            .upload(&file_id, &file_data, &metadata.mime_type)
            .timed(metrics_timer_name!("upload_photo", "s3_upload"))
            .await
            .trace_to_internal_err("photo::upload:s3_upload_err", "s3上传失败")?;

        // 更新数据库
        let now = Utc::now();
        let insert_result = entities::photo::ActiveModel {
            user_id: Set(user_id),
            name: Set(metadata.name),
            size: Set(file_data.len() as i64),
            width: Set(metadata.width as i32),
            height: Set(metadata.height as i32),
            mime_type: Set(metadata.mime_type),
            md5: Set(md5_hash),
            file_id: Set(file_id.clone()),
            created_at: Set(created_at.unwrap_or(now)),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&state.db)
        .timed(metrics_timer_name!("upload_photo", "db_insert"))
        .await
        .trace_to_internal_err("photo::upload:db_insert_err", "保存照片失败");
        // 删除文件
        let photo = match insert_result {
            Ok(photo) => photo,
            Err(e) => {
                let _ = state
                    .s3_client
                    .delete(&file_id)
                    .await
                    .trace_to_internal_err("photo::upload:s3_delete_err", "删除文件失败");
                return Err(e);
            }
        };

        // 增加时间线统计
        // 错误不返回
        let _ = TimelineStatService::incr_stat(state, photo.created_at)
            .await
            .trace_to_internal_err("photo::upload:incr_timeline_err", "增加时间线统计错误");

        // 发送人脸识别任务
        // 错误不返回
        #[cfg(feature = "face_recognition")]
        let _ = state
            .face_tx
            .send(FaceTask {
                photo_id: photo.id,
                image_bytes: file_data,
                img_width: metadata.width,
                img_height: metadata.height,
            })
            .await
            .trace_to_internal_err("photo::upload:send_face_task_err", "发送人脸识别任务错误");

        // 生成token
        let (thumbnail_token, preview_token, original_token) =
            PhotoVO::generate_tokens(&file_id, &state.token_cipher);

        metrics_success!("upload_photo");

        Ok(PhotoVO {
            id: photo.id.to_string(),
            name: photo.name,
            width: photo.width,
            height: photo.height,
            size: photo.size,
            created_at: photo.created_at,
            is_favorited: None,
            is_collected: None,
            thumbnail_token,
            preview_token,
            original_token,
        })
    }

    /// 游标分页查询照片列表
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `redis`: Redis连接池
    /// - `user_id`: 用户ID
    /// - `query`: 分页查询参数
    /// - `token_cipher`: 加密密钥
    ///
    /// # 返回
    /// 返回分页照片列表，包含是否有更多数据的标识
    pub async fn get_photo_cursor_page(
        state: &PhotoState,
        user_id: i64,
        query: PhotoCursorQuery,
    ) -> Result<CursorPageVO<PhotoVO, String>, AppError> {
        metrics_group!("get_photo_cursor_page");

        let size = usize::try_from(query.size)
            .trace_to_bad_request_warn("invalid_size", "size 必须为正数")?;
        if size > DB_BATCH_SIZE {
            warn!(user_id=%user_id, "size超过最大值");
            return Err(AppError::bad_request(HIT_MAX_SIZE_MSG));
        }

        let decoded_cursor = query.cursor.map(PhotoCursor::decode).transpose()?;

        // 获取photo_ids
        let mut photo_ids = PhotoMapper::query_cursor_page_ids(
            &state.db,
            decoded_cursor,
            (size + 1) as u64,
            query.direction,
        )
        .timed(metrics_timer_name!(
            "get_photo_cursor_page",
            "find_cursor_page_ids"
        ))
        .await?;
        // 空返回
        if photo_ids.is_empty() {
            return Ok(CursorPageVO::empty());
        }

        // 判断has_more, 截断多余的
        let has_more = photo_ids.len() > size;
        if has_more {
            photo_ids.truncate(size);
        }

        // 获取喜欢收藏夹的id
        let favorite_collection_id = CollectionService::get_favorite_collection_id(state, user_id)
            .timed(metrics_timer_name!(
                "get_photo_cursor_page",
                "get_favorite_collection_id"
            ))
            .await?;

        // 带redis缓存的获取照片信息
        // 获取照片是否被喜欢
        let (photos_result, favorited_photo_ids_result) = tokio::join!(
                state
                    .redis
                    .get_or_load_batch(
                        &photo_ids,
                        |id| RedisKeys::photo::photo_info(*id),
                        24 * 60 * 60,
                        |miss_ids| async move {
                            PhotoMapper::query_by_ids(&state.db, &miss_ids).await
                        },
                        |photo| photo.id,
                    )
                    .timed(metrics_timer_name!("get_photo_cursor_page", "get_photos")),
                CollectionPhotoMapper::exists_in_collection(
                    &state.db,
                    favorite_collection_id,
                    &photo_ids
                )
                .timed(metrics_timer_name!(
                    "get_photo_cursor_page",
                    "exists_in_collection"
                ))
            );
        let photos = photos_result?;
        let favorited_photo_ids = favorited_photo_ids_result?
            .into_iter()
            .collect::<HashSet<i64>>();

        // 获取next_cursor
        let next_cursor = if has_more {
            photos.get(size - 1).and_then(|opt| opt.as_ref()).map(|p| {
                PhotoCursor {
                    id: p.id,
                    created_at: p.created_at,
                }
                .encode()
            })
        } else {
            None
        };
        // 组装records
        let records: Vec<PhotoVO> = timed!(
            "get_photo_cursor_page",
            "records",
            photos
                .into_iter()
                .flatten()
                .map(|p| {
                    let (thumbnail_token, preview_token, original_token) =
                        PhotoVO::generate_tokens(&p.file_id, &state.token_cipher);

                    PhotoVO {
                        id: p.id.to_string(),
                        name: p.name,
                        width: p.width,
                        height: p.height,
                        size: p.size,
                        created_at: p.created_at,
                        is_favorited: Some(favorited_photo_ids.contains(&p.id)),
                        is_collected: None,
                        thumbnail_token,
                        preview_token,
                        original_token,
                    }
                })
                .collect()
        );

        metrics_success!("get_photo_cursor_page");

        Ok(CursorPageVO {
            records,
            next_cursor,
            has_more,
        })
    }

    /// 根据ID获取照片详情
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `photo_id`: 照片ID
    ///
    /// # 返回
    /// 返回照片模型，不存在返回404错误
    pub async fn get_photo_info_by_id(
        state: &PhotoState,
        photo_id: i64,
    ) -> Result<PhotoInfo, AppError> {
        metrics_group!("get_photo_info_by_id");

        let res = PhotoMapper::query_by_id(&state.db, photo_id)
            .await
            .map(PhotoInfo::from);

        metrics_success!("get_photo_info_by_id");
        res
    }

    pub async fn exists_by_md5_batch(
        state: &PhotoState,
        user_id: UserId,
        md5s: &[String],
    ) -> Result<Vec<bool>, AppError> {
        metrics_group!("exists_by_md5_batch");

        if md5s.len() > DB_BATCH_SIZE {
            warn!(user_id=%user_id.0, "md5s 数量超过最大值，当前为 {} 个", md5s.len());
            return Err(AppError::bad_request(HIT_MAX_SIZE_MSG));
        }

        let existing = PhotoMapper::exists_by_md5_batch(&state.db, md5s).await?;
        let res = md5s.iter().map(|md5| existing.contains(md5)).collect();

        metrics_success!("exists_by_md5_batch");
        Ok(res)
    }

    /// 获取所有照片的时间范围
    ///
    /// # 参数
    /// - `db`: 数据库连接
    ///
    /// # 返回
    /// 返回最早和最晚照片的创建时间元组
    pub async fn get_time_range(state: &PhotoState) -> Result<TimeRange, AppError> {
        metrics_group!("get_time_range");

        let res = PhotoMapper::query_time_range(&state.db).await;

        metrics_success!("get_time_range");
        res
    }

    /// 批量删除照片
    ///
    /// 执行步骤：
    /// 1. 鉴权（仅管理员）
    /// 2. 查询所有照片元数据（file_id、created_at）
    /// 3. [face_recognition] 查询并分批删除人脸特征，收集涉及的 person_id
    /// 4. 分批事务：删除评论点赞 → 删除评论 → 删除照片记录
    /// 5. [face_recognition] 并发重新计算受影响人物的统计
    /// 6. 分批删除 S3 文件
    /// 7. 批量递减时间线统计
    ///
    /// # 注意
    /// - S3 / 时间线 / 人脸统计失败仅记录警告，不回滚已提交的数据库事务；
    pub async fn delete_photos(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: Vec<i64>,
    ) -> Result<(), AppError> {
        metrics_group!("delete_photos");

        // 鉴权
        if user_id.0 != 1 {
            return Err(AppError::forbidden("只有管理员可以删除图片"));
        }

        if photo_ids.is_empty() {
            return Ok(());
        }

        // 查询照片数据
        let photos = PhotoMapper::query_by_ids(&state.db, &photo_ids)
            .timed(metrics_timer_name!("delete_photos", "find_photos"))
            .await?;

        // 处理缺失的ID
        if photos.len() != photo_ids.len() {
            let found_ids: HashSet<i64> = photos.iter().map(|p| p.id).collect();
            let missing_ids: Vec<i64> = photo_ids
                .iter()
                .filter(|id| !found_ids.contains(id))
                .copied()
                .collect();
            return Err(AppError::NotFound(
                format!("以下照片不存在: {:?}", missing_ids).into(),
            ));
        }

        // [face_recognition] 查询并收集人脸特征
        #[cfg(feature = "face_recognition")]
        let features = {
            use sea_orm::QuerySelect;

            face_feature::Entity::find()
                .select_only()
                .column(face_feature::Column::Id)
                .column(face_feature::Column::PersonId)
                .column(face_feature::Column::Embedding)
                .column(face_feature::Column::Score)
                .filter(face_feature::Column::PhotoId.is_in(photo_ids.iter().copied()))
                .into_tuple::<(i64, Option<i64>, Embedding512, f32)>()
                .all(&state.db)
                .timed(metrics_timer_name!("delete_photos", "find_features"))
                .await
                .trace_to_internal_err("db_query_err", "获取人脸特征错误")?
                .into_iter()
                .map(|(id, pid, emb, score)| (id, pid, emb.to_vec(), score))
                .collect::<Vec<_>>()
        };

        // 执行数据库层面的删除
        state
            .db
            .transaction::<_, (), AppError>(|txn| {
                Box::pin(async move {
                    // [face_recognition]
                    // 人脸特征减量计算 -> 删除人脸特征
                    #[cfg(feature = "face_recognition")]
                    {
                        use crate::mappers::{FaceFeatureMapper, FacePersonMapper};

                        // 删除人脸特征
                        FaceFeatureMapper::delete_by_ids(
                            txn,
                            features.iter().map(|f| f.0).collect(),
                        )
                        .timed(metrics_timer_name!("delete_photos", "delete_features"))
                        .await
                        .trace_to_internal_err("delete_features", "删除人脸特征失败")?;

                        // 减量计算人物
                        FacePersonMapper::decr_by_features(txn, &features).await?;
                    }

                    // 获取相关的评论id
                    let comment_ids = CommentMapper::query_ids_by_photo_ids(txn, &photo_ids)
                        .timed(metrics_timer_name!("delete_photos", "find_comment_ids"))
                        .await?;

                    // 删除评论点赞
                    CommentLikeMapper::delete_by_comment_ids(txn, &comment_ids)
                        .timed(metrics_timer_name!("delete_photos", "delete_comment_like"))
                        .await?;

                    // 删除评论
                    CommentMapper::delete_by_ids(txn, &comment_ids)
                        .timed(metrics_timer_name!("delete_photos", "delete_comment"))
                        .await
                        .trace_to_internal_err("delete_comment", "删除评论失败")?;

                    // 删除照片
                    PhotoMapper::delete_by_ids(txn, &photo_ids)
                        .timed(metrics_timer_name!("delete_photos", "delete_photo"))
                        .await
                        .trace_to_internal_err("delete_photo", "删除照片记录失败")?;

                    Ok(())
                })
            })
            .await
            .trace_to_internal_err("db_err", "数据库批量删除出错")?;

        // 删除照片文件
        state
            .s3_client
            .delete_batch(photos.iter().map(|p| p.file_id.clone()).collect())
            .await?;

        // 照片时间线统计删减
        // 报错不返回
        let _ = TimelineStatMapper::decr_stat_by_created_ats(
            &state.db,
            photos.iter().map(|p| p.created_at).collect(),
        )
        .await;

        Ok(())
    }
}
