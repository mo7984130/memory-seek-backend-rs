/// 执行耗时监控计时器
///
/// 通过 RAII 模式自动记录代码块的执行耗时。创建时记录起始时间，
/// 销毁时将耗时写入 histogram 指标，适用于监控函数或请求的处理时长。
/// 未启用 metrics feature 时为空操作（零成本 ZST）。
pub struct MetricsTimer {
    #[cfg(feature = "metrics")]
    name: String,
    #[cfg(feature = "metrics")]
    start: std::time::Instant,
}

impl MetricsTimer {
    /// 创建计时器并记录当前时间作为起始点
    ///
    /// 未启用 metrics feature 时返回零尺寸空实例。
    pub fn start(name: impl Into<String>) -> Self {
        let name = name.into();
        #[cfg(not(feature = "metrics"))]
        let _ = &name;
        Self {
            #[cfg(feature = "metrics")]
            name,
            #[cfg(feature = "metrics")]
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for MetricsTimer {
    /// 计时器销毁时将从创建到销毁的耗时记录到 histogram 指标
    fn drop(&mut self) {
        #[cfg(feature = "metrics")]
        metrics::histogram!(self.name.clone()).record(self.start.elapsed().as_secs_f64());
    }
}
