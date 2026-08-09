use serde::{Deserialize, Serialize};

use crate::photo::timeline_stat::TimelineStatId;

/// 每月照片统计数据
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/"))]
#[cfg_attr(feature = "orm", derive(sea_orm::FromQueryResult))]
pub struct MonthStat {
    /// 月份字符串，格式为 YYYY-MM
    pub date_str: TimelineStatId,
    /// 该月照片数量
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
}
