//! 时间线统计 API。
//!
//! 路由前缀:`/visual/timeline`。

use crate::client::{Client, Request};
use crate::error::Result;
use types_visual::MonthStat;

/// 获取每月影像统计。`GET /visual/timeline/stats`
pub async fn get_monthly_stats(client: &Client) -> Result<Vec<MonthStat>> {
    client
        .send_json(Request::get("/visual/timeline/stats"))
        .await
}
