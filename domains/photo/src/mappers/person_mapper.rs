use common::{Result, ext::ToOk};
use insight_face_rs::FaceEmbedding;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use types::photo::{FaceBBox, face::FaceId, person::*, photo::PhotoId};

pub struct PersonMapper;

/// 人物封面冗余字段更新(与 `photo_person.cover_*` 列对应)
pub struct PersonCoverUpdate {
    pub cover_face_id: FaceId,
    pub cover_photo_id: PhotoId,
    pub cover_file_id: String,
    pub cover_bbox: FaceBBox,
}

// 创建
impl PersonMapper {
    pub async fn insert(db: &impl ConnectionTrait, person: NewPerson) -> Result<PersonRecord> {
        let model = Entity::insert(ActiveModel::from(person))
            .exec_with_returning(db)
            .await?;
        PersonRecord::try_from(model)
    }
}

// 修改
impl PersonMapper {
    /// 重命名人物
    pub async fn rename(
        db: &impl ConnectionTrait,
        person_id: PersonId,
        new_name: String,
    ) -> Result<u64> {
        Entity::update_many()
            .filter(Column::Id.eq(person_id))
            .col_expr(Column::Name, Expr::value(new_name))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }

    /// 按 ID 加行锁查询(`SELECT ... FOR UPDATE`, 供转移归属/合并等读-改-写流程使用)
    pub async fn query_by_id_for_update(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<Option<PersonRecord>> {
        Entity::find()
            .filter(Column::Id.eq(person_id))
            .lock_exclusive()
            .one(db)
            .await?
            .map(PersonRecord::try_from)
            .transpose()
    }

    /// 增量更新人物统计(数量/权重/质心)与可选封面冗余字段
    ///
    /// `cover` 为 `None` 表示封面不变; `Some` 时连同 `cover_face_id/cover_photo_id/
    /// cover_file_id/cover_bbox` 一并更新(封面决策在 service 层完成)。
    pub async fn update_stats(
        db: &impl ConnectionTrait,
        person_id: PersonId,
        face_count: u64,
        weight: f64,
        centroid: FaceEmbedding,
        cover: Option<PersonCoverUpdate>,
    ) -> Result<u64> {
        let centroid: insight_face_rs::PgVector = centroid.into();
        let mut update = Entity::update_many()
            .filter(Column::Id.eq(person_id))
            .col_expr(Column::FaceCount, Expr::value(face_count as i64))
            .col_expr(Column::Weight, Expr::value(weight))
            .col_expr(Column::Centroid, Expr::value(centroid));

        if let Some(cover) = cover {
            update = update
                .col_expr(Column::CoverFaceId, Expr::value(cover.cover_face_id.0))
                .col_expr(Column::CoverPhotoId, Expr::value(cover.cover_photo_id.0))
                .col_expr(Column::CoverFileId, Expr::value(cover.cover_file_id))
                .col_expr(
                    Column::CoverBbox,
                    Expr::value(serde_json::to_value(cover.cover_bbox).unwrap()),
                );
        }

        update.exec(db).await?.rows_affected.to_ok()
    }
}

// 查询
impl PersonMapper {
    pub async fn query(
        db: &impl ConnectionTrait,
        cursor: Option<PersonId>,
        size: u64,
    ) -> Result<Vec<PersonRecord>> {
        let mut query = Entity::find().order_by_desc(Column::Id);
        if let Some(person_id) = cursor {
            query = query.filter(Column::Id.lt(person_id));
        }
        query
            .limit(size)
            .all(db)
            .await?
            .into_iter()
            .map(PersonRecord::try_from)
            .collect::<std::result::Result<Vec<_>, _>>()
    }

    /// 按 ID 查询人物
    pub async fn query_by_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<Option<PersonRecord>> {
        Entity::find()
            .filter(Column::Id.eq(person_id))
            .one(db)
            .await?
            .map(PersonRecord::try_from)
            .transpose()
    }

    /// 按 ID 批量查询人物
    pub async fn query_by_ids(
        db: &impl ConnectionTrait,
        person_ids: &[PersonId],
    ) -> Result<Vec<PersonRecord>> {
        if person_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .filter(Column::Id.is_in(person_ids.iter().map(|id| id.0)))
            .all(db)
            .await?
            .into_iter()
            .map(PersonRecord::try_from)
            .collect::<std::result::Result<Vec<_>, _>>()
    }
}

// 删除
impl PersonMapper {
    /// 删除人物
    pub async fn delete_by_id(db: &impl ConnectionTrait, person_id: PersonId) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::Id.eq(person_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
