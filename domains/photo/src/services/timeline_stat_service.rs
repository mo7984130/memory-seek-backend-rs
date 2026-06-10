use crate::{
    mappers::timeline_stat_mapper::TimelineStatMapper, models::timeline_stat::TimeRange,
    state::PhotoState,
};
use common::{Result, metrics_group, metrics_success};

pub(crate) struct TimelineStatService;

impl TimelineStatService {
    pub async fn get_time_range(state: &PhotoState) -> Result<TimeRange> {
        metrics_group!("get_time_range");

        let res = TimelineStatMapper::query_time_range(&state.db).await;

        metrics_success!("get_time_range");
        res
    }
}
