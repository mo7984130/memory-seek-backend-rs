use sea_orm::{FromQueryResult, entity::prelude::DateTimeUtc};
use serde::Serialize;

#[derive(Serialize, FromQueryResult)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub min_time: Option<DateTimeUtc>,
    pub max_time: Option<DateTimeUtc>,
}
impl Default for TimeRange {
    /// 创建时间范围实例，最小和最大时间均为 `None`
    fn default() -> Self {
        Self {
            min_time: None,
            max_time: None,
        }
    }
}

