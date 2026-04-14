pub struct MetricsConcurrencyGuard {
    name: &'static str,
}

impl MetricsConcurrencyGuard {
    pub fn start(name: &'static str) -> Self {
        metrics::gauge!(name).increment(1.0);
        Self { name }
    }
}

impl Drop for MetricsConcurrencyGuard {
    fn drop(&mut self) {
        metrics::gauge!(self.name).decrement(1.0);
    }
}