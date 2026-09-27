pub struct GaugeGuard {
    name: String,
}

impl GaugeGuard {
    /// 创建指标守卫并将对应运行中计数加一.
    pub fn start(name: impl Into<String>) -> Self {
        let name = name.into();
        metrics::gauge!(name.clone()).increment(1.0);
        #[cfg(not(feature = "metrics"))]
        let _ = &name;
        Self { name }
    }
}

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        metrics::gauge!(self.name.clone()).decrement(1.0);
    }
}
