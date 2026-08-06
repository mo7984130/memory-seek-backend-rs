use common::{Result, ext::ToOk};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, Query},
};
use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
use types::photo::{face::*, person::PersonId, photo::PhotoId};

pub struct FaceMapper;

// 创建
impl FaceMapper {
    pub async fn query_by_photo_id(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
    ) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::try_from)
            .collect()
    }
}

// 修改
impl FaceMapper {
    pub async fn update_person_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
        face_ids: impl IntoIterator<Item = FaceId>,
    ) -> Result<u64> {
        Entity::update_many()
            .filter(Column::Id.is_in(face_ids))
            .col_expr(Column::PersonId, Expr::value(person_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 清空所有人脸的 person_id(全量聚类重建前调用, 避免悬空引用)
    pub async fn clear_person_id(db: &impl ConnectionTrait) -> Result<u64> {
        Entity::update_many()
            .col_expr(Column::PersonId, Expr::value(sea_orm::Value::BigInt(None)))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 修改单张人脸归属
    pub async fn update_face_person_id(
        db: &impl ConnectionTrait,
        face_id: FaceId,
        person_id: PersonId,
    ) -> Result<u64> {
        Entity::update_many()
            .filter(Column::Id.eq(face_id))
            .col_expr(Column::PersonId, Expr::value(person_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 清空单张人脸归属(取消归属, person_id 置 NULL)
    pub async fn clear_face_person_id(db: &impl ConnectionTrait, face_id: FaceId) -> Result<u64> {
        Entity::update_many()
            .filter(Column::Id.eq(face_id))
            .col_expr(Column::PersonId, Expr::value(sea_orm::Value::BigInt(None)))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 将某人物下的全部人脸归属转移到另一人物(合并)
    pub async fn move_person_faces(
        db: &impl ConnectionTrait,
        source_person_id: PersonId,
        target_person_id: PersonId,
    ) -> Result<u64> {
        Entity::update_many()
            .filter(Column::PersonId.eq(source_person_id))
            .col_expr(Column::PersonId, Expr::value(target_person_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 清空某人物下所有人脸的归属(删除人物前调用, 避免悬空引用)
    pub async fn clear_person_id_by_person(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<u64> {
        Entity::update_many()
            .filter(Column::PersonId.eq(person_id))
            .col_expr(Column::PersonId, Expr::value(sea_orm::Value::BigInt(None)))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}

// 查询
impl FaceMapper {
    pub async fn query_by_id(db: &impl ConnectionTrait, id: FaceId) -> Result<Option<FaceRecord>> {
        Entity::find()
            .filter(Column::Id.eq(id))
            .one(db)
            .await?
            .map(FaceRecord::try_from)
            .transpose()
    }

    pub async fn query_by_ids(
        db: &impl ConnectionTrait,
        ids: &[FaceId],
    ) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::try_from)
            .collect()
    }

    /// 按 ID 加行锁查询(`SELECT ... FOR UPDATE`, 供转移归属等读-改-写流程使用)
    pub async fn lock_by_id(
        db: &impl ConnectionTrait,
        face_id: FaceId,
    ) -> Result<Option<FaceRecord>> {
        Entity::find()
            .filter(Column::Id.eq(face_id))
            .lock_exclusive()
            .one(db)
            .await?
            .map(FaceRecord::try_from)
            .transpose()
    }

    /// 查询某人物下 score 最高的人脸(封面决策/封面回退用)
    pub async fn query_top_score_by_person_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<Option<FaceRecord>> {
        Entity::find()
            .filter(Column::PersonId.eq(person_id))
            .order_by(Column::Score, Order::Desc)
            .one(db)
            .await?
            .map(FaceRecord::try_from)
            .transpose()
    }

    pub async fn query_all(db: &impl ConnectionTrait) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::try_from)
            .collect()
    }

    /// 查询全部未分配人脸(`person_id IS NULL`, 增量插入或聚类离群)
    pub async fn query_unassigned(db: &impl ConnectionTrait) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::PersonId.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::try_from)
            .collect()
    }

    /// 查询当前用户"包含未分配人脸"的照片 id(keyset 分页, 基于照片的 (created_at, id))
    ///
    /// 用 `EXISTS` 子查询过滤, 保证同一照片多张未分配人脸不产生重复行。
    pub async fn query_unassigned_face_photo_ids_cursor_page(
        db: &impl ConnectionTrait,
        user_id: UserId,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        let subquery = Query::select()
            .expr(Expr::val(1))
            .from(Entity)
            .and_where(
                Expr::col((Entity, Column::PhotoId))
                    .equals((types::photo::photo::Entity, types::photo::photo::Column::Id)),
            )
            .and_where(Column::PersonId.is_null())
            .to_owned();

        let mut query = types::photo::photo::Entity::find()
            .filter(types::photo::photo::Column::UserId.eq(user_id))
            .filter(Expr::exists(subquery))
            .order_by(types::photo::photo::Column::CreatedAt, Order::Desc)
            .order_by(types::photo::photo::Column::Id, Order::Desc)
            .limit(size);

        if let Some(cursor) = cursor {
            query = query.filter(cursor.before(
                types::photo::photo::Column::CreatedAt,
                types::photo::photo::Column::Id,
            ));
        }

        query
            .select_only()
            .column(types::photo::photo::Column::Id)
            .into_tuple::<PhotoId>()
            .all(db)
            .await?
            .to_ok()
    }

    pub async fn query_photo_ids_cursor_page(
        db: &impl ConnectionTrait,
        person_id: PersonId,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<Vec<PhotoId>> {
        let mut query = Entity::find()
            .select_only()
            .column(Column::PhotoId)
            .filter(Column::PersonId.eq(person_id))
            .order_by(Column::CreatedAt, Order::Desc)
            .order_by(Column::Id, Order::Desc)
            .limit(size);

        if let Some(cursor) = cursor {
            query = query.filter(cursor.before(Column::CreatedAt, Column::Id));
        }

        query.into_tuple::<PhotoId>().all(db).await?.to_ok()
    }
}

// 删除
impl FaceMapper {
    /// 删除单张人脸(仅限未归属人脸, 归属校验在 service 层完成)
    pub async fn delete_by_id(db: &impl ConnectionTrait, face_id: FaceId) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::Id.eq(face_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
