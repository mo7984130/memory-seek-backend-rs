use std::collections::HashSet;

use common::Result;
use common::ext::{ResultErrExt, ToErr, ToOk, log_err};
use entities::photo::photo_like::*;
use entities::{auth::user::UserId, photo::photo::PhotoId};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    entity::prelude::DateTimeUtc,
};

pub struct PhotoLikeMapper;

// 创建
impl PhotoLikeMapper {
    pub async fn insert_ignore(
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

        let result = Entity::insert(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([Column::PhotoId, Column::UserId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(db)
            .await;

        match result {
            Ok(_) => Ok(true),
            Err(DbErr::RecordNotInserted) => Ok(false),
            Err(e) => log_err(
                "db_insert_err",
                "插入照片点赞时错误",
                e,
                common::error::AppError::InternalServerError,
            )
            .to_err(),
        }
    }
}

// 查询
impl PhotoLikeMapper {
    /// 批量查询用户对一组照片的点赞状态
    pub async fn query_is_like_by_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_ids: Vec<PhotoId>,
    ) -> Result<HashSet<PhotoId>> {
        if photo_ids.is_empty() {
            return HashSet::new().to_ok();
        }

        Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .filter(Column::UserId.eq(user_id.0))
            .filter(Column::PhotoId.is_in(photo_ids.into_iter().map(|id| id.0)))
            .into_tuple::<i64>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询照片是否点赞数据库错误")?
            .into_iter()
            .map(PhotoId)
            .collect::<HashSet<PhotoId>>()
            .to_ok()
    }

    /// 查询用户点赞的照片ID列表（带游标分页）
    ///
    /// cursor 为 `(created_at, id)` 元组，用于复合游标分页，
    /// 确保相同时间戳的记录不会被跳过。
    pub async fn query_user_liked_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        cursor: Option<(DateTimeUtc, i64)>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        let mut query = Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .filter(Column::UserId.eq(user_id.0))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id);

        if let Some((cursor_time, cursor_id)) = cursor {
            query = query.filter(
                sea_orm::Condition::any()
                    .add(Column::CreatedAt.lt(cursor_time))
                    .add(
                        sea_orm::Condition::all()
                            .add(Column::CreatedAt.eq(cursor_time))
                            .add(Column::Id.lt(cursor_id)),
                    ),
            );
        }

        query
            .limit(size)
            .into_tuple::<i64>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询用户点赞照片列表数据库错误")?
            .into_iter()
            .map(PhotoId)
            .collect::<Vec<PhotoId>>()
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
            .filter(Column::PhotoId.eq(photo_id.0))
            .filter(Column::UserId.eq(user_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "尝试删除照片点赞错误")?;

        Ok(res.rows_affected != 0)
    }

    pub async fn delete_all_by_photo_id(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::PhotoId.eq(photo_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "根据照片id删除所有点赞数据库错误")?
            .rows_affected
            .to_ok()
    }
}
