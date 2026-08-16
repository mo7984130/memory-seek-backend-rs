use audit::{AuditEvent, AuditService};
use types::photo::{face::FaceRecord, models::FaceIds, person::PersonId, photo::PhotoId};

use super::PhotoRepo;
use crate::mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper};
use common::{
    error::contextual::Result,
    error::{AppError, ContextualError},
    models::CursorPage,
};

impl PhotoRepo {
    /// 分页查询需要进行人脸计算的照片.
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
    /// 在事务中批量插入检测到的人脸.
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
    /// 备份并清理人脸相关表, 用于全量重算.
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
    /// 查询人脸及其人物名称, 用于人物视图展示.
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
    /// 分页查询包含未分配人脸的照片 ID.
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
    /// 原子删除未分配的人脸记录.
    pub(crate) async fn delete_unassigned_faces(
        &self,
        ids: &FaceIds,
        user_id: types::auth::user::UserId,
    ) -> common::error::contextual::Result<u64> {
        let ids = ids.clone();
        common::db_transaction!(contextual & self.db, |txn| {
            let count = FaceMapper::delete_unassigned_by_ids(txn, &ids).await?;
            for face_id in ids.iter() {
                AuditService::append(
                    txn,
                    AuditEvent::new("face_delete")
                        .with_actor(user_id.0)
                        .with_target("face", face_id.0),
                )
                .await
                .map_err(|error| {
                    ContextualError::error(
                        "photo_audit_append",
                        "人脸删除审计事件写入失败",
                        error,
                        AppError::InternalServerError,
                    )
                })?;
            }
            Ok(count)
        })
        .await
    }
    /// 查询人物关联的照片 ID.
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
