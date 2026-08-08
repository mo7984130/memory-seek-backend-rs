use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::State,
    routing::{delete, get, post},
};
use common::{
    Result,
    ext::ResultRExt,
    extractors::{OptionalClientIp, ValidatedJson, ValidatedPath, ValidatedQuery},
    models::CursorPage,
    r::R,
    traits::controller::ControllerRouter,
};
use types::{
    auth::user::{AdminId, UserId},
    cursor::TimeIdCursor,
    photo::{
        FaceView,
        behavior::{BehaviorTargetType, UserBehaviorAction},
        dto::face::{
            FaceDeleteBatchParam, FaceDeleteBatchResult, UnassignedFacePhotoCursorParam,
        },
        dto::photo::PhotoView,
        face::FaceId,
        person::PersonId,
        photo::PhotoId,
    },
};

use crate::{
    PhotoState,
    services::{
        behavior_service::{BehaviorRecordReq, BehaviorService},
        face_service::FaceService,
    },
};

pub struct FaceController;

impl ControllerRouter for FaceController {
    type State = PhotoState;

    fn protected_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
            .route("/admin/full", get(Self::full_compute))
            .route("/admin/incremental", get(Self::incremental_compute))
            .route("/photo/{photo_id}", get(Self::get_faces_by_photo_id))
            .route("/unassigned-photos", get(Self::get_unassigned_face_photos))
            .route(
                "/feature/{feature_id}/belonging/{person_id}",
                post(Self::change_belonging),
            )
            .route("/feature/{feature_id}/belonging", post(Self::unassign_face))
            .route("/feature/{feature_id}", delete(Self::delete_face))
            .route("/feature", delete(Self::delete_faces_batch))
    }

    fn public_routes() -> axum::Router<std::sync::Arc<Self::State>> {
        Router::new()
    }
}

// 创建
impl FaceController {
    async fn full_compute(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
    ) -> Result<R<()>> {
        let admin = AdminId::new(user_id)?;
        FaceService::compute(state.clone(), admin, true).await?;

        // 行为审计：全量人脸计算
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::FaceCompute)
                .with_detail(serde_json::json!({ "full": true }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }

    async fn incremental_compute(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
    ) -> Result<R<()>> {
        let admin = AdminId::new(user_id)?;
        FaceService::compute(state.clone(), admin, false).await?;

        // 行为审计：增量人脸计算
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::FaceCompute)
                .with_detail(serde_json::json!({ "full": false }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }
}

// 修改
impl FaceController {
    /// 修改人脸归属: 将单张人脸移动到指定人物
    async fn change_belonging(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath((face_id, person_id)): ValidatedPath<(FaceId, PersonId)>,
    ) -> Result<R<()>> {
        FaceService::change_face_belonging(&state, face_id, Some(person_id)).await?;

        // 行为审计：修改人脸归属
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::FaceChangeBelonging)
                .with_target(BehaviorTargetType::Face, face_id.0)
                .with_detail(serde_json::json!({ "toPersonId": person_id.0 }))
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }

    /// 取消人脸归属(路径不带 person_id 段)
    async fn unassign_face(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath(face_id): ValidatedPath<FaceId>,
    ) -> Result<R<()>> {
        FaceService::change_face_belonging(&state, face_id, None).await?;

        // 行为审计：取消人脸归属
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::FaceUnassign)
                .with_target(BehaviorTargetType::Face, face_id.0)
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }
}

// 查询
impl FaceController {
    async fn get_faces_by_photo_id(
        State(state): State<Arc<PhotoState>>,
        ValidatedPath(photo_id): ValidatedPath<PhotoId>,
    ) -> Result<R<Vec<FaceView>>> {
        FaceService::get_faces_by_photo_id(&state, photo_id)
            .await
            .to_r_ok()
    }

    /// 获取"包含未分配人脸"的照片列表(游标分页, 不区分照片归属者)
    async fn get_unassigned_face_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(param): ValidatedQuery<UnassignedFacePhotoCursorParam>,
    ) -> Result<R<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>> {
        FaceService::get_unassigned_face_photos(&state, user_id, param)
            .await
            .to_r_ok()
    }
}

// 删除
impl FaceController {
    /// 删除人脸(仅限未归属人物的人脸)
    async fn delete_face(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedPath(face_id): ValidatedPath<FaceId>,
    ) -> Result<R<()>> {
        FaceService::delete_face(&state, face_id).await?;

        // 行为审计：删除人脸（记录保留，人脸删除后仍可追溯）
        BehaviorService::record(
            &state,
            BehaviorRecordReq::new(user_id, UserBehaviorAction::FaceDelete)
                .with_target(BehaviorTargetType::Face, face_id.0)
                .with_ip(ip.map(|ip| ip.to_string())),
        )
        .await;

        Ok(()).to_r_ok()
    }

    /// 批量删除人脸(仅限未归属人物的人脸, 已归属人脸会被跳过)
    async fn delete_faces_batch(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        OptionalClientIp(ip): OptionalClientIp,
        ValidatedJson(param): ValidatedJson<FaceDeleteBatchParam>,
    ) -> Result<R<FaceDeleteBatchResult>> {
        let result = FaceService::delete_faces_batch(&state, &param.face_ids).await?;

        // 行为审计：批量删除人脸（按批量传入的 face_id 逐条记录）
        let ip_str = ip.map(|ip| ip.to_string());
        for face_id in param.face_ids.iter() {
            BehaviorService::record(
                &state,
                BehaviorRecordReq::new(user_id, UserBehaviorAction::FaceDelete)
                    .with_target(BehaviorTargetType::Face, face_id.0)
                    .with_ip(ip_str.clone()),
            )
            .await;
        }

        Ok(result).to_r_ok()
    }
}
