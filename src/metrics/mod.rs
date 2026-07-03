mod collectors;
mod guard;

use anyhow::Context;
use prometheus::{
    Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Registry, TextEncoder,
};

use self::collectors::Collectors;
pub use self::guard::Guard;

#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    pub auth_failed_total: IntCounter,
    pub command_denied_total: IntCounterVec,
    pub redis_operations_total: IntCounterVec,
    pub redis_operation_duration: HistogramVec,
    pub redis_operations_inflight: IntGaugeVec,
    pub configured_targets: IntGauge,
}

impl Metrics {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();
        let collectors = Collectors::new()?;

        collectors.register(&registry)?;

        Ok(Self {
            registry,
            auth_failed_total: collectors.auth_failed_total,
            command_denied_total: collectors.command_denied_total,
            redis_operations_total: collectors.redis_operations_total,
            redis_operation_duration: collectors.redis_operation_duration,
            redis_operations_inflight: collectors.redis_operations_inflight,
            configured_targets: collectors.configured_targets,
        })
    }

    pub fn render(&self) -> anyhow::Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();

        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .context("Failed to encode Prometheus metrics")?;

        String::from_utf8(buffer).context("Prometheus metrics output was not valid UTF-8")
    }

    pub fn content_type() -> &'static str {
        "text/plain; version=0.0.4; charset=utf-8"
    }

    pub fn begin_operation(&self, target: impl Into<String>, kind: &'static str) -> Guard {
        let target = target.into();

        self.redis_operations_inflight
            .with_label_values(&[target.as_str(), kind])
            .inc();

        let timer = self
            .redis_operation_duration
            .with_label_values(&[target.as_str(), kind])
            .start_timer();

        Guard::new(self.clone(), target, kind, timer)
    }

    pub fn command_denied(&self, target: &str, kind: &str) {
        self.command_denied_total
            .with_label_values(&[target, kind])
            .inc();
    }

    pub fn auth_failed(&self) {
        self.auth_failed_total.inc();
    }
}
