use super::Metrics;

pub struct ConnectionGuard {
    metrics: Metrics,
    target: String,
}

impl ConnectionGuard {
    pub(super) fn new(metrics: Metrics, target: String) -> Self {
        Self { metrics, target }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.metrics
            .realtime_inflight
            .with_label_values(&[self.target.as_str()])
            .dec();
    }
}
