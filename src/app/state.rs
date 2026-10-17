use anyhow::Context;
use axum::http::HeaderMap;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, SemaphorePermit, TryAcquireError, watch};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::auth::AuthLockout;
use crate::client::TrustedProxies;
use crate::config::{Bridge, Redis, TokenCaps, TokenHash};
use crate::metrics::Metrics;
use crate::security::SecurityPolicy;

#[derive(Clone)]
struct ConnectionSlot {
    generation: u64,
    connection: Option<ConnectionManager>,
}

pub struct AppState {
    pub(crate) targets: Vec<Arc<RedisTarget>>,
    pub(crate) token_routes: HashMap<TokenHash, AuthRoute>,
    pub(crate) security: SecurityPolicy,
    pub(crate) request_timeout: Duration,
    pub(crate) acquire_timeout: Duration,
    pub(crate) max_response_bytes: usize,
    pub(crate) metrics: Metrics,
    pub(crate) metrics_token: Option<String>,
    pub(crate) hash_token: Option<String>,
    pub(crate) auth_lockout: AuthLockout,
    pub(crate) trust_proxy_headers: bool,
    pub(crate) trusted_proxies: TrustedProxies,
    realtime_limit: Arc<Semaphore>,
}

pub(crate) struct RedisTarget {
    pub(crate) config: Redis,
    client: redis::Client,
    connections: Vec<watch::Sender<ConnectionSlot>>,
    connection_changed: watch::Sender<u64>,
    available_shards: AtomicUsize,
    metrics: Metrics,
    next_connection: AtomicUsize,
    operation_limit: Semaphore,
}

#[derive(Clone)]
pub(crate) struct AuthRoute {
    target: Arc<RedisTarget>,
    token_type: TokenCaps,
}

impl fmt::Debug for AppState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppState")
            .field("target_count", &self.targets.len())
            .field("security", &self.security)
            .field("request_timeout", &self.request_timeout)
            .field("acquire_timeout", &self.acquire_timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("trust_proxy_headers", &self.trust_proxy_headers)
            .field("trusted_proxy_count", &self.trusted_proxies.len())
            .field(
                "realtime_permits_available",
                &self.realtime_limit.available_permits(),
            )
            .finish()
    }
}

impl fmt::Debug for RedisTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisTarget")
            .field("rrb_id", &self.config.rrb_id)
            .field("connection_string", &"[redacted]")
            .field("operation_limit", &self.config.operation_limit)
            .field("connection_shards", &self.config.connection_shards)
            .field("connections_initialized", &self.connections_initialized())
            .finish()
    }
}

impl fmt::Debug for AuthRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthRoute")
            .field("target", &self.target.id())
            .field("token_type", &self.token_type)
            .finish()
    }
}

impl AppState {
    pub fn new(config: Bridge) -> anyhow::Result<Self> {
        let metrics = Metrics::new()?;
        metrics.configured_targets.set(config.targets.len() as i64);

        let mut targets = Vec::with_capacity(config.targets.len());
        let mut token_routes = HashMap::new();

        for target_config in config.targets {
            if target_config.operation_limit == 0 {
                anyhow::bail!(
                    "Redis target {} has operation_limit=0",
                    target_config.rrb_id
                );
            }

            if target_config.connection_shards == 0 {
                anyhow::bail!(
                    "Redis target {} has connection_shards=0",
                    target_config.rrb_id
                );
            }

            let client = redis::Client::open(target_config.connection_string.as_str())
                .with_context(|| {
                    format!("Invalid Redis URL for target {}", target_config.rrb_id)
                })?;

            let connections = (0..target_config.connection_shards)
                .map(|_| {
                    watch::channel(ConnectionSlot {
                        generation: 0,
                        connection: None,
                    })
                    .0
                })
                .collect();

            metrics
                .redis_connected_shards
                .with_label_values(&[target_config.rrb_id.as_str()])
                .set(0);

            let target = Arc::new(RedisTarget {
                client,
                connection_changed: watch::channel(0_u64).0,
                available_shards: AtomicUsize::new(0),
                metrics: metrics.clone(),
                operation_limit: Semaphore::new(target_config.operation_limit),
                connections,
                next_connection: AtomicUsize::new(0),
                config: target_config,
            });

            for token in &target.config.tokens {
                if token.enabled
                    && token_routes
                        .insert(
                            token.hash.clone(),
                            AuthRoute {
                                target: target.clone(),
                                token_type: token.token_type.clone(),
                            },
                        )
                        .is_some()
                {
                    anyhow::bail!("Duplicate enabled token hash configured");
                }
            }

            targets.push(target);
        }

        Ok(Self {
            targets,
            token_routes,
            security: config.security,
            request_timeout: config.request_timeout,
            acquire_timeout: config.acquire_timeout,
            max_response_bytes: config.max_response_bytes,
            metrics,
            metrics_token: config.metrics_token,
            hash_token: config.hash_token,
            auth_lockout: AuthLockout::new(
                config.auth_lockout_failures,
                config.auth_lockout_window,
                config.auth_lockout_duration,
                config.auth_lockout_max_entries,
            ),
            trust_proxy_headers: config.trust_proxy_headers,
            trusted_proxies: config.trusted_proxies,
            realtime_limit: Arc::new(Semaphore::new(config.max_realtime_concurrency)),
        })
    }

