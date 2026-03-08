use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoTimelineStatVO {
    pub date_str: String,
    pub count: i32,
    pub anchor_time: DateTime<Utc>,
}
