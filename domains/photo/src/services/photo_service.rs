use bytes::Bytes;
use chrono::Utc;
use common::{
    error::AppError,
    ext::{CacheExtension, OkExt, ResultErrExt, ToErr, log_warn},
    metrics_group, metrics_success, metrics_timer_name,
    models::CursorPage,
    timed,
    utils::{DbUtils, FileValidator, MetricsTimerExt},
};
use constants::RedisKeys;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, entity::prelude::DateTimeUtc};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    mappers::{
        collection_mapper::CollectionMapper, collection_photo_mapper::CollectionPhotoMapper,
        photo_mapper::PhotoMapper, timeline_stat_mapper::TimelineStatMapper,
    },
    models::photo::{PhotoCursor, PhotoCursorQuery, PhotoVO},
    services::collection_service::CollectionService,
    state::PhotoState,
};
use common::Result;

use entities::{
    auth::user::UserId,
    photo::photo::{PhotoId, PhotoRecord, ActiveModel},
};

pub(crate) struct PhotoService;

// 查询
impl PhotoService {
    pub async fn load_photos_info(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<PhotoVO>> {
        let favorite_collection_id =
            CollectionService::get_favorite_collection_id(state, user_id).await?;
        let (photos_result, favorited_photo_ids_result) = tokio::join!(
            state.redis.get_or_load_batch(
                &photo_ids,
                |id| RedisKeys::photo::photo::photo_info(*id),
                24 * 60 * 60,
                |miss_ids| async move { PhotoMapper::query_by_ids(&state.db, &miss_ids).await },
                |photo| photo.id,
            ),
            CollectionPhotoMapper::exists_in_collection(
                &state.db,
                favorite_collection_id,
                &photo_ids
            )
        );
        let photos = photos_result?;
        let favorited_photo_ids = favorited_photo_ids_result?;
        photos
            .into_iter()
            .flatten()
            .map(|p| {
                PhotoVO::from(p.clone())
                    .with_favorited(favorited_photo_ids.contains(&p.id))
                    .with_tokens(&state.token_cipher)
            })
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn get_photo_cursor_page(
        state: &PhotoState,
        user_id: UserId,
        query: PhotoCursorQuery,
    ) -> Result<CursorPage<PhotoVO, String>> {
        metrics_group!("get_photo_cursor_page");

        let size = query.size;
        if size > Self::MD5_MAX_SIZE {
            let err = log_warn(
                "over_max_size",
                "size超过最大值",
                "",
                AppError::bad_request("size超过最大值"),
            );
            return Err(err);
        }

        let decoded_cursor = query.cursor.map(PhotoCursor::decode).transpose()?;

        // 获取photo_ids
        let photo_ids = PhotoMapper::query_cursor_page_ids(
            &state.db,
            decoded_cursor,
            size + 1,
            query.direction,
        )
        .timed(metrics_timer_name!(
            "get_photo_cursor_page",
            "find_cursor_page_ids"
        ))
        .await?;
        if photo_ids.is_empty() {
            return Ok(CursorPage::empty());
        }

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = Self::load_photos_info(state, user_id, &photo_ids).await?;

        // 获取next_cursor
        let next_cursor = if has_more {
            photo_vos.last().and_then(|vo| {
                // 解析错误概率很小, 不返回错误, 而是返回空CursorPage
                let id = vo
                    .id
                    .parse::<i64>()
                    .trace_internal_err("parse_photo_vo_id_err", "解析照片VOid错误");
                match id {
                    Ok(id) => Some(
                        PhotoCursor {
                            id: PhotoId(id),
                            created_at: vo.created_at,
                        }
                        .encode(),
                    ),
                    Err(_) => None,
                }
            })
        } else {
            None
        };

        metrics_success!("get_photo_cursor_page");

        Ok(CursorPage {
            records: photo_vos,
            next_cursor,
            has_more,
        })
    }
}

impl PhotoService {
    #[instrument(name = "photo_upload", skip_all, fields(user_id, file_name))]
    pub async fn upload_photo(
        state: &PhotoState,
        user_id: UserId,
        file_data: Bytes,
        file_name: String,
        content_type: String,
        created_at: Option<DateTimeUtc>,
    ) -> Result<PhotoVO> {
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
            let err = log_warn(
                "upload_photo:img_exist",
                "图片已存在",
                md5_hash,
                AppError::bad_request("图片已存在"),
            );
            return Err(err);
        }

        // 效验文件
        let metadata = {
            let file_data_clone = file_data.clone();
            timed!("upload_photo", "validate_photo", {
                tokio::task::spawn_blocking(move || {
                    FileValidator::validate_image(&file_data_clone, &file_name, &content_type)
                })
                .await
                .trace_internal_err(
                    "spawn_blocking_validate_photo_err",
                    "tokio spawn_blocking join err",
                )?
                .map_err(|e| {
                    let msg = e.to_string();
                    log_warn(
                        "upload_photo:invalid_photo",
                        "图片效验不通过",
                        &msg,
                        AppError::bad_request("图片效验不通过"),
                    )
                })?
            })
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
        let insert_result = ActiveModel {
            user_id: Set(user_id.0),
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
        .trace_internal_err("photo::upload:db_insert_err", "保存照片失败");

        //更新数据库失败的话 删除文件
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
        let _ = TimelineStatMapper::incr_stat(&state.db, photo.created_at).await;

        metrics_success!("upload_photo");

        PhotoVO::from(PhotoRecord::from(photo))
            .with_tokens(&state.token_cipher)
            .to_ok()
    }

    const MD5_MAX_SIZE: u64 = 1024;

    pub async fn exists_by_md5_batch(state: &PhotoState, md5s: &[String]) -> Result<Vec<bool>> {
        metrics_group!("exists_by_md5_batch");

        if md5s.len() > Self::MD5_MAX_SIZE as usize {
            return log_warn(
                "over_md5_size",
                "md5的数量超过最大值",
                "",
                AppError::bad_request("md5的数量超过最大值"),
            )
            .to_err();
        }

        let existing = PhotoMapper::exists_by_md5_batch(&state.db, md5s).await?;
        let res = md5s
            .iter()
            .map(|md5| existing.contains(md5))
            .collect::<Vec<bool>>();

        metrics_success!("exists_by_md5_batch");
        Ok(res)
    }

    pub async fn delete_photos(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: Vec<PhotoId>,
    ) -> Result<()> {
        metrics_group!("delete_photos");

        // 鉴权
        if user_id.0 != 0 {
            return log_warn(
                "delete_photos_not_admin",
                "用户想要删除照片, 不是管理员",
                "",
                AppError::forbidden("非管理员用户无法删除照片"),
            )
            .to_err();
        }

        if photo_ids.is_empty() {
            return Ok(());
        }

        // 数据库方面
        let photos = DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 查询照片信息
                let photos = PhotoMapper::query_by_ids(txn, &photo_ids).await?;

                // 删除收藏夹照片
                let affected_collections =
                    CollectionPhotoMapper::delete_by_photo_ids(txn, &photo_ids).await?;

                // 更新收藏夹照片计数
                CollectionMapper::decr_photo_count_batch(txn, &affected_collections).await?;

                // 删除数据库照片
                PhotoMapper::delete_by_ids(txn, &photo_ids).await?;

                Ok(photos)
            })
        })
        .await
        .trace_internal_err("db_txn_err", "数据库事务错误")?;

        // 删除照片文件
        let file_ids = photos.iter().map(|p| p.file_id.clone()).collect::<Vec<_>>();
        state.s3_client.delete_batch(file_ids).await?;

        // 更新照片时间线统计
        TimelineStatMapper::decr_stat_by_created_ats(
            &state.db,
            &photos.iter().map(|p| p.created_at).collect::<Vec<_>>(),
        )
        .await?;

        Ok(())
    }
}
