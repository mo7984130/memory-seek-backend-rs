use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{get, post},
};
use common_core::{Result, types::CursorPage};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedJson,
    extractors::ValidatedPath, extractors::ValidatedQuery,
};
use types_core::cursor::{CountIdCursor, TimeIdCursor};
use types_identity::auth::user::UserId;
use types_visual::{
    PersonView, dto::person::PersonCursorParam, dto::person::PersonSearchParam,
    dto::person::PersonVisualCursorParam, dto::person::RenamePersonParam, dto::visual::VisualView,
    person::PersonId, visual::VisualId,
};

use crate::{VisualState, services::person_service::PersonService};

pub struct PersonController;

impl ControllerRouter for PersonController {
    type State = VisualState;

    /// 人物模块的受保护路由
    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/", get(Self::get_persons))
            .route("/search", get(Self::search_persons))
            .route("/{person_id}/name", post(Self::rename))
            .route("/{person_id}/visuals", get(Self::get_person_visuals))
    }

    /// 人物模块无公开路由
    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 修改
impl PersonController {
    /// 重命名人物
    async fn rename(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
        ValidatedJson(req): ValidatedJson<RenamePersonParam>,
    ) -> Result<R<()>> {
        PersonService::rename_person(&state, person_id, req, user_id).await?;

        Ok(()).to_r_ok()
    }
}

// 查询
impl PersonController {
    /// 查询人物列表
    pub async fn get_persons(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<PersonCursorParam>,
    ) -> Result<R<CursorPage<PersonView, CountIdCursor<PersonId>>>> {
        PersonService::get_persons(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 按完整名字或姓名首字母前缀搜索人物
    pub async fn search_persons(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<PersonSearchParam>,
    ) -> Result<R<CursorPage<PersonView, PersonId>>> {
        PersonService::search_persons(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 游标分页查询人物关联的照片
    pub async fn get_person_visuals(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
        ValidatedQuery(req): ValidatedQuery<PersonVisualCursorParam>,
    ) -> Result<R<CursorPage<VisualView, TimeIdCursor<VisualId>>>> {
        PersonService::get_person_visuals(&state, user_id, person_id, req)
            .await
            .to_r_ok()
    }
}
