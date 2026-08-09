/// 生成时间线月度统计缓存的 Redis 缓存键
///
/// 月度统计为全库聚合结果，整表一条缓存。
///
/// # 返回
/// 格式为 `p:ts:m` 的缓存键
#[inline]
pub fn monthly_stats() -> &'static str {
    //photo:timeline_stat:monthly
    "p:ts:m"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_stats_returns_correct_value() {
        assert_eq!(monthly_stats(), "p:ts:m");
    }
}
