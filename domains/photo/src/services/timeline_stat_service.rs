use crate::{
    mappers::timeline_stat_mapper::TimelineStatMapper, models::timeline_stat::MonthStat,
    state::PhotoState,
};
use common::{Result, metrics_group, metrics_success, metrics_timer_name, utils::MetricsTimerExt};

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    pub async fn get_monthly_stats(state: &PhotoState) -> Result<Vec<MonthStat>> {
        metrics_group!("get_monthly_stats");

        let res = TimelineStatMapper::query_monthly_stats(&state.db)
            .timed(metrics_timer_name!("get_monthly_stats", "query_monthly_stats"))
            .await;

        metrics_success!("get_monthly_stats");
        res
    }
}
