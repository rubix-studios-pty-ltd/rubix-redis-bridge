use prometheus::{
    HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};

pub(super) struct Collectors {
    pub(super) auth_failed_total: IntCounter,
    pub(super) command_denied_total: IntCounterVec,
    pub(super) redis_operations_total: IntCounterVec,
    pub(super) redis_operation_duration: HistogramVec,
    pub(super) redis_operations_inflight: IntGaugeVec,
    pub(super) configured_targets: IntGauge,
}

impl Collectors {
    pub(super) fn new() -> anyhow::Result<Self> {
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

        let redis_operation_duration = HistogramVec::new(
            HistogramOpts::new(
                "rrb_redis_operation_duration_seconds",
                "Redis operation duration in seconds.",
            ),
            &["target", "kind"],
        )?;

        let redis_operations_inflight = IntGaugeVec::new(
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

        Ok(Self {
            auth_failed_total,
            command_denied_total,
            redis_operations_total,
            redis_operation_duration,
            redis_operations_inflight,
            configured_targets,
        })
    }

    pub(super) fn register(&self, registry: &Registry) -> anyhow::Result<()> {
        registry.register(Box::new(self.auth_failed_total.clone()))?;
        registry.register(Box::new(self.command_denied_total.clone()))?;
        registry.register(Box::new(self.redis_operations_total.clone()))?;
        registry.register(Box::new(self.redis_operation_duration.clone()))?;
        registry.register(Box::new(self.redis_operations_inflight.clone()))?;
        registry.register(Box::new(self.configured_targets.clone()))?;

        Ok(())
    }
}
