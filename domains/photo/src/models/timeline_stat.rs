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

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoTimelineStatVO {
    pub date_str: String,
    pub count: i64,
    pub anchor_time: DateTime<Utc>,
}
