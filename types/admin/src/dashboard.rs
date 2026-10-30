use serde::{Deserialize, Serialize};
use types_visual::dto::timeline_stat::MonthStat;

/// 管理首页的影像总览统计。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "admin/"))]
pub struct VisualDashboardStats {
    /// 图片与视频总数。
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_count: u64,
    /// 原始媒体文件的总字节数。
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_bytes: u64,
    /// 按月汇总的影像数量。
    pub monthly_timeline: Vec<MonthStat>,
}