    pub(crate) fn start_connection_workers(&self) {
        for target in &self.targets {
            for shard in 0..target.connections.len() {
                let target = Arc::clone(target);
                tokio::spawn(async move {
                    target.maintain_connection(shard).await;
                });
            }
        }
    }

    pub(crate) fn targets_ready(&self) -> bool {
        !self.targets.is_empty()
            && self
                .targets
                .iter()
                .all(|target| target.available_shards.load(Ordering::Acquire) > 0)
    }

    pub(crate) fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub(crate) fn acquire_timeout(&self) -> Duration {
        self.acquire_timeout
    }

    pub(crate) fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }

    pub(crate) fn target_count(&self) -> usize {
        self.targets.len()
    }

    pub(crate) fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub(crate) fn security(&self) -> &SecurityPolicy {
        &self.security
    }

    pub(crate) fn acquire_realtime(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        self.realtime_limit.clone().try_acquire_owned()
    }

    pub(crate) fn client_ip(&self, headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
        if !self.trust_proxy_headers {
            return peer.ip();
        }

        self.trusted_proxies.resolve(headers, peer.ip())
    }

    pub(crate) fn base64(headers: &HeaderMap) -> bool {
        headers
            .get("upstash-encoding")
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(',')
                    .any(|entry| entry.trim().eq_ignore_ascii_case("base64"))
            })
            .unwrap_or(false)
    }
}

impl AuthRoute {
    pub(crate) fn target(&self) -> Arc<RedisTarget> {
        self.target.clone()
    }

    pub(crate) fn allows_command_route(&self) -> bool {
        self.token_type.allows_command_route()
    }

    pub(crate) fn token_type(&self) -> &TokenCaps {
        &self.token_type
    }

    pub(crate) fn allows_realtime(&self) -> bool {
        self.token_type.allows_realtime()
    }
}

impl RedisTarget {
    pub(crate) fn id(&self) -> &str {
        self.config.rrb_id.as_str()
    }

