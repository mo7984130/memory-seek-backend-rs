use bytes::Bytes;
use chrono::Utc;
use common::{
    error::AppError,
    ext::{
        BoolExt, CacheExtension, OkExt, ResultErrExt, ResultInspectErrAsync, ToErr, TraceExt,
        log_warn,
    },
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    timed,
    utils::{DbUtils, FileValidator, MetricsTimerExt},
};
use constants::RedisKeys;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
        comment_like_mapper::CommentLikeMapper, comment_mapper::CommentMapper,
        photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper,
        timeline_stat_mapper::TimelineStatMapper,
    },
    models::photo::{PhotoCursor, PhotoCursorParam, PhotoResult},
    state::PhotoState,
};
use common::Result;
use memory_seek_type::photo::models::{DeletePhotosParam, ExistsByMd5BatchParam, UploadPhotoParam};

use entities::{
    auth::user::UserId,
    photo::photo::{ActiveModel, PhotoId, PhotoRecord},
};

pub(crate) struct PhotoService;

// 查询
impl PhotoService {
    #[tracing::instrument(skip_all)]
    pub async fn load_photos_info(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: &Vec<PhotoId>,
    ) -> Result<Vec<PhotoResult>> {
        let (photos_result, liked_photo_ids_result) = tokio::join!(
            state.redis.get_or_load_batch(
                photo_ids,
                |id| RedisKeys::photo::photo::photo_info(*id),
                24 * 60 * 60,
                |miss_ids| async move { PhotoMapper::query_by_ids(&state.db, &miss_ids).await },
                |photo| photo.id,
            ),
            PhotoLikeMapper::query_is_like_by_photo_ids(&state.db, user_id, photo_ids)
        );
        let photos = photos_result?;
        let liked_photo_ids = liked_photo_ids_result?;
        photos
            .into_iter()
            .flatten()
            .map(|p| {
                let liked = liked_photo_ids.contains(&p.id);
                let file_id = p.file_id.clone();
                PhotoResult::from(p)
                    .with_liked(liked)
                    .with_tokens(&file_id, &state.token_cipher)
            })
            .collect::<Vec<_>>()
            .to_ok()
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_photo_cursor_page(
        state: &PhotoState,
        user_id: UserId,
        query: PhotoCursorParam,
    ) -> Result<CursorPage<PhotoResult, String>> {
        metrics_group!();
        let decoded_cursor = query.cursor.map(PhotoCursor::decode).transpose()?;

        // 获取photo_ids
        let photo_ids = PhotoMapper::query_cursor_page_ids(
            &state.db,
            decoded_cursor,
            query.size + 1,
            query.direction,
            query.anchor_time,
        )
        .timed(metrics_name!("find_cursor_page_ids"))
        .await?;
        if photo_ids.is_empty() {
            return Ok(CursorPage::empty());
        }

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, query.size);

        let photo_vos = Self::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        // 获取next_cursor
        let next_cursor = if has_more {
            match photo_vos.last() {
                Some(last_vo) => {
                    let id = last_vo
                        .id
                        .parse::<i64>()
                        .trace_internal_err("parse_photo_vo_id_err", "解析照片VOid错误")?;
                    Some(
                        PhotoCursor {
                            id: PhotoId(id),
                            created_at: last_vo.created_at,
                        }
                        .encode(),
                    )
                }
                None => None,
            }
        } else {
            None
        };

        metrics_success!();

        Ok(CursorPage {
            records: photo_vos,
            next_cursor,
            has_more,
        })
    }
}

impl PhotoService {
    #[instrument(skip_all, fields(user_id, file_name = %param.file_name))]
    pub async fn upload_photo(
        state: &PhotoState,
        user_id: UserId,
        file_data: Bytes,
        param: UploadPhotoParam,
    ) -> Result<PhotoResult> {
        metrics_group!();

        // 效验文件
        let metadata = {
            timed!("validate_photo", {
                FileValidator::validate_image(&file_data, &param.file_name, &param.content_type)?
            })
        };

        // 计算md5
        let md5_hash = {
            let file_data_clone = file_data.clone();
            timed!(
                "md5_hash",
                tokio::task::spawn_blocking(move || format!(
                    "{:x}",
                    md5::compute(&file_data_clone)
                ))
                .await?
            )
        };
        PhotoMapper::exists_by_md5(&state.db, &md5_hash)
            .await?
            .false_or_warn(
                "upload_photo:img_exist",
                "图片已存在",
                AppError::bad_request("图片已存在"),
            )?;

        // 上传文件
        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);
        state
            .s3_client
            .upload(&file_id, &file_data, &metadata.mime_type)
            .timed(metrics_name!("s3_upload"))
            .await?;

