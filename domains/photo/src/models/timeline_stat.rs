use sea_orm::FromQueryResult;
use serde::Serialize;

/// 每月照片统计数据
#[derive(Serialize, FromQueryResult)]
#[serde(rename_all = "camelCase")]
pub struct MonthStat {
    /// 月份字符串，格式为 YYYY-MM
    pub date_str: String,
    /// 该月照片数量
    pub count: i64,
}
