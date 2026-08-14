use types::photo::{face::FaceRecord, models::FaceIds, person::PersonId, photo::PhotoId};

use super::PhotoRepo;
use crate::mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper};
use common::{
    error::contextual::Result,
    error::{AppError, ContextualError},
    models::CursorPage,
};

impl PhotoRepo {
    pub(crate) async fn query_face_compute_photos(
        &self,
        full: bool,
        size: u64,
        previous_id: PhotoId,
    ) -> common::error::contextual::Result<Vec<(PhotoId, String)>> {
        use sea_orm::{
            ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
            sea_query::{Expr, Query},
        };
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
            .all(&self.db)
            .await
            .map_err(Into::into)
    }
    pub(crate) async fn insert_faces(
        &self,
        faces: Vec<types::photo::face::NewFaceRecord>,
    ) -> common::error::contextual::Result<()> {
        use sea_orm::EntityTrait;
        if !faces.is_empty() {
            types::photo::face::Entity::insert_many(
                faces.into_iter().map(types::photo::face::ActiveModel::from),
            )
            .exec_without_returning(&self.db)
            .await?;
        }
        Ok(())
    }
    pub(crate) async fn backup_face_tables(
        &self,
        storage: &backup::storage::BackupStorage,
    ) -> Result<()> {
        use sea_orm::EntityName;
        storage
            .backup_tables(
                &self.db,
                &[
                    types::photo::face::Entity.table_name(),
                    types::photo::person::Entity.table_name(),
                ],
                backup::storage::BackupType::Manual,
            )
            .await
            .map_err(|error| {
                ContextualError::error(
                    "backup_face_tables",
                    "备份人脸与人物表失败",
                    error.to_string(),
                    AppError::InternalServerError,
                )
            })?;
        Ok(())
    }
    pub(crate) async fn query_faces_with_person_names(
        &self,
        photo_id: PhotoId,
    ) -> common::error::contextual::Result<(Vec<FaceRecord>, Vec<(PersonId, String)>)> {
        let faces = FaceMapper::query_by_photo_id(&self.db, photo_id).await?;
        let ids = faces
            .iter()
            .filter_map(|face| face.person_id)
            .collect::<std::collections::HashSet<_>>();
        let names =
            PersonMapper::query_id_and_name_by_ids(&self.db, &ids.into_iter().collect::<Vec<_>>())
                .await?;
        Ok((faces, names))
    }
    pub(crate) async fn query_unassigned_face_photo_ids(
        &self,
        req: &types::photo::dto::face::UnassignedFacePhotoCursorParam,
    ) -> common::error::contextual::Result<CursorPage<PhotoId, ()>> {
        Ok(CursorPage::from_oversize(
            FaceMapper::query_unassigned_face_photo_ids_cursor_page(
                &self.db,
                req.cursor.clone(),
                req.size,
            )
            .await?,
            req.size,
        ))
    }
    pub(crate) async fn delete_unassigned_faces(
        &self,
        ids: &FaceIds,
    ) -> common::error::contextual::Result<u64> {
        FaceMapper::delete_unassigned_by_ids(&self.db, ids).await
    }
    pub(crate) async fn query_person_photo_ids(
        &self,
        person_id: PersonId,
        req: &types::photo::dto::person::PersonPhotoCursorParam,
    ) -> common::error::contextual::Result<CursorPage<PhotoId, ()>> {
        Ok(CursorPage::from_oversize(
            FaceMapper::query_photo_ids_cursor_page(
                &self.db,
                person_id,
                req.cursor.clone(),
                req.size,
            )
            .await?,
            req.size,
        ))
    }
}
