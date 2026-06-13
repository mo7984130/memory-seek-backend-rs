/// 并发度监控守卫
///
/// 通过 RAII 模式自动跟踪当前并发执行数量。创建时递增 gauge 指标，
/// 销毁时递减，适用于监控同时活跃的请求数或任务数。
pub struct MetricsConcurrencyGuard {
    name: &'static str,
}

impl MetricsConcurrencyGuard {
    /// 创建并发度守卫并递增指定的 gauge 指标
    ///
    /// # 参数
    /// - `name`: metrics gauge 指标名称（静态字符串）
    ///
    /// # 返回
    /// 返回守卫实例，当实例被 drop 时会自动递减指标
    pub fn start(name: &'static str) -> Self {
        metrics::gauge!(name).increment(1.0);
        Self { name }
    }
}

impl Drop for MetricsConcurrencyGuard {
    /// 守卫销毁时递减 gauge 指标，表示一个并发单元已释放
    fn drop(&mut self) {
        metrics::gauge!(self.name).decrement(1.0);
    }
}