        // 更新数据库
        let now = Utc::now();
        let photo = ActiveModel {
            user_id: Set(user_id.0),
            name: Set(metadata.name),
            size: Set(file_data.len() as i64),
            width: Set(metadata.width as i32),
            height: Set(metadata.height as i32),
            mime_type: Set(metadata.mime_type),
            md5: Set(md5_hash),
            file_id: Set(file_id.clone()),
            created_at: Set(param.created_at.unwrap_or(now)),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&state.db)
        .timed(metrics_name!("db_insert"))
        .await
        .inspect_err_async(|_| async {
            let _ = state.s3_client.delete(&file_id).await.trace();
        })
        .await?;

        // 增加时间线统计
        // 错误不返回
        let _ = TimelineStatMapper::incr_stat(&state.db, photo.created_at).await;

        metrics_success!();

        let file_id = photo.file_id.clone();
        PhotoResult::from(PhotoRecord::from(photo))
            .with_tokens(&file_id, &state.token_cipher)
            .to_ok()
    }

    pub async fn exists_by_md5_batch(
        state: &PhotoState,
        param: ExistsByMd5BatchParam,
    ) -> Result<Vec<bool>> {
        metrics_group!("exists_by_md5_batch");

        let existing = PhotoMapper::exists_by_md5_batch(&state.db, &param.md5s).await?;
        let res = param
            .md5s
            .iter()
            .map(|md5| existing.contains(md5))
            .collect::<Vec<bool>>();

        metrics_success!("exists_by_md5_batch");
        Ok(res)
    }

    pub async fn delete_photos(
        state: &PhotoState,
        user_id: UserId,
        param: DeletePhotosParam,
    ) -> Result<()> {
        metrics_group!("delete_photos");

        let photo_ids: Vec<PhotoId> = param
            .photo_ids
            .into_iter()
            .filter_map(|id| id.parse::<i64>().ok().map(PhotoId))
            .collect();

        if photo_ids.is_empty() {
            return log_warn(
                "delete_photos_invalid_ids",
                "有效的照片ID为空",
                AppError::bad_request("没有有效的照片ID"),
            )
            .to_err();
        }

        // 数据库方面
        let photos = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 查询照片信息
                let photos = PhotoMapper::query_by_ids(txn, &photo_ids).await?;

                // 鉴权
                if photos.iter().any(|p| p.user_id != user_id) {
                    return log_warn(
                        "del_photos_not_belong",
                        "用户尝试删除不属于它的照片",
                        AppError::bad_request("无法删除不属于自己的照片"),
                    )
                    .to_err();
                }

                // 删除收藏夹照片
                let affected_collections =
                    CollectionPhotoMapper::delete_by_photo_ids(txn, &photo_ids).await?;

                // 更新收藏夹照片计数
                CollectionMapper::decr_photo_count_batch(txn, &affected_collections).await?;

                // 删除照片评论点赞
                let comment_ids = CommentMapper::delete_by_photo_ids(txn, &photo_ids).await?;
                CommentLikeMapper::delete_by_comment_ids(txn, &comment_ids).await?;

                // 删除照片点赞
                for photo_id in &photo_ids {
                    PhotoLikeMapper::delete_all_by_photo_id(txn, *photo_id).await?;
                }

                // 删除数据库照片
                PhotoMapper::delete_by_ids(txn, &photo_ids).await?;

                Ok(photos)
            })
        })
        .timed(metrics_name!("delete_photos", "db_transaction"))
        .await
        .trace_internal_err("db_txn_err", "数据库事务错误")?;

        // 删除照片文件
        let file_ids = photos.iter().map(|p| p.file_id.clone()).collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("delete_photos", "s3_delete_batch"))
            .await?;

        // 更新照片时间线统计
        TimelineStatMapper::decr_stat_by_created_ats(
            &state.db,
            &photos.iter().map(|p| p.created_at).collect::<Vec<_>>(),
        )
        .timed(metrics_name!("delete_photos", "timeline_decr_stat"))
        .await?;

        metrics_success!("delete_photos");

        Ok(())
    }
}
