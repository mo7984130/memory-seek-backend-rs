use common::{
    error::{AppError, contextual::Result},
    models::CursorPage,
};
use types::{
    auth::user::UserId,
    photo::{models::LikedPhotosQuery, photo::PhotoId},
};

use super::PhotoRepo;
use crate::mappers::{photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper};

impl PhotoRepo {
    /// 校验照片存在后创建照片点赞.
    pub(crate) async fn like_photo(&self, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                PhotoMapper::ensure_exist(txn, photo_id).await?;
                if !PhotoLikeMapper::insert(txn, user_id, photo_id).await? {
                    return Err(AppError::bad_request("已经点赞过"));
                }
                PhotoMapper::update_like_count_delta(txn, photo_id, 1).await?;
                Ok(())
            })
        })
        .await?;
        self.cache_photo_like_status(user_id, photo_id, true).await;
        Ok(())
    }
    /// 删除用户对照片的点赞并维护点赞计数.
    pub(crate) async fn unlike_photo(&self, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        self.transaction(|txn| {
            Box::pin(async move {
                if !PhotoLikeMapper::delete(txn, user_id, photo_id).await? {
                    return Err(AppError::bad_request("还未点赞"));
                }
                PhotoMapper::update_like_count_delta(txn, photo_id, -1).await?;
                Ok(())
            })
        })
        .await?;
        self.cache_photo_like_status(user_id, photo_id, false).await;
        Ok(())
    }
    /// 按游标查询用户点赞过的照片 ID.
    pub(crate) async fn query_liked_photo_ids(
        &self,
        user_id: UserId,
        req: &LikedPhotosQuery,
    ) -> common::error::contextual::Result<
        CursorPage<(PhotoId, sea_orm::entity::prelude::DateTimeUtc), ()>,
    > {
        Ok(CursorPage::from_oversize(
            PhotoLikeMapper::query_user_liked_photo_ids(&self.db, user_id, &req.cursor, req.size)
                .await?,
            req.size,
        ))
    }
}
