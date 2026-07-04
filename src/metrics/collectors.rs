use prometheus::{
    HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};

pub(super) struct Collectors {
    pub(super) auth_failed_total: IntCounter,
    pub(super) auth_lockouts_total: IntCounter,
    pub(super) auth_locked_requests_total: IntCounter,
    pub(super) auth_lockout_entry_limit_total: IntCounter,
    pub(super) auth_lockout_tracked_ips: IntGauge,
    pub(super) auth_lockout_locked_ips: IntGauge,
    pub(super) request_denied_total: IntCounterVec,
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

        let auth_lockouts_total = IntCounter::with_opts(Opts::new(
            "rrb_auth_lockouts_total",
            "Total client IP lockouts created after repeated failed authentication attempts.",
        ))?;

        let auth_locked_requests_total = IntCounter::with_opts(Opts::new(
            "rrb_auth_locked_requests_total",
            "Total requests rejected because the client IP is currently locked out.",
        ))?;

        let auth_lockout_entry_limit_total = IntCounter::with_opts(Opts::new(
            "rrb_auth_lockout_entry_limit_total",
            "Total failed authentication attempts not tracked because the auth lockout table was full.",
        ))?;

        let auth_lockout_tracked_ips = IntGauge::with_opts(Opts::new(
            "rrb_auth_lockout_tracked_ips",
            "Current number of client IPs tracked by the auth lockout table.",
        ))?;

        let auth_lockout_locked_ips = IntGauge::with_opts(Opts::new(
            "rrb_auth_lockout_locked_ips",
            "Current number of client IPs locked out after failed authentication attempts.",
        ))?;

        let request_denied_total = IntCounterVec::new(
            Opts::new(
                "rrb_request_denied_total",
                "Total requests denied before Redis execution.",
            ),
            &["route", "reason"],
        )?;

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
            auth_lockouts_total,
            auth_locked_requests_total,
            auth_lockout_entry_limit_total,
            auth_lockout_tracked_ips,
            auth_lockout_locked_ips,
            request_denied_total,
            command_denied_total,
            redis_operations_total,
            redis_operation_duration,
            redis_operations_inflight,
            configured_targets,
        })
    }

    pub(super) fn register(&self, registry: &Registry) -> anyhow::Result<()> {
        registry.register(Box::new(self.auth_failed_total.clone()))?;
        registry.register(Box::new(self.auth_lockouts_total.clone()))?;
        registry.register(Box::new(self.auth_locked_requests_total.clone()))?;
        registry.register(Box::new(self.auth_lockout_entry_limit_total.clone()))?;
        registry.register(Box::new(self.auth_lockout_tracked_ips.clone()))?;
        registry.register(Box::new(self.auth_lockout_locked_ips.clone()))?;
        registry.register(Box::new(self.request_denied_total.clone()))?;
        registry.register(Box::new(self.command_denied_total.clone()))?;
        registry.register(Box::new(self.redis_operations_total.clone()))?;
        registry.register(Box::new(self.redis_operation_duration.clone()))?;
        registry.register(Box::new(self.redis_operations_inflight.clone()))?;
        registry.register(Box::new(self.configured_targets.clone()))?;

        Ok(())
    }
}
