use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get, post},
};
use common::{
    Result,
    ext::ResultRExt,
    extractors::{ValidatedJson, ValidatedPath, ValidatedQuery},
    models::CursorPage,
    r::R,
    traits::controller::ControllerRouter,
};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{
        PersonView,
        dto::person::{
            MergePersonParam, PersonCursorParam, PersonPhotoCursorParam, RenamePersonParam,
        },
        dto::photo::PhotoView,
        person::PersonId,
        photo::PhotoId,
    },
};

use crate::{PhotoState, services::person_service::PersonService};

pub struct PersonController;

impl ControllerRouter for PersonController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/admin/full_scan", get(Self::full_scan))
            .route("/", get(Self::get_persons))
            .route("/merge", post(Self::merge))
            .route("/{person_id}/name", post(Self::rename))
            .route("/{person_id}/photos", get(Self::get_person_photos))
            .route("/{person_id}", delete(Self::delete))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl PersonController {
    pub async fn full_scan(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        let admin = user_id.ensure_admin()?;
        PersonService::full_scan(state, admin).await.to_r_ok()
    }
}

// 修改
impl PersonController {
    /// 重命名人物
    async fn rename(
        State(state): State<Arc<PhotoState>>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
        ValidatedJson(param): ValidatedJson<RenamePersonParam>,
    ) -> Result<R<()>> {
        PersonService::rename_person(&state, person_id, param)
            .await
            .to_r_ok()
    }

    /// 合并人物
    async fn merge(
        State(state): State<Arc<PhotoState>>,
        ValidatedJson(param): ValidatedJson<MergePersonParam>,
    ) -> Result<R<PersonView>> {
        PersonService::merge_person(&state, param).await.to_r_ok()
    }
}

// 查询
impl PersonController {
    pub async fn get_persons(
        State(state): State<Arc<PhotoState>>,
        ValidatedQuery(query): ValidatedQuery<PersonCursorParam>,
    ) -> Result<R<CursorPage<PersonView, PersonId>>> {
        PersonService::get_persons(&state, query).await.to_r_ok()
    }

    /// 获取人物的照片列表(游标分页)
    pub async fn get_person_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
        ValidatedQuery(query): ValidatedQuery<PersonPhotoCursorParam>,
    ) -> Result<R<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>> {
        PersonService::get_person_photos(&state, user_id, person_id, query)
            .await
            .to_r_ok()
    }
}

// 删除
impl PersonController {
    /// 删除人物
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
    ) -> Result<R<()>> {
        PersonService::delete_person(&state, person_id)
            .await
            .to_r_ok()
    }
}
