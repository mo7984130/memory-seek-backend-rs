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
    auth::user::{AdminId, UserId},
    cursor::{FaceCountIdCursor, TimeIdCursor},
    photo::{
        PersonView,
        dto::person::{
            MergePersonParam, PersonCursorParam, PersonPhotoCursorParam, PersonSearchParam,
            RenamePersonParam, SecondaryClusterParam,
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
            .route("/admin/secondary_cluster", post(Self::secondary_cluster))
            .route("/", get(Self::get_persons))
            .route("/search", get(Self::search_persons))
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
    /// 启动全量人物扫描任务.
    pub async fn full_scan(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<()>> {
        let admin = AdminId::new(user_id)?;
        PersonService::full_scan(state.clone(), admin).await?;

        Ok(()).to_r_ok()
    }

    /// 二次聚类: 将未分配人脸按 centroid 余弦相似度指派到已有人物
    /// 启动未分配人脸的二次聚类任务.
    pub async fn secondary_cluster(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<SecondaryClusterParam>,
    ) -> Result<R<()>> {
        let admin = AdminId::new(user_id)?;
        PersonService::assign_unassigned_faces(state.clone(), admin, req).await?;

        Ok(()).to_r_ok()
    }
}

// 修改
impl PersonController {
    /// 重命名人物
    /// 修改人物名称.
    async fn rename(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
        ValidatedJson(req): ValidatedJson<RenamePersonParam>,
    ) -> Result<R<()>> {
        PersonService::rename_person(&state, person_id, req, user_id).await?;

        Ok(()).to_r_ok()
    }

    /// 合并人物（高危操作，仅管理员）
    /// 合并两个​​人物, 将源人物的人脸转移到目标人物.
    async fn merge(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<MergePersonParam>,
    ) -> Result<R<PersonView>> {
        let admin = AdminId::new(user_id)?;

        let MergePersonParam {
            source_person_id,
            target_person_id,
        } = req;

        let result = PersonService::merge_person(
            &state,
            admin,
            MergePersonParam {
                source_person_id,
                target_person_id,
            },
        )
        .await?;

        Ok(result).to_r_ok()
    }
}

// 查询
impl PersonController {
    /// 查询人物列表.
    pub async fn get_persons(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<PersonCursorParam>,
    ) -> Result<R<CursorPage<PersonView, FaceCountIdCursor<PersonId>>>> {
        PersonService::get_persons(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 按关键词前缀搜索人物(完整名字或姓名首字母)
    /// 按名称或姓名首字母搜索人物.
    pub async fn search_persons(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<PersonSearchParam>,
    ) -> Result<R<CursorPage<PersonView, PersonId>>> {
        PersonService::search_persons(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 获取人物的照片列表(游标分页)
    /// 查询人物关联的照片.
    pub async fn get_person_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
        ValidatedQuery(req): ValidatedQuery<PersonPhotoCursorParam>,
    ) -> Result<R<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>> {
        PersonService::get_person_photos(&state, user_id, person_id, req)
            .await
            .to_r_ok()
    }
}

// 删除
impl PersonController {
    /// 删除人物（高危操作，仅管理员）
    /// 删除人物及其关联数据.
    async fn delete(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(person_id): ValidatedPath<PersonId>,
    ) -> Result<R<()>> {
        let admin = AdminId::new(user_id)?;

        PersonService::delete_person(&state, admin, person_id).await?;

        Ok(()).to_r_ok()
    }
}
