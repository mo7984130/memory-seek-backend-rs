use common::{
    DbConn as ConnectionTrait,
    error::contextual::ext::{OptionExt, UintExt},
    error::{AppError, ContextualError, contextual::Result},
    ext::{ToErr, ToOk},
    types::CursorPage,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, ExprTrait, Order, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, Query},
};
use types::{
    cursor::TimeIdCursor,
    photo::{face::*, person::PersonId, photo::PhotoId},
};

pub struct FaceMapper;

// 创建
impl FaceMapper {
    pub async fn inserts(db: &impl ConnectionTrait, new_faces: Vec<NewFaceRecord>) -> Result<u64> {
        Entity::insert_many(new_faces.into_iter().map(ActiveModel::from))
            .exec_without_returning(db)
            .await?
            .to_ok()
    }
}

// 修改
impl FaceMapper {
    pub async fn update_person_id(
        db: &impl ConnectionTrait,
        person_id: Option<PersonId>,
        face_id: FaceId,
    ) -> Result<()> {
        Entity::update_many()
            .filter(Column::Id.eq(face_id))
            .col_expr(Column::PersonId, Expr::value(person_id))
            .exec(db)
            .await?
            .rows_affected
            .no_zero_or_warn(
                "update_person_id_fail",
                "修改人脸的人物id失败",
                AppError::bad_request("修改人脸人物失败"),
            )?;
        Ok(())
    }

    /// 更新多个人脸的人物id
    pub async fn update_person_ids(
        db: &impl ConnectionTrait,
        person_id: Option<PersonId>,
        face_ids: impl ExactSizeIterator<Item = FaceId>,
    ) -> Result<()> {
        let size = face_ids.len() as u64;
        let affected = Entity::update_many()
            .filter(Column::Id.is_in(face_ids))
            .col_expr(Column::PersonId, Expr::value(person_id))
            .exec(db)
            .await?
            .rows_affected;
        if affected != size {
            ContextualError::error_without_source(
                "update_person_ids_fail",
                "修改人脸人物id失败",
                AppError::bad_request("修改人脸人物失败"),
            )
            .to_err()
        } else {
            Ok(())
        }
    }

    /// 清除该人物的所有人脸的人物id
    pub async fn clean_person_id_by_person_id(
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

    /// 清除所有人脸的人物id
    pub async fn clean_person_id(db: &impl ConnectionTrait) -> Result<u64> {
        Entity::update_many()
            .col_expr(Column::PersonId, Expr::value(sea_orm::Value::BigInt(None)))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}

// 查询
impl FaceMapper {
    /// 查询所有人脸
    pub async fn query_all(db: &impl ConnectionTrait) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 加行锁查询
    pub async fn lock_by_id(db: &impl ConnectionTrait, face_id: FaceId) -> Result<FaceRecord> {
        Entity::find()
            .filter(Column::Id.eq(face_id))
            .lock_exclusive()
            .one(db)
            .await?
            .map(FaceRecord::from)
            .ok_or_warn(
                "face_not_found",
                "人脸不存在",
                AppError::not_found("人脸不存在"),
            )
    }

    pub async fn lock_by_ids(
        db: &impl ConnectionTrait,
        face_ids: impl IntoIterator<Item = &FaceId>,
    ) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(face_ids.into_iter().copied()))
            .lock_exclusive()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 按照片 id 加行锁批量查询人脸
    pub async fn lock_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .lock_exclusive()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 查询某人物下 score 最高的人脸
    ///
    /// 若提供了 `exclude_face_ids`，则排除这些人脸的查询结果
    pub async fn query_top_score_by_person_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
        exclude_face_ids: Option<&[FaceId]>,
    ) -> Result<Option<FaceRecord>> {
        let mut query = Entity::find().filter(Column::PersonId.eq(person_id));

        // 如果提供了排除列表，添加过滤条件
        if let Some(ids) = exclude_face_ids {
            query = query.filter(Column::Id.is_not_in(ids.iter().copied()));
        }

        query
            .order_by(Column::Score, Order::Desc)
            .one(db)
            .await?
            .map(FaceRecord::from)
            .to_ok()
    }

    // 查询人脸计算所需的照片id和file_id
    pub async fn query_face_compute_photos(
        db: &impl ConnectionTrait,
        full: bool,
        size: u64,
        previous_id: PhotoId,
    ) -> Result<Vec<(PhotoId, String)>> {
        let condition = if full {
            Condition::all().add(types::photo::photo::Column::Id.gt(previous_id))
        } else {
            let subquery = Query::select()
                .expr(Expr::val(1))
                .from(types::photo::face::Entity)
                .and_where(
                    Expr::col((
                        types::photo::face::Entity,
                        types::photo::face::Column::PhotoId,
                    ))
                    .equals((types::photo::photo::Entity, types::photo::photo::Column::Id)),
                )
                .to_owned();
            Condition::all()
                .add(types::photo::photo::Column::Id.gt(previous_id))
                .add(Expr::exists(subquery).not())
        };
        types::photo::photo::Entity::find()
            .select_only()
            .column(types::photo::photo::Column::Id)
            .column(types::photo::photo::Column::FileId)
            .filter(condition)
            .order_by(types::photo::photo::Column::Id, sea_orm::Order::Asc)
            .limit(size)
            .into_tuple::<(PhotoId, String)>()
            .all(db)
            .await
            .map_err(Into::into)
    }

    /// 查询照片里面的人脸.
    pub async fn query_by_photo_id(
        db: &impl ConnectionTrait,
        photo_id: PhotoId,
    ) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::PhotoId.eq(photo_id))
            .lock_exclusive()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn lock_unassigned_faces(db: &impl ConnectionTrait) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::PersonId.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    pub async fn query_unassigned_face_photo_ids_cursor_page(
        db: &impl ConnectionTrait,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<CursorPage<PhotoId, ()>> {
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

        let records = query
            .select_only()
            .column(types::photo::photo::Column::Id)
            .into_tuple::<PhotoId>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size))
    }

    // 游标查询人物的照片
    pub async fn query_person_photo_ids(
        db: &impl ConnectionTrait,
        person_id: PersonId,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<CursorPage<PhotoId, ()>> {
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

        let records = query
            .select_only()
            .column(types::photo::photo::Column::Id)
            .into_tuple::<PhotoId>()
            .all(db)
            .await?;

        Ok(CursorPage::from_oversize(records, size))
    }

    pub async fn lock_by_person_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<Vec<FaceRecord>> {
        Entity::find()
            .filter(Column::PersonId.eq(person_id))
            .lock_exclusive()
            .all(db)
            .await?
            .into_iter()
            .map(FaceRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }
}

// 删除
impl FaceMapper {
    pub async fn delete_by_ids(db: &impl ConnectionTrait, face_ids: &[FaceId]) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::Id.is_in(face_ids.iter().copied()))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 删除照片的全部人脸记录
    pub async fn delete_by_photo_ids(
        db: &impl ConnectionTrait,
        photo_ids: &[PhotoId],
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::PhotoId.is_in(photo_ids.iter().copied()))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
