/// 生成时间线月度统计的 Redis 缓存键；月度统计为全库聚合，整表共用一条缓存
#[inline]
pub fn monthly_stats() -> &'static str {
    //visual:timeline_stat:monthly
    "v:ts:m"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_stats_returns_correct_value() {
        assert_eq!(monthly_stats(), "v:ts:m");
    }
}
