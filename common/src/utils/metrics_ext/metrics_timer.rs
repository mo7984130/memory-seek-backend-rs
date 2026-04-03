pub struct MetricsTimer {
    name: &'static str,
    start: std::time::Instant,
}
impl MetricsTimer {
    pub fn start(name: &'static str) -> Self {
        Self {
            name,
            start: std::time::Instant::now(),
        }
    }
}
impl Drop for MetricsTimer {
    fn drop(&mut self) {
        metrics::histogram!(self.name).record(self.start.elapsed().as_secs_f64());
    }
}