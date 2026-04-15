use axum::extract::State;
use axum::routing::get;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use crate::state::AppState;
use crate::models::common::PhotoTimelineStatVO;
use crate::services::timeline_stat_service::TimelineStatService;

pub struct TimelineController;

impl TimelineController {
    pub fn routes() -> Router<Arc<AppState>> {
        Router::new().route("/stats", get(Self::get_stats))
    }

    async fn get_stats(
        State(state): State<Arc<AppState>>,
    ) -> Result<R<Vec<PhotoTimelineStatVO>>, AppError> {
        let stats = TimelineStatService::get_stats(&state.db).await?;
        Ok(R::ok(stats))
    }
}
