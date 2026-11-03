use serde::{Deserialize, Serialize};
use types_visual::dto::timeline_stat::MonthStat;

/// 管理首页的影像总览统计。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualDashboardStats {
    /// 图片与视频总数。
    pub total_count: u64,
    /// 原始媒体文件的总字节数。
    pub total_bytes: u64,
    /// 按月汇总的影像数量。
    pub monthly_timeline: Vec<MonthStat>,
}
