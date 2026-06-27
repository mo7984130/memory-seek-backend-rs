use common::{
    Result,
    error::AppError,
    ext::{ResultErrExt, ToErr, log_warn},
    metrics_group, metrics_success, metrics_timer_name, timed,
    utils::{DbUtils, MetricsTimerExt},
};
use entities::{auth::user::UserId, photo::photo::PhotoId};

use crate::{
    mappers::{photo_like_mapper::PhotoLikeMapper, photo_mapper::PhotoMapper},
    state::PhotoState,
};

pub(crate) struct PhotoLikeService;

// 创建
impl PhotoLikeService {
    pub async fn like(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        metrics_group!("like_photo");

        // 检查照片是否存在
        if !PhotoMapper::exists(&state.db, photo_id)
            .timed(metrics_timer_name!("like_photo", "check_exists"))
            .await?
        {
            return log_warn(
                "photo_not_found",
                "用户尝试点赞不存在的照片",
                "",
                AppError::not_found("照片不存在"),
            )
            .to_err();
        }

        timed!("like_photo", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    let inserted =
                        PhotoLikeMapper::insert_ignore(txn, user_id, photo_id).await?;

                    if !inserted {
                        return log_warn(
                            "photo_like_already_exist",
                            "用户尝试点赞一个已经点赞过的照片",
                            "",
                            AppError::bad_request("已经点赞过"),
                        )
                        .to_err();
                    }

                    // 增加点赞总数
                    PhotoMapper::update_like_count_delta(txn, photo_id, 1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!("like_photo");
        Ok(())
    }
}

// 查询
impl PhotoLikeService {
    /// 批量查询用户对一组照片的点赞状态
    pub async fn get_like_status(
        state: &PhotoState,
        user_id: UserId,
        photo_ids: Vec<PhotoId>,
    ) -> Result<std::collections::HashSet<PhotoId>> {
        metrics_group!("get_photo_like_status");

        let result = PhotoLikeMapper::query_is_like_by_photo_ids(&state.db, user_id, photo_ids)
            .timed(metrics_timer_name!("get_photo_like_status", "query_likes"))
            .await?;

        metrics_success!("get_photo_like_status");
        Ok(result)
    }

    /// 查询用户点赞的照片列表
    pub async fn get_user_liked_photos(
        state: &PhotoState,
        user_id: UserId,
        cursor: Option<String>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        metrics_group!("get_user_liked_photos");

        let decoded_cursor = cursor
            .as_ref()
            .and_then(|s| PhotoLikeCursor::decode(s).ok());

        let photo_ids = PhotoLikeMapper::query_user_liked_photo_ids(
            &state.db,
            user_id,
            decoded_cursor.map(|c| (c.created_at, c.id.0)),
            size,
        )
        .timed(metrics_timer_name!("get_user_liked_photos", "query_ids"))
        .await?;

        metrics_success!("get_user_liked_photos");
        Ok(photo_ids)
    }
}

// 删除
impl PhotoLikeService {
    pub async fn unlike(state: &PhotoState, user_id: UserId, photo_id: PhotoId) -> Result<()> {
        metrics_group!("unlike_photo");

        timed!("unlike_photo", "db_transaction", {
            DbUtils::write(&state.db, |txn| {
                Box::pin(async move {
                    let deleted = PhotoLikeMapper::delete(txn, user_id, photo_id).await?;

                    if !deleted {
                        return log_warn(
                            "photo_like_not_exist",
                            "用户尝试取消点赞一个未点赞过的照片",
                            "",
                            AppError::bad_request("还未点赞"),
                        )
                        .to_err();
                    }

                    // 减少点赞总数
                    PhotoMapper::update_like_count_delta(txn, photo_id, -1).await?;
                    Ok(())
                })
            })
            .await
        })?;

        metrics_success!("unlike_photo");
        Ok(())
    }
}

/// 点赞游标，用于复合游标分页
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PhotoLikeCursor {
    pub created_at: sea_orm::entity::prelude::DateTimeUtc,
    pub id: PhotoId,
}

impl PhotoLikeCursor {
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            json.as_bytes(),
        )
    }

    pub fn decode(s: &str) -> Result<Self> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .trace_warn_bad_request(
                "photo_like_cursor:decode_err",
                "解码photo_like_cursor错误, base64解码失败",
                "解码photo_like_cursor错误",
            )?;
        let json = String::from_utf8(bytes).trace_warn_bad_request(
            "photo_like_cursor:from_utf8_err",
            "解码photo_like_cursor错误, bytes转String错误",
            "解码photo_like_cursor错误",
        )?;
        serde_json::from_str(&json).trace_warn_bad_request(
            "photo_like_cursor:from_str_err",
            "解码photo_like_cursor错误, json解析失败",
            "解码photo_like_cursor错误",
        )
    }
}
