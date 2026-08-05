use std::collections::HashSet;

use common::Result;
use common::ext::ToOk;
use common::models::TimeIdCursor;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    entity::prelude::DateTimeUtc,
};
use types::photo::photo_like::*;
use types::{auth::user::UserId, photo::photo::PhotoId};

pub struct PhotoLikeMapper;

// 创建
impl PhotoLikeMapper {
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<bool> {
        let now = chrono::Utc::now();
        let active_model = ActiveModel {
            photo_id: Set(photo_id.0),
            user_id: Set(user_id.0),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let rows_affected = Entity::insert(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([Column::PhotoId, Column::UserId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;

        Ok(rows_affected > 0)
    }
}

// 查询
impl PhotoLikeMapper {
    /// 批量查询用户对一组照片的点赞状态
    pub async fn query_is_like_by_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_ids: &[PhotoId],
    ) -> Result<HashSet<PhotoId>> {
        if photo_ids.is_empty() {
            return HashSet::new().to_ok();
        }

        Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .into_tuple::<i64>()
            .all(db)
            .await?
            .into_iter()
            .map(PhotoId)
            .collect::<HashSet<PhotoId>>()
            .to_ok()
    }

    /// 查询用户点赞的照片ID和点赞时间列表（带游标分页）
    ///
    /// 返回 `(PhotoId, DateTimeUtc)` 元组，其中 DateTimeUtc 为点赞时间。
    pub async fn query_user_liked_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        cursor: &Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<Vec<(PhotoId, DateTimeUtc)>> {
        let mut query = Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .column(Column::CreatedAt)
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::PhotoId);

        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::PhotoId));
        }

        query
            .limit(size)
            .into_tuple::<(i64, DateTimeUtc)>()
            .all(db)
            .await?
            .into_iter()
            .map(|(photo_id, created_at)| (PhotoId(photo_id), created_at))
            .collect::<Vec<(PhotoId, DateTimeUtc)>>()
            .to_ok()
    }
}

// 删除
impl PhotoLikeMapper {
    pub async fn delete(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<bool> {
        let res = Entity::delete_many()
            .filter(Column::PhotoId.eq(photo_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(res.rows_affected != 0)
    }

    pub async fn delete_all_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<u64> {
        if photo_ids.is_empty() {
            return Ok(0);
        }

        Entity::delete_many()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
