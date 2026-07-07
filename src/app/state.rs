use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Context;
use axum::http::HeaderMap;
use redis::aio::ConnectionManager;
use tokio::sync::{OnceCell, Semaphore, SemaphorePermit};

use crate::auth::AuthLockout;
use crate::client::TrustedProxies;
use crate::config::{Bridge, Redis, TokenHash, TokenTypes};
use crate::metrics::Metrics;
use crate::security::SecurityPolicy;

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
}

pub(crate) struct RedisTarget {
    pub(crate) config: Redis,
    connections: Vec<OnceCell<ConnectionManager>>,
    next_connection: AtomicUsize,
    operation_limit: Semaphore,
}

#[derive(Clone)]
pub(crate) struct AuthRoute {
    target: Arc<RedisTarget>,
    token_type: TokenTypes,
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

            let connections: Vec<OnceCell<ConnectionManager>> = (0..target_config
                .connection_shards)
                .map(|_| OnceCell::new())
                .collect();

            let target = Arc::new(RedisTarget {
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

        let metrics = Metrics::new()?;
        metrics.configured_targets.set(targets.len() as i64);

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
        })
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

    pub(crate) fn has_targets(&self) -> bool {
        !self.targets.is_empty()
    }

    pub(crate) fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub(crate) fn security(&self) -> &SecurityPolicy {
        &self.security
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

    pub(crate) fn token_type(&self) -> &TokenTypes {
        &self.token_type
    }

    //    pub(crate) fn allows_realtime(&self) -> bool {
    //        self.token_type.allows_realtime()
    //    }
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

    pub(crate) async fn connection(&self) -> anyhow::Result<ConnectionManager> {
        let shard = self.next_connection.fetch_add(1, Ordering::Relaxed) % self.connections.len();

        let connection = self.connections[shard]
            .get_or_try_init(|| async {
                let client = redis::Client::open(self.config.connection_string.as_str())
                    .with_context(|| {
                        format!("Invalid Redis URL for target {}", self.config.rrb_id)
                    })?;

                client.get_connection_manager().await.with_context(|| {
                    format!(
                        "Failed to connect to Redis target {} connection shard {}",
                        self.config.rrb_id, shard
                    )
                })
            })
            .await?;

        Ok(connection.clone())
    }

    fn connections_initialized(&self) -> usize {
        self.connections
            .iter()
            .filter(|connection| connection.get().is_some())
            .count()
    }
}
