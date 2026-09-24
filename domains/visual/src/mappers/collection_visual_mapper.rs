use std::collections::HashMap;

use common::ext::ToOk;
use common::types::CursorPage;
use common::{DbConn as ConnectionTrait, error::contextual::Result};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use types_core::UserId;
use types_core::cursor::TimeIdCursor;
use types_visual::collection_visual::*;
use types_visual::{collection::CollectionId, visual::VisualId};

pub(crate) struct CollectionVisualMapper;

impl CollectionVisualMapper {
    /// 删除指定相册中的指定影像关联, 并返回受影响的影像数量.
    pub async fn delete_by_collection_id_and_visual_ids(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        visual_ids: &[VisualId],
    ) -> Result<u64> {
        if visual_ids.is_empty() {
            return Ok(0);
        }

        let result = Entity::delete_many()
            .filter(Column::CollectionId.eq(collection_id))
            .filter(Column::VisualId.is_in(visual_ids.iter().copied()))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected as u64)
    }

    /// 根据visual_ids 删除收藏夹影像
    /// 返回HashMap<受影响的收藏夹id, 该收藏夹删除的影像个数(为负)>
    /// 删除影像对应的全部相册关联, 并返回受影响的相册计数.
    pub async fn delete_by_visual_ids(
        db: &impl ConnectionTrait,
        visual_ids: &[VisualId],
    ) -> Result<HashMap<CollectionId, i64>> {
        if visual_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let affected: HashMap<CollectionId, i64> = Entity::find()
            .filter(Column::VisualId.is_in(visual_ids.iter().copied()))
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
            .filter(Column::VisualId.is_in(visual_ids.iter().copied()))
            .exec(db)
            .await?;

        Ok(affected)
    }

    /// 查询相册中的影像 ID.
    pub async fn query_visual_id_by_collection_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        cursor: Option<&TimeIdCursor<VisualId>>,
        size: u64,
    ) -> Result<CursorPage<VisualId, ()>> {
        let mut query = Entity::find()
            .filter(Column::CollectionId.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size + 1);

        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::Id));
        }

        let records = query
            .select_only()
            .column(Column::VisualId)
            .into_tuple::<VisualId>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size))
    }

    /// 删除相册下的全部影像关联.
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

    /// 查询包含指定影像的所有收藏夹 ID
    /// 查询影像所属的相册 ID, 并按用户权限过滤.
    pub async fn query_collection_ids_by_visual_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<Vec<CollectionId>> {
        Entity::find()
            .filter(Column::VisualId.eq(visual_id))
            .filter(Column::UserId.eq(user_id))
            .select_only()
            .column(Column::CollectionId)
            .into_tuple::<CollectionId>()
            .all(db)
            .await?
            .to_ok()
    }
}
