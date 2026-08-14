use std::time::Duration;

use common::{error::contextual::Result, metrics_name, utils::MetricsTimerExt};
use constants::RedisKeys;
use types::photo::person::PersonId;

use super::PhotoRepo;
use crate::{
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper, photo_mapper::PhotoMapper},
    models::PersonBriefRow,
};

impl PhotoRepo {
    pub(crate) async fn load_faces_with_photo_files(
        &self,
    ) -> common::error::contextual::Result<(
        Vec<types::photo::face::FaceRecord>,
        Vec<(types::photo::photo::PhotoId, String)>,
    )> {
        let faces = FaceMapper::query_all(&self.db).await?;
        let ids = faces
            .iter()
            .map(|face| face.photo_id)
            .collect::<std::collections::HashSet<_>>();
        let files = PhotoMapper::query_id_and_file_id_by_ids(
            &self.db,
            &ids.into_iter().collect::<Vec<_>>(),
        )
        .await?;
        Ok((faces, files))
    }
    pub(crate) async fn load_unassigned_faces_and_persons(
        &self,
    ) -> common::error::contextual::Result<(
        Vec<types::photo::face::FaceRecord>,
        Vec<types::photo::person::PersonRecord>,
    )> {
        Ok((
            FaceMapper::query_unassigned(&self.db).await?,
            PersonMapper::query_all(&self.db).await?,
        ))
    }
    pub(crate) async fn rename_person(
        &self,
        id: PersonId,
        name: String,
        initials: Option<String>,
    ) -> Result<()> {
        use common::error::AppError;
        let rows = PersonMapper::rename(&self.db, id, name, initials).await?;
        if rows == 0 {
            return Err(common::error::ContextualError::warn_without_source(
                "person_rename_fail",
                "重命名人物失败",
                AppError::bad_request("重命名人物失败"),
            ));
        }
        self.invalidate_persons(&[id]).await;
        Ok(())
    }
    pub(crate) async fn query_person(
        &self,
        id: PersonId,
    ) -> common::error::contextual::Result<Option<types::photo::person::PersonRecord>> {
        PersonMapper::query_by_id(&self.db, id).await
    }
    pub(crate) async fn query_person_page(
        &self,
        cursor: Option<types::cursor::FaceCountIdCursor<PersonId>>,
        size: u64,
    ) -> common::error::contextual::Result<Vec<types::photo::person::PersonRecord>> {
        PersonMapper::query(&self.db, cursor, size).await
    }
    pub(crate) async fn search_person_page(
        &self,
        keyword: &str,
        cursor: Option<PersonId>,
        size: u64,
    ) -> common::error::contextual::Result<Vec<types::photo::person::PersonRecord>> {
        PersonMapper::query_search(&self.db, keyword, cursor, size).await
    }
    pub(crate) async fn get_person_briefs(
        &self,
        ids: &[PersonId],
    ) -> common::error::contextual::Result<Vec<Option<PersonBriefRow>>> {
        self.cache_person
            .get_or_load_batch(
                ids,
                |id| RedisKeys::photo::person::person_info(*id),
                Duration::from_secs(24 * 60 * 60),
                |miss_ids| async move { PersonMapper::query_brief_by_ids(&self.db, &miss_ids).await },
                |person| person.id,
            )
            .timed(metrics_name!("cache_get_or_load_batch"))
            .await
    }

    pub(crate) async fn invalidate_persons(&self, ids: &[PersonId]) {
        let keys = ids
            .iter()
            .map(|id| RedisKeys::photo::person::person_info(*id))
            .collect::<Vec<_>>();
        let _ = self.cache_person.invalidate_batch(&keys).await;
    }
}
