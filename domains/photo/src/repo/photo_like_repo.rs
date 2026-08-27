use audit::{AuditEvent, AuditRecorder};
use common::{
    db_transaction,
    error::{AppError, ContextualError, contextual::Result},
    models::CursorPage,
    time::DateTime,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{models::LikedPhotosQuery, photo::PhotoId},
};

use crate::state::PhotoState;
use crate::{
    mappers::{photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper},
    repo::PhotoRepo,
};

pub(crate) struct PhotoLikeRepo;

impl PhotoLikeRepo {
    /// 点赞照片.
    pub(crate) async fn like(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            // 确认存在
            PhotoMapper::ensure_exist(txn, photo_id).await?;
            // 插入
            if !PhotoLikeMapper::insert(txn, user_id, photo_id).await? {
                return Err(ContextualError::error_without_source(
                    "photo_already_liked",
                    "照片已经点赞过",
                    AppError::bad_request("已经点赞过"),
                ));
            }
            // 更新计数
            PhotoMapper::update_like_count_delta(txn, photo_id, 1).await?;

            AuditRecorder::append(
                txn,
                AuditEvent::new("like")
                    .with_actor(user_id.0)
                    .with_target("photo", photo_id.0),
            )
            .await?;
            Ok(())
        })
        .await?;

        PhotoRepo::cache_photo_like_status(state, user_id, photo_id, true).await;
        PhotoRepo::invalidate_photo_info(state, photo_id).await;

        Ok(())
    }

    /// 取消点赞
    pub(crate) async fn unlike(
        state: &PhotoState,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<()> {
        db_transaction!(scoped & state.db, |txn| {
            if !PhotoLikeMapper::delete(txn, user_id, photo_id).await? {
                return Err(ContextualError::error_without_source(
                    "photo_not_liked",
                    "照片尚未点赞",
                    AppError::bad_request("还未点赞"),
                ));
            }
            PhotoMapper::update_like_count_delta(txn, photo_id, -1).await?;
            AuditRecorder::append(
                txn,
                AuditEvent::new("unlike")
                    .with_actor(user_id.0)
                    .with_target("photo", photo_id.0),
            )
            .await?;
            Ok(())
        })
        .await?;
        PhotoRepo::cache_photo_like_status(state, user_id, photo_id, false).await;
        PhotoRepo::invalidate_photo_info(state, photo_id).await;
        Ok(())
    }

    /// 查询用户点赞过的照片 ID.
    pub(crate) async fn query_liked_photo_ids(
        state: &PhotoState,
        user_id: UserId,
        req: &LikedPhotosQuery,
    ) -> Result<CursorPage<(PhotoId, DateTime), TimeIdCursor<PhotoId>>> {
        PhotoLikeMapper::query_user_liked_photo_ids(&state.db, user_id, &req.cursor, req.size).await
    }
}
