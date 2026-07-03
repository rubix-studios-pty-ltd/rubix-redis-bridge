use prometheus::HistogramTimer;

use super::Metrics;

pub struct Guard {
    metrics: Metrics,
    target: String,
    kind: &'static str,
    timer: Option<HistogramTimer>,
    completed: bool,
}

impl Guard {
    pub(super) fn new(
        metrics: Metrics,
        target: String,
        kind: &'static str,
        timer: HistogramTimer,
    ) -> Self {
        Self {
            metrics,
            target,
            kind,
            timer: Some(timer),
            completed: false,
        }
    }

    pub fn success(mut self) {
        self.completed = true;
        self.metrics
            .redis_operations_total
            .with_label_values(&[self.target.as_str(), self.kind, "ok"])
            .inc();
    }

    pub fn error(mut self) {
        self.completed = true;
        self.metrics
            .redis_operations_total
            .with_label_values(&[self.target.as_str(), self.kind, "error"])
            .inc();
    }

    pub fn timeout(mut self) {
        self.completed = true;
        self.metrics
            .redis_operations_total
            .with_label_values(&[self.target.as_str(), self.kind, "timeout"])
            .inc();
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.completed {
            self.metrics
                .redis_operations_total
                .with_label_values(&[self.target.as_str(), self.kind, "cancelled"])
                .inc();
        }

        self.metrics
            .redis_operations_inflight
            .with_label_values(&[self.target.as_str(), self.kind])
            .dec();

        if let Some(timer) = self.timer.take() {
            timer.observe_duration();
        }
    }
}
