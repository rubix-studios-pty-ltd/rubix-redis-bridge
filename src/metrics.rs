use anyhow::Context;
use prometheus::{
    Encoder, HistogramOpts, HistogramTimer, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec, Opts, Registry, TextEncoder,
};

#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    pub auth_failed_total: IntCounter,
    pub command_denied_total: IntCounterVec,
    pub redis_operations_total: IntCounterVec,
    pub redis_operation_duration_seconds: HistogramVec,
    pub inflight_redis_operations: IntGaugeVec,
    pub configured_targets: IntGauge,
}

impl Metrics {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();

        let auth_failed_total = IntCounter::with_opts(Opts::new(
            "rrb_auth_failed_total",
            "Total failed bridge authentication attempts.",
        ))?;

        let command_denied_total = IntCounterVec::new(
            Opts::new(
                "rrb_command_denied_total",
                "Total Redis commands denied or rejected by bridge validation.",
            ),
            &["target", "kind"],
        )?;

        let redis_operations_total = IntCounterVec::new(
            Opts::new(
                "rrb_redis_operations_total",
                "Total Redis operations executed by the bridge.",
            ),
            &["target", "kind", "status"],
        )?;

        let redis_operation_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "rrb_redis_operation_duration_seconds",
                "Redis operation duration in seconds.",
            ),
            &["target", "kind"],
        )?;

        let inflight_redis_operations = IntGaugeVec::new(
            Opts::new(
                "rrb_inflight_redis_operations",
                "In-flight Redis operations by target.",
            ),
            &["target", "kind"],
        )?;

        let configured_targets = IntGauge::with_opts(Opts::new(
            "rrb_configured_targets",
            "Number of configured Redis bridge targets.",
        ))?;

        registry.register(Box::new(auth_failed_total.clone()))?;
        registry.register(Box::new(command_denied_total.clone()))?;
        registry.register(Box::new(redis_operations_total.clone()))?;
        registry.register(Box::new(redis_operation_duration_seconds.clone()))?;
        registry.register(Box::new(inflight_redis_operations.clone()))?;
        registry.register(Box::new(configured_targets.clone()))?;

        Ok(Self {
            registry,
            auth_failed_total,
            command_denied_total,
            redis_operations_total,
            redis_operation_duration_seconds,
            inflight_redis_operations,
            configured_targets,
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

    pub fn begin_redis_operation(
        &self,
        target: impl Into<String>,
        kind: &'static str,
    ) -> RedisOperationGuard {
        let target = target.into();

        self.inflight_redis_operations
            .with_label_values(&[target.as_str(), kind])
            .inc();

        let timer = self
            .redis_operation_duration_seconds
            .with_label_values(&[target.as_str(), kind])
            .start_timer();

        RedisOperationGuard {
            metrics: self.clone(),
            target,
            kind,
            timer: Some(timer),
            completed: false,
        }
    }

    pub fn inc_command_denied(&self, target: &str, kind: &str) {
        self.command_denied_total
            .with_label_values(&[target, kind])
            .inc();
    }

    pub fn inc_auth_failed(&self) {
        self.auth_failed_total.inc();
    }
}

pub struct RedisOperationGuard {
    metrics: Metrics,
    target: String,
    kind: &'static str,
    timer: Option<HistogramTimer>,
    completed: bool,
}

impl RedisOperationGuard {
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

impl Drop for RedisOperationGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.metrics
                .redis_operations_total
                .with_label_values(&[self.target.as_str(), self.kind, "cancelled"])
                .inc();
        }

        self.metrics
            .inflight_redis_operations
            .with_label_values(&[self.target.as_str(), self.kind])
            .dec();

        if let Some(timer) = self.timer.take() {
            timer.observe_duration();
        }
    }
}
