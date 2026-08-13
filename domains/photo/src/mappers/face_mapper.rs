use common::{error::DeferredResult as Result, ext::ToOk};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, Query},
};
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

    /// 按照片 id 加行锁批量查询人脸(`SELECT ... FOR UPDATE`,
    /// 删除照片前锁定全部人脸行, 防止与并发转移归属互死锁/丢失更新)
    pub async fn lock_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<FaceRecord>> {
        if photo_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .lock_exclusive()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::try_from)
            .collect()
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

    /// 查询"包含未分配人脸"的照片 id(keyset 分页, 基于照片的 (created_at, id))
    ///
    /// 不区分照片归属者, 全局扫描未分配人脸。
    /// 用 `EXISTS` 子查询过滤, 保证同一照片多张未分配人脸不产生重复行。
    /// 分页契约: 查询 size+1 条, 多出的 1 条用于 has_more 判定,
    /// 由 service 层用 CursorPage::from_oversize_fn 截断消费。
    pub async fn query_unassigned_face_photo_ids_cursor_page(
        db: &impl ConnectionTrait,
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
            .filter(Expr::exists(subquery))
            .order_by(types::photo::photo::Column::CreatedAt, Order::Desc)
            .order_by(types::photo::photo::Column::Id, Order::Desc)
            .limit(size + 1);

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

    /// 查询某人物的人脸照片 id(keyset 分页, 基于照片的 (created_at, id))
    ///
    /// 用 `EXISTS` 子查询过滤该人物的人脸, 保证同一照片多张人脸不产生重复行;
    /// 排序与游标均基于 photo 表, 与 `next_cursor` 的编码维度一致。
    /// 分页契约: 查询 size+1 条, 多出的 1 条用于 has_more 判定,
    /// 由 service 层用 CursorPage::from_oversize_fn 截断消费。
    pub async fn query_photo_ids_cursor_page(
        db: &impl ConnectionTrait,
        person_id: PersonId,
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
            .and_where(Column::PersonId.eq(person_id))
            .to_owned();

        let mut query = types::photo::photo::Entity::find()
            .filter(Expr::exists(subquery))
            .order_by(types::photo::photo::Column::CreatedAt, Order::Desc)
            .order_by(types::photo::photo::Column::Id, Order::Desc)
            .limit(size + 1);

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

    /// 删除照片的全部人脸记录(删除照片时清理人脸, 归属人物统计由 service 层维护)
    pub async fn delete_by_photo_ids(
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

    /// 批量删除未归属人脸(仅删除 `person_id IS NULL` 的人脸, 归属校验由 SQL 条件原子完成)
    pub async fn delete_unassigned_by_ids(
        db: &impl ConnectionTrait,
        face_ids: &[FaceId],
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::Id.is_in(face_ids.iter().copied()))
            .filter(Column::PersonId.is_null())
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
