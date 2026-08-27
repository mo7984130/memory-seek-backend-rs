use common::{
    DbConn as ConnectionTrait,
    error::contextual::ext::{OptionExt, UintExt},
    error::{AppError, contextual::Result},
    ext::ToOk,
    types::CursorPage,
    types::HasChanged::Changed,
};
use insight_face_rs::BoundingBox;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, extension::postgres::PgExpr},
};
use types::{
    cursor::CountIdCursor,
    photo::{
        face::{FaceId, FaceRecord},
        person::*,
    },
};

use crate::mappers::{face_mapper::FaceMapper, photo_mapper::PhotoMapper};

pub struct PersonMapper;

// 创建
impl PersonMapper {
    /// 插入新人物记录
    pub async fn insert(db: &impl ConnectionTrait, person: NewPerson) -> Result<PersonRecord> {
        let model = Entity::insert(ActiveModel::from(person))
            .exec_with_returning(db)
            .await?;
        Ok(PersonRecord::from(model))
    }
}

// 修改
impl PersonMapper {
    pub async fn add_faces(
        db: &impl ConnectionTrait,
        person: PersonRecord,
        faces: &[FaceRecord],
    ) -> Result<()> {
        let mut update_person = UpdatePersonRecord::new(person.id);

        // 判断是否需要修改封面
        // 取反判断, 视NaN为最小
        let max_score_face = faces.iter().max_by(|a, b| b.score.total_cmp(&a.score));
        match max_score_face {
            // None即无人脸, 直接返回即可
            None => return Ok(()),
            Some(max_score_face) => {
                // 更新封面
                if max_score_face.score > person.cover.face_score {
                    update_person.with_cover_face(
                        max_score_face,
                        PhotoMapper::query_file_id_by_id(db, max_score_face.photo_id).await?,
                    );
                }
            }
        }

        // 修改其他
        let mut new_centroid = person.centroid;
        let mut new_weight = person.weight;
        faces.iter().for_each(|f| {
            new_centroid = new_centroid.add_scaled(&f.embedding, f.score);
            new_weight += f.score as f64;
        });
        update_person.centroid = Changed(new_centroid);
        update_person.face_count = Changed(person.face_count + faces.len() as u64);
        update_person.weight = Changed(new_weight);

        Self::update(db, update_person).await?;

        Ok(())
    }

    pub async fn remove_faces(
        db: &impl ConnectionTrait,
        person: PersonRecord,
        faces: &[FaceRecord],
    ) -> Result<()> {
        let mut update_person = UpdatePersonRecord::new(person.id);
        let remove_faces_id: Vec<FaceId> = faces.iter().map(|f| f.id).collect();

        // 判断是否需要修改封面
        if faces.iter().any(|f| f.id == person.cover.face_id) {
            // 获取剩下的分数最高的
            let new_cover_face =
                FaceMapper::query_top_score_by_person_id(db, person.id, Some(&remove_faces_id))
                    .await?;

            // 更新封面
            // 当这个为None时, 代表已经没有剩余的人脸了
            // 现在不做特殊处理, 保留无人脸的人物
            if let Some(face) = new_cover_face {
                update_person.with_cover_face(
                    &face,
                    PhotoMapper::query_file_id_by_id(db, face.photo_id).await?,
                );
            }
        }

        // 修改其他
        let mut new_centroid = person.centroid;
        let mut new_weight = person.weight;
        faces.iter().for_each(|f| {
            new_centroid = new_centroid.sub_scaled(&f.embedding, f.score);
            new_weight -= f.score as f64;
        });
        update_person.centroid = Changed(new_centroid);
        update_person.face_count = Changed(person.face_count - faces.len() as u64);
        update_person.weight = Changed(new_weight);

        Self::update(db, update_person).await?;

        Ok(())
    }

    pub async fn update(db: &impl ConnectionTrait, person: UpdatePersonRecord) -> Result<u64> {
        let mut update = Entity::update_many().filter(Column::Id.eq(person.id));

        if let Changed(name) = person.name {
            update = update.col_expr(Column::Name, Expr::value(name))
        };
        if let Changed(name_initials) = person.name_initials {
            update = update.col_expr(Column::NameInitials, Expr::value(name_initials))
        };

        if let Changed(face_count) = person.face_count {
            update = update.col_expr(Column::FaceCount, Expr::value(face_count as i64))
        };
        if let Changed(weight) = person.weight {
            update = update.col_expr(Column::Weight, Expr::value(weight))
        };
        if let Changed(centroid) = person.centroid {
            update = update.col_expr(Column::Centroid, Expr::value(centroid))
        };

        if let Changed(cover) = person.cover {
            update = update
                .col_expr(Column::CoverFaceId, Expr::value(cover.face_id))
                .col_expr(Column::CoverPhotoId, Expr::value(cover.photo_id))
                .col_expr(Column::CoverFileId, Expr::value(cover.file_id))
                .col_expr(
                    Column::CoverBbox,
                    Expr::value(BoundingBox::from(cover.bbox)),
                );
        }

        update.exec(db).await?.rows_affected.to_ok()
    }
}

