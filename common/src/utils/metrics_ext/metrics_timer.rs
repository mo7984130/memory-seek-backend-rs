/// 执行耗时监控计时器
///
/// 通过 RAII 模式自动记录代码块的执行耗时。创建时记录起始时间，
/// 销毁时将耗时写入 histogram 指标，适用于监控函数或请求的处理时长。
pub struct MetricsTimer {
    name: &'static str,
    start: std::time::Instant,
}

impl MetricsTimer {
    /// 创建计时器并记录当前时间作为起始点
    ///
    /// # 参数
    /// - `name`: metrics histogram 指标名称（静态字符串）
    ///
    /// # 返回
    /// 返回计时器实例，当实例被 drop 时会自动记录耗时
    pub fn start(name: &'static str) -> Self {
        Self {
            name,
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for MetricsTimer {
    /// 计时器销毁时将从创建到销毁的耗时记录到 histogram 指标
    fn drop(&mut self) {
        metrics::histogram!(self.name).record(self.start.elapsed().as_secs_f64());
    }
}
