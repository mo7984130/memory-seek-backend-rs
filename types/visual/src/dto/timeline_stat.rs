use serde::{Deserialize, Serialize};

use crate::timeline_stat::TimelineStatId;

/// 每月影像统计数据
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "orm", derive(sea_orm::FromQueryResult))]
pub struct MonthStat {
    /// 月份字符串，格式为 YYYY-MM
    pub date_str: TimelineStatId,
    /// 该月影像数量
    pub count: i64,
}
