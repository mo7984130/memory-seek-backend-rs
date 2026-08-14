use std::sync::Arc;

use axum::{Extension, Router, extract::State, routing::get};
use common::{
    Result, ext::ResultRExt, extractors::ValidatedQuery, models::CursorPage, r::R,
    traits::controller::ControllerRouter,
};
use types::auth::user::{AdminId, UserId};
use types::cursor::TimeIdCursor;
use types::photo::behavior::UserBehaviorId;
use types::photo::dto::behavior::{
    BehaviorAuditItem, BehaviorAuditQuery, BehaviorStatsItem, BehaviorStatsQuery, BehaviorTopItem,
    BehaviorTopQuery,
};

use crate::{services::behavior_service::BehaviorService, state::PhotoState};

pub struct BehaviorController;

impl ControllerRouter for BehaviorController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .route("/stats", get(Self::stats))
            .route("/top", get(Self::top))
            .route("/audit", get(Self::audit))
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new()
    }
}

// 管理端统计
impl BehaviorController {
    /// 行为量时序统计（按日/周/月聚合）
    async fn stats(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<BehaviorStatsQuery>,
    ) -> Result<R<Vec<BehaviorStatsItem>>> {
        let admin = AdminId::new(user_id)?;
        BehaviorService::get_stats(&state, admin, req)
            .await
            .to_r_ok()
    }

    /// 热门目标排行（如浏览量 Top N 照片）
    async fn top(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<BehaviorTopQuery>,
    ) -> Result<R<Vec<BehaviorTopItem>>> {
        let admin = AdminId::new(user_id)?;
        BehaviorService::get_top(&state, admin, req).await.to_r_ok()
    }

    /// 审计流水（可按照片/人物/动作追溯，含 IP 与详情）
    async fn audit(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<BehaviorAuditQuery>,
    ) -> Result<R<CursorPage<BehaviorAuditItem, TimeIdCursor<UserBehaviorId>>>> {
        let admin = AdminId::new(user_id)?;
        BehaviorService::get_audit(&state, admin, req)
            .await
            .to_r_ok()
    }
}
