use crate::{mappers::timeline_stat_mapper::TimelineStatMapper, state::PhotoState};
use common::{Result, metrics_group, metrics_name, metrics_success, utils::MetricsTimerExt};
use types::photo::dto::timeline_stat::MonthStat;

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    #[tracing::instrument(skip_all)]
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        metrics_group!();

        let res = TimelineStatMapper::query_monthly_stats(&state.db)
            .timed(metrics_name!("query_monthly_stats"))
            .await;

        metrics_success!();
        res
    }
}