// 查询
impl PersonMapper {
    pub async fn query_all(db: &impl ConnectionTrait) -> Result<Vec<PersonRecord>> {
        Entity::find()
            .all(db)
            .await?
            .into_iter()
            .map(PersonRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 通过 ID 查询人物
    pub async fn query_by_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<PersonRecord> {
        Entity::find()
            .filter(Column::Id.eq(person_id))
            .one(db)
            .await?
            .map(PersonRecord::from)
            .ok_or_error(
                "person_not_found",
                "人物不存在",
                AppError::not_found("人物不存在"),
            )
    }

    /// 加行锁查询
    pub async fn lock_by_id(
        db: &impl ConnectionTrait,
        person_id: PersonId,
    ) -> Result<PersonRecord> {
        Entity::find()
            .filter(Column::Id.eq(person_id))
            .lock_exclusive()
            .one(db)
            .await?
            .map(PersonRecord::from)
            .ok_or_error(
                "person_not_found",
                "人物不存在",
                AppError::not_found("人物不存在"),
            )
    }
    pub async fn lock_by_ids(
        db: &impl ConnectionTrait,
        person_ids: impl IntoIterator<Item = &PersonId>,
    ) -> Result<Vec<PersonRecord>> {
        Entity::find()
            .filter(Column::Id.is_in(person_ids.into_iter().copied()))
            .lock_exclusive()
            .all(db)
            .await?
            .into_iter()
            .map(PersonRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 按 ID 批量查询人物 id 与 name
    pub async fn query_id_and_name_by_ids(
        db: &impl ConnectionTrait,
        person_ids: impl IntoIterator<Item = PersonId>,
    ) -> Result<Vec<(PersonId, String)>> {
        Entity::find()
            .filter(Column::Id.is_in(person_ids))
            .select_only()
            .column(Column::Id)
            .column(Column::Name)
            .into_tuple::<(PersonId, String)>()
            .all(db)
            .await?
            .to_ok()
    }

    pub async fn query_page(
        db: &impl ConnectionTrait,
        cursor: Option<CountIdCursor<PersonId>>,
        size: u64,
    ) -> Result<CursorPage<PersonRecord, ()>> {
        let mut query = Entity::find()
            .order_by_desc(Column::FaceCount)
            .order_by_desc(Column::Id);
        if let Some(cursor) = cursor {
            query = query.filter(cursor.before(Column::FaceCount, Column::Id));
        }
        let records = query
            .limit(size + 1)
            .all(db)
            .await?
            .into_iter()
            .map(PersonRecord::from)
            .collect();

        Ok(CursorPage::from_oversize(records, size))
    }

    /// 按关键词前缀搜索人物
    ///
    /// 匹配 `name` 或 `name_initials` 的前缀(ILIKE 忽略大小写);
    /// `name_initials` 为 NULL 的存量数据仅能按 `name` 命中。
    pub async fn query_search(
        db: &impl ConnectionTrait,
        keyword: &str,
        cursor: Option<PersonId>,
        size: u64,
    ) -> Result<CursorPage<PersonRecord, ()>> {
        let escaped = keyword
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let mut query = Entity::find()
            .filter(
                Condition::any()
                    .add(Expr::col(Column::Name).ilike(format!("{escaped}%")))
                    .add(Expr::col(Column::NameInitials).ilike(format!("{escaped}%"))),
            )
            .order_by_desc(Column::Id);
        if let Some(person_id) = cursor {
            query = query.filter(Column::Id.lt(person_id));
        }
        let records = query
            .limit(size + 1)
            .all(db)
            .await?
            .into_iter()
            .map(PersonRecord::from)
            .collect();

        Ok(CursorPage::from_oversize(records, size))
    }
}

// 删除
impl PersonMapper {
    pub async fn delete(db: &impl ConnectionTrait, person_id: PersonId) -> Result<()> {
        Entity::delete_many()
            .filter(Column::Id.eq(person_id))
            .exec(db)
            .await?
            .rows_affected
            .no_zero_or_error(
                "delete_person_err",
                "删除人物失败",
                AppError::InternalServerError,
            )?;
        Ok(())
    }
}