    pub(crate) async fn acquire_operation(
        &self,
    ) -> Result<SemaphorePermit<'_>, tokio::sync::AcquireError> {
        self.operation_limit.acquire().await
    }

    pub(crate) async fn connection(&self) -> anyhow::Result<(usize, u64, ConnectionManager)> {
        let mut changes = self.connection_changed.subscribe();

        let start = self.next_connection.fetch_add(1, Ordering::Relaxed);

        loop {
            drop(changes.borrow_and_update());

            let index = select_ready_shard(start, self.connections.len(), |index| {
                self.connections[index].borrow().connection.is_some()
            });

            if let Some(index) = index {
                let slot = self.connections[index].borrow().clone();

                if let Some(connection) = slot.connection {
                    return Ok((index, slot.generation, connection));
                }
            }

            changes
                .changed()
                .await
                .context("Redis connection supervisor stopped")?;
        }
    }

    fn connected(&self, shard: usize, connection: ConnectionManager) {
        self.connections[shard].send_modify(|slot| {
            slot.generation = slot.generation.wrapping_add(1).max(1);
            slot.connection = Some(connection);
        });

        self.metrics
            .redis_connected_shards
            .with_label_values(&[self.id()])
            .inc();

        let available = self.available_shards.fetch_add(1, Ordering::AcqRel) + 1;

        self.connection_changed
            .send_modify(|revision| *revision = revision.wrapping_add(1));

        if available == 1 {
            info!(target = %self.id(), "Redis target available");
        } else {
            debug!(target = %self.id(), shard, "Redis shard connected");
        }
    }

    fn disconnected(&self, shard: usize, generation: u64) -> bool {
        let invalidated = self.connections[shard].send_if_modified(|slot| {
            if slot.generation != generation || slot.connection.is_none() {
                return false;
            }
            slot.connection = None;
            true
        });

        if invalidated {
            self.metrics
                .redis_connected_shards
                .with_label_values(&[self.id()])
                .dec();

            let remaining = self.available_shards.fetch_sub(1, Ordering::AcqRel) - 1;

            self.connection_changed
                .send_modify(|revision| *revision = revision.wrapping_add(1));

            if remaining == 0 {
                warn!(target = %self.id(), "All Redis shards unavailable; reconnecting");
            } else {
                debug!(target = %self.id(), shard, "Redis shard unhealthy; reconnecting");
            }
        }

        invalidated
    }

    async fn maintain_connection(self: Arc<Self>, shard: usize) {
        const PROBE_INTERVAL: Duration = Duration::from_secs(5);
        const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
        const MAX_BACKOFF: Duration = Duration::from_secs(5);
        const INITIAL_BACKOFF: Duration = Duration::from_millis(250);
        const MAX_FAILED_PROBES: u8 = 2;

        if shard > 0 {
            sleep(Duration::from_millis((shard as u64 * 50) % 750)).await;
        }

        let mut receiver = self.connections[shard].subscribe();
        let mut backoff = INITIAL_BACKOFF;
        let mut failures = 0_u64;
        let mut failed_probes = 0_u8;

        loop {
            let slot = receiver.borrow_and_update().clone();

            if let Some(mut connection) = slot.connection {
                tokio::select! {
                    changed = receiver.changed() => {
                        if changed.is_err() {
                            return;
                        }
                    }
                    _ = sleep(PROBE_INTERVAL) => {
                        let result: Result<redis::RedisResult<String>, _> = tokio::time::timeout(
                            PROBE_TIMEOUT,
                            redis::cmd("PING").query_async(&mut connection),
                        ).await;

                        if matches!(result, Ok(Ok(ref pong)) if pong == "PONG") {
                            failed_probes = 0;
                        } else {
                            failed_probes = failed_probes.saturating_add(1);

                            if failed_probes >= MAX_FAILED_PROBES {
                                self.disconnected(shard, slot.generation);
                                failed_probes = 0;
                            }
                        }
                    }
                }
                continue;
            }

            let config = ConnectionManagerConfig::new()
                .set_connection_timeout(Some(Duration::from_secs(1)))
                .set_number_of_retries(2)
                .set_min_delay(Duration::from_millis(100))
                .set_max_delay(Duration::from_millis(250));

            let connection = self.client.get_connection_manager_with_config(config).await;

            match connection {
                Ok(connection) => {
                    self.metrics
                        .redis_connection_attempts
                        .with_label_values(&[self.id(), "success"])
                        .inc();
                    self.connected(shard, connection);
                    backoff = INITIAL_BACKOFF;
                    failures = 0;
                    failed_probes = 0;
                }
                Err(error) => {
                    self.metrics
                        .redis_connection_attempts
                        .with_label_values(&[self.id(), "error"])
                        .inc();
                    failures = failures.saturating_add(1);

                    if shard == 0 && (failures == 1 || failures.is_multiple_of(12)) {
                        warn!(target = %self.id(), failures, %error, "Redis connection unavailable; retrying");
                    }

                    sleep(backoff).await;
                    backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
                }
            }
        }
    }

    pub(crate) async fn pubsub(&self) -> anyhow::Result<redis::aio::PubSub> {
        self.client.get_async_pubsub().await.with_context(|| {
            format!(
                "Failed to connect realtime subscription for Redis target {}",
                self.config.rrb_id
            )
        })
    }

    fn connections_initialized(&self) -> usize {
        self.available_shards.load(Ordering::Acquire)
    }
}

pub(crate) fn select_ready_shard(
    start: usize,
    len: usize,
    ready: impl Fn(usize) -> bool,
) -> Option<usize> {
    (0..len)
        .map(|offset| (start % len + offset) % len)
        .find(|&index| ready(index))
}
