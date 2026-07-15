mod collectors;
mod connection;
mod guard;

use anyhow::Context;
use prometheus::{
    Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Registry, TextEncoder,
};

use self::collectors::Collectors;
pub use self::connection::ConnectionGuard;
pub use self::guard::Guard;

#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    pub auth_failed_total: IntCounter,
    pub auth_lockouts_total: IntCounter,
    pub auth_locked_requests_total: IntCounter,
    pub auth_lockout_entry_limit_total: IntCounter,
    pub auth_lockout_tracked_ips: IntGauge,
    pub auth_lockout_locked_ips: IntGauge,
    pub request_denied_total: IntCounterVec,
    pub command_denied_total: IntCounterVec,
    pub redis_operations_total: IntCounterVec,
    pub redis_operation_duration: HistogramVec,
    pub redis_operations_inflight: IntGaugeVec,
    pub realtime_total: IntCounterVec,
    pub realtime_inflight: IntGaugeVec,
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
            auth_lockouts_total: collectors.auth_lockouts_total,
            auth_locked_requests_total: collectors.auth_locked_requests_total,
            auth_lockout_entry_limit_total: collectors.auth_lockout_entry_limit_total,
            auth_lockout_tracked_ips: collectors.auth_lockout_tracked_ips,
            auth_lockout_locked_ips: collectors.auth_lockout_locked_ips,
            request_denied_total: collectors.request_denied_total,
            command_denied_total: collectors.command_denied_total,
            redis_operations_total: collectors.redis_operations_total,
            redis_operation_duration: collectors.redis_operation_duration,
            redis_operations_inflight: collectors.redis_operations_inflight,
            realtime_total: collectors.realtime_total,
            realtime_inflight: collectors.realtime_inflight,
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

    pub fn realtime_connection(&self, target: impl Into<String>) -> ConnectionGuard {
        let target = target.into();

        self.realtime_total
            .with_label_values(&[target.as_str()])
            .inc();
        self.realtime_inflight
            .with_label_values(&[target.as_str()])
            .inc();

        ConnectionGuard::new(self.clone(), target)
    }

    pub fn auth_failed(&self) {
        self.auth_failed_total.inc();
    }

    pub fn lockout_created(&self) {
        self.auth_lockouts_total.inc();
    }

    pub fn locked_request(&self) {
        self.auth_locked_requests_total.inc();
    }

    pub fn lockout_entry_limit(&self) {
        self.auth_lockout_entry_limit_total.inc();
    }

    pub fn set_lockout_state(&self, tracked_ips: usize, locked_ips: usize) {
        self.auth_lockout_tracked_ips.set(tracked_ips as i64);
        self.auth_lockout_locked_ips.set(locked_ips as i64);
    }

    pub fn request_denied(&self, route: &str, reason: &str) {
        self.request_denied_total
            .with_label_values(&[route, reason])
            .inc();
    }
}
