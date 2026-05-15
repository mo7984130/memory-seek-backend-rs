#[cfg(feature = "face_recognition")]
use crate::mappers::FaceFeatureMapper;
use crate::mappers::{
    CollectionMapper, CollectionPhotoMapper, CommentLikeMapper, CommentMapper, PhotoMapper,
};
use crate::models::photo::{CursorPageVO, PhotoCursor, PhotoCursorQuery, PhotoVO};
use crate::photo::{PhotoInfo, TimeRange};
use crate::services::CollectionService;
#[cfg(feature = "face_recognition")]
use crate::services::feature_service::FeatureService;
use crate::services::timeline_stat_service::TimelineStatService;
use crate::state::PhotoState;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use common::constants::RedisKeys;
use common::error::AppError;
use common::models::UserId;
use common::utils::{CacheExtension, FileValidator, MetricsTimerExt, ResultExt};
use common::{metrics_group, metrics_success, metrics_timer_name, timed};
use const_format::formatcp;
use sea_orm::prelude::DateTimeUtc;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
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

/// S3 单次批量删除最大条数
const S3_BATCH_SIZE: usize = 1024;

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
                .trace_internal_err(
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
                .trace_bad_request_err("photo::upload:invaild_photo", "图片效验不通过"))
                .await
                .trace_internal_err(
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
            .trace_internal_err("photo::upload:s3_upload_err", "s3上传失败")?;

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
            created_at: Set(created_at.unwrap_or(now).into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&state.db)
        .timed(metrics_timer_name!("upload_photo", "db_insert"))
        .await
        .trace_internal_err("photo::upload:db_insert_err", "保存照片失败");
        // 删除文件
        let photo = match insert_result {
            Ok(photo) => photo,
            Err(e) => {
                let _ = state
                    .s3_client
                    .delete(&file_id)
                    .await
                    .trace_internal_err("photo::upload:s3_delete_err", "删除文件失败");
                return Err(e);
            }
        };

        // 增加时间线统计
        // 错误不返回
        let _ = TimelineStatService::incr_stat(state, photo.created_at)
            .await
            .trace_internal_err("photo::upload:incr_timeline_err", "增加时间线统计错误");

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
            .trace_internal_err("photo::upload:send_face_task_err", "发送人脸识别任务错误");

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

        let size =
            usize::try_from(query.size).trace_bad_request_err("invalid_size", "size 必须为正数")?;
        if size > DB_BATCH_SIZE {
            warn!(user_id=%user_id, "size超过最大值");
            return Err(AppError::bad_request(HIT_MAX_SIZE_MSG));
        }

        let decoded_cursor = query.cursor.map(|s| PhotoCursor::decode(s)).transpose()?;

        // 获取photo_ids
        let mut photo_ids = PhotoMapper::find_cursor_page_ids(
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
                            Ok(PhotoMapper::find_by_ids(&state.db, &miss_ids).await?)
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

        let res = PhotoMapper::find_by_id(&state.db, photo_id)
            .await
            .map(PhotoInfo::from);

        metrics_success!("get_photo_info_by_id");
        return res;
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
        return Ok(res);
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

        let res = PhotoMapper::find_time_range(&state.db).await;

        metrics_success!("get_time_range");
        return res;
    }

    /// 批量删除照片
    ///
    /// 执行步骤：
    /// 1. 鉴权（仅管理员）
    /// 2. 查询所有照片元数据（file_id、created_at）
    /// 3. [face_recognition] 查询并分批删除人脸特征，收集涉及的 person_id
    /// 4. 分批事务：删除收藏关联 → 更新收藏计数 → 删除评论点赞 → 删除评论 → 删除照片记录
    /// 5. [face_recognition] 并发重新计算受影响人物的统计
    /// 6. 分批删除 S3 文件
    /// 7. 批量递减时间线统计
    ///
    /// # 注意
    /// - 数据库操作按 `DB_BATCH_SIZE` 分批，避免单次事务过大
    /// - S3 删除按 `S3_BATCH_SIZE` 分批（AWS 硬限 1000）
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
        let photos = PhotoMapper::find_by_ids(&state.db, &photo_ids)
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

        Ok(())
    }

    // /// 删除照片
    // ///
    // /// 执行以下步骤：
    // /// 1. 验证权限（仅管理员可删除）
    // /// 2. 获取照片的所有人脸特征，逐个删除（减量计算更新人物统计）（仅face_recognition feature）
    // /// 3. 删除照片的所有收藏关联，更新收藏夹照片数量
    // /// 4. 删除照片的所有评论及其点赞记录
    // /// 5. 删除照片记录
    // /// 6. 删除 OSS 文件
    // /// 7. 更新时间线统计
    // ///
    // /// 使用事务保证数据库操作的原子性
    // ///
    // /// # 参数
    // /// - `db`: 数据库连接
    // /// - `redis`: Redis连接池
    // /// - `s3`: OSS客户端
    // /// - `user_id`: 用户ID
    // /// - `photo_id`: 照片ID
    // ///
    // /// # 返回
    // /// 成功返回空元组
    // ///
    // /// # 错误
    // /// - 非管理员返回403错误
    // /// - 照片不存在返回404错误
    // pub async fn delete_photo(
    //     state: &PhotoState,
    //     user_id: i64,
    //     photo_id: i64,
    // ) -> Result<(), AppError> {
    //     metrics_group!("delete_photo");

    //     if user_id != 1 {
    //         return Err(AppError::forbidden("只有管理员可以删除照片"));
    //     }

    //     let photo = PhotoMapper::find_by_id(&state.db, photo_id)
    //         .timed(metrics_timer_name!("delete_photo", "find_photo"))
    //         .await?;

    //     #[cfg(feature = "face_recognition")]
    //     {
    //         let features = FaceFeatureMapper::find_by_photo_id(&state.db, photo_id)
    //             .timed(metrics_timer_name!("delete_photo", "find_features"))
    //             .await?;
    //         let person_ids: std::collections::HashSet<i64> =
    //             features.iter().filter_map(|f| f.person_id).collect();
    //         state
    //             .db
    //             .transaction::<_, (), AppError>(|txn| {
    //                 Box::pin(async move {
    //                     let feature_ids: Vec<i64> = features.iter().map(|f| f.id).collect();
    //                     if !feature_ids.is_empty() {
    //                         FaceFeatureMapper::delete_by_ids(txn, feature_ids)
    //                             .timed(metrics_timer_name!("delete_photo", "delete_features"))
    //                             .await?;
    //                     }

    //                     Self::delete_photo_transaction_body(txn, photo_id).await
    //                 })
    //             })
    //             .await
    //             .trace_internal_err("photo::delete_photo:db_err", "数据库出错")?;
    //     }

    //     #[cfg(not(feature = "face_recognition"))]
    //     {
    //         state
    //             .db
    //             .transaction::<_, (), AppError>(|txn| {
    //                 Box::pin(
    //                     async move { Self::delete_photo_transaction_body(txn, photo_id).await },
    //                 )
    //             })
    //             .await
    //             .trace_internal_err("photo::delete_photo:db_err", "数据库出错")?;
    //     }

    //     #[cfg(feature = "face_recognition")]
    //     {
    //         let futs = person_ids
    //             .iter()
    //             .map(|&pid| FeatureService::recalculate_person_stats(state, pid));
    //         let _ = futures::future::join_all(futs).await;
    //     }

    //     let _ = state
    //         .s3_client
    //         .delete(&photo.file_id)
    //         .timed(metrics_timer_name!("delete_photo", "s3_delete"))
    //         .await
    //         .trace_internal_err("photo::delete_photo:s3_delete_err", "删除图片文件失败");

    //     let _ = TimelineStatService::decr_stat(state, photo.created_at)
    //         .timed(metrics_timer_name!("delete_photo", "decr_timeline"))
    //         .await
    //         .trace_internal_err("photo::delete_photo:decr_timeline_err", "减量时间线错误");

    //     Ok(())
    // }

    // /// 删除图片的事务具体执行逻辑
    // ///
    // /// 按照数据库外键约束和业务逻辑顺序，依次清理关联数据：
    // /// 1. 移除收藏关系并更新收藏夹图片计数
    // /// 2. 清理评论相关的点赞数据
    // /// 3. 删除图片下的所有评论
    // /// 4. 最后删除图片元数据
    // async fn delete_photo_transaction_body(
    //     txn: &sea_orm::DatabaseTransaction,
    //     photo_id: i64,
    // ) -> Result<(), AppError> {
    //     // --- 1. 处理收藏夹关联 ---
    //     // 从 collection_photo 表中删除该图片的所有收藏记录，并返回受影响的收藏夹 ID 列表
    //     let collection_ids = CollectionPhotoMapper::delete_by_photo_id(txn, photo_id)
    //         .timed(metrics_timer_name!("delete_photo", "collection_photo"))
    //         .await
    //         .trace_internal_err(
    //             "photo::delete_photo:delete_collection_photo",
    //             "删除收藏夹关联失败",
    //         )?;

    //     // 针对每一个包含该图片的收藏夹，将其图片总数减 1
    //     if !collection_ids.is_empty() {
    //         CollectionMapper::increment_photo_counts(txn, collection_ids, -1)
    //             .timed(metrics_timer_name!("delete_photo", "decrement_photo_count"))
    //             .await
    //             .trace_internal_err(
    //                 "photo::delete_photo:decrement_photo_count",
    //                 "更新收藏夹图片计数失败",
    //             )?;
    //     }

    //     // --- 2. 处理评论及评论点赞 ---
    //     // 首先找出该图片下的所有评论 ID（为了后续删除这些评论收到的点赞）
    //     let comment_ids = CommentMapper::find_ids_by_photo_id(txn, photo_id)
    //         .timed(metrics_timer_name!("delete_photo", "find_comment_ids"))
    //         .await
    //         .trace_internal_err("photo::delete_photo:find_comment_ids_err", "查找评论失败")?;

    //     // 如果该图片有评论，则先删除这些评论对应的所有点赞记录（清理从表）
    //     if !comment_ids.is_empty() {
    //         CommentLikeMapper::delete_by_comment_ids(txn, comment_ids)
    //             .timed(metrics_timer_name!("delete_photo", "delete_comment_like"))
    //             .await
    //             .trace_internal_err(
    //                 "photo::delete_photo:delete_comment_like",
    //                 "删除评论点赞失败",
    //             )?;
    //     }

    //     // 删除该图片下的所有评论主体
    //     CommentMapper::delete_by_photo_id(txn, photo_id)
    //         .timed(metrics_timer_name!("delete_photo", "delete_comment"))
    //         .await
    //         .trace_internal_err("photo::delete_photo:delete_comment", "删除评论失败")?;

    //     // --- 3. 处理图片主体 ---
    //     // 最后一步：删除图片主表记录
    //     PhotoMapper::delete_by_id(txn, photo_id)
    //         .timed(metrics_timer_name!("delete_photo", "delete_photo"))
    //         .await
    //         .trace_internal_err("photo::delete_photo:delete_photo", "删除照片失败")?;

    //     Ok(())
    // }
}
