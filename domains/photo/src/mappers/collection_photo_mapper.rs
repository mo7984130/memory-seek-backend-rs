use std::collections::{HashMap, HashSet};

use chrono::Utc;
use common::Result;
use common::error::AppError;
use common::ext::{OkExt, ResultErrExt, ToErr, log_warn};
use entities::auth::user::UserId;
use entities::photo::collection_photo::*;
use entities::photo::{collection::CollectionId, photo::PhotoId};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::models::collection::CollectionPhotoCursor;

pub(crate) struct CollectionPhotoMapper;

impl CollectionPhotoMapper {
    pub async fn exists_in_collection(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        photo_ids: &[PhotoId],
    ) -> Result<HashSet<PhotoId>> {
        if photo_ids.is_empty() {
            return Ok(HashSet::new());
        }

        Entity::find()
            .filter(Column::CollectionId.eq(collection_id.0))
            .filter(Column::PhotoId.is_in(photo_ids.iter().map(|id| id.0)))
            .select_only()
            .column(Column::PhotoId)
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?
            .into_iter()
            .map(PhotoId::from)
            .collect::<HashSet<_>>()
            .to_ok()
    }

    pub async fn inserts(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &[PhotoId],
    ) -> Result<()> {
        if photo_ids.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        let models: Vec<ActiveModel> = photo_ids
            .iter()
            .map(|photo_id| ActiveModel {
                collection_id: Set(collection_id.0),
                photo_id: Set(photo_id.0),
                user_id: Set(user_id.0),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            })
            .collect();

        Entity::insert_many(models)
            .exec(db)
            .await
            .trace_internal_err("db_insert_err", "批量添加到收藏夹失败")?;

        Ok(())
    }

    pub async fn is_belong(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::CollectionId.eq(collection_id.0))
            .filter(Column::UserId.eq(user_id.0))
            .count(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?;

        Ok(count > 0)
    }

    pub async fn count_photo_by_collection_id(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
    ) -> Result<u64> {
        Entity::find()
            .filter(Column::CollectionId.eq(collection_id.0))
            .count(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
    }

    pub async fn delete_by_collection_id_and_photo_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: &[PhotoId],
    ) -> Result<u64> {
        if photo_ids.is_empty() {
            return Ok(0);
        }

        let result = Entity::delete_many()
            .filter(Column::CollectionId.eq(collection_id.0))
            .filter(Column::PhotoId.is_in(photo_ids.iter().map(|id| id.0)))
            .filter(Column::UserId.eq(user_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "批量移除失败")?;

        Ok(result.rows_affected as u64)
    }

    /// 根据photo_ids 删除收藏夹照片
    /// 返回HashMap<受影响的收藏夹id, 该收藏夹删除的照片个数>
    pub async fn delete_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<HashMap<CollectionId, u64>> {
        if photo_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let affected: HashMap<CollectionId, u64> = Entity::find()
            .filter(Column::PhotoId.is_in(photo_ids.iter().map(|id| id.0)))
            .select_only()
            .column(Column::CollectionId)
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "获取受影响的收藏夹Id错误")?
            .into_iter()
            .fold(HashMap::new(), |mut map, collection_id| {
                *map.entry(CollectionId(collection_id)).or_insert(0u64) += 1;
                map
            });

        Entity::delete_many()
            .filter(Column::PhotoId.is_in(photo_ids.iter().map(|id| id.0)))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "批量移除失败")?;

        Ok(affected)
    }

    pub async fn query_photo_id_by_collection_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        cursor: Option<&CollectionPhotoCursor>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        let mut query = Entity::find()
            .filter(Column::CollectionId.eq(collection_id.0))
            .filter(Column::UserId.eq(user_id.0))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size);

        if let Some(c) = cursor {
            query = query.filter(
                sea_orm::Condition::any()
                    .add(Column::CreatedAt.lt(c.created_at))
                    .add(
                        sea_orm::Condition::all()
                            .add(Column::CreatedAt.eq(c.created_at))
                            .add(Column::Id.lt(c.id.0)),
                    ),
            );
        }

        query
            .select_only()
            .column(Column::PhotoId)
            .into_values::<i64, Column>()
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?
            .into_iter()
            .map(|id| PhotoId(id))
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn delete_by_collection_id(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        user_id: UserId,
    ) -> Result<u64> {
        let res = Entity::delete_many()
            .filter(Column::CollectionId.eq(collection_id.0))
            .filter(Column::UserId.eq(user_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_del_err", "删除收藏夹照片失败")?;

        if res.rows_affected == 0 {
            return log_warn(
                "delete_rows_affected_zero",
                "删除收藏夹照片影响行为零",
                "",
                AppError::bad_request("删除收藏夹照片失败"),
            )
            .to_err();
        }

        Ok(res.rows_affected as u64)
    }
}
