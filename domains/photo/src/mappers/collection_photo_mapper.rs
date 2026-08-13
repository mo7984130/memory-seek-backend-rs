use std::collections::HashMap;

use common::error::deferred::Result;
use common::ext::OkExt;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
use types::photo::collection_photo::*;
use types::photo::{collection::CollectionId, photo::PhotoId};

pub(crate) struct CollectionPhotoMapper;

impl CollectionPhotoMapper {
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
            .filter(Column::CollectionId.eq(collection_id))
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected as u64)
    }

    /// 根据photo_ids 删除收藏夹照片
    /// 返回HashMap<受影响的收藏夹id, 该收藏夹删除的照片个数(为负)>
    pub async fn delete_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<HashMap<CollectionId, i64>> {
        if photo_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let affected: HashMap<CollectionId, i64> = Entity::find()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .select_only()
            .column(Column::CollectionId)
            .into_tuple::<CollectionId>()
            .all(db)
            .await?
            .into_iter()
            .fold(HashMap::new(), |mut map, collection_id| {
                *map.entry(collection_id).or_insert(0i64) -= 1;
                map
            });

        Entity::delete_many()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .exec(db)
            .await?;

        Ok(affected)
    }

    /// 分页契约: 查询 size+1 条, 多出的 1 条用于 has_more 判定,
    /// 由 service 层用 CursorPage::from_oversize 截断消费。
    pub async fn query_photo_id_by_collection_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        cursor: Option<&TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        let mut query = Entity::find()
            .filter(Column::CollectionId.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size + 1);

        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::Id));
        }

        query
            .select_only()
            .column(Column::PhotoId)
            .into_tuple::<PhotoId>()
            .all(db)
            .await?
            .to_ok()
    }

    pub async fn delete_by_collection_id(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        user_id: UserId,
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::CollectionId.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 查询包含指定照片的所有收藏夹 ID
    pub async fn query_collection_ids_by_photo_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
        photo_id: PhotoId,
    ) -> Result<Vec<CollectionId>> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .filter(Column::UserId.eq(user_id))
            .select_only()
            .column(Column::CollectionId)
            .into_tuple::<CollectionId>()
            .all(db)
            .await?
            .to_ok()
    }
}
