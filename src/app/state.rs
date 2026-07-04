use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::http::HeaderMap;
use redis::aio::ConnectionManager;
use tokio::sync::{OnceCell, Semaphore, SemaphorePermit};

use crate::client::TrustedProxies;
use crate::config::{BridgeConfig, RedisTargetConfig};
use crate::metrics::Metrics;
use crate::security::SecurityPolicy;

use super::lockout::AuthLockout;

pub struct AppState {
    pub(crate) targets: HashMap<String, Arc<RedisTarget>>,
    pub(crate) security: SecurityPolicy,
    pub(crate) request_timeout: Duration,
    pub(crate) metrics: Metrics,
    pub(crate) metrics_token: Option<String>,
    pub(crate) auth_lockout: AuthLockout,
    pub(crate) trust_proxy_headers: bool,
    pub(crate) trusted_proxies: TrustedProxies,
}

pub(crate) struct RedisTarget {
    pub(crate) config: RedisTargetConfig,
    connection: OnceCell<ConnectionManager>,
    operation_limit: Semaphore,
}

impl fmt::Debug for AppState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppState")
            .field("target_count", &self.targets.len())
            .field("security", &self.security)
            .field("request_timeout", &self.request_timeout)
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
            .field("max_connections", &self.config.max_connections)
            .field("connection_initialized", &self.connection.get().is_some())
            .field("operation_limit", &self.config.max_connections)
            .finish()
    }
}

impl AppState {
    pub fn new(config: BridgeConfig) -> anyhow::Result<Self> {
        let targets: HashMap<String, Arc<RedisTarget>> = config
            .targets
            .into_iter()
            .map(|(token, target_config)| {
                (
                    token,
                    Arc::new(RedisTarget {
                        operation_limit: Semaphore::new(target_config.max_connections),
                        config: target_config,
                        connection: OnceCell::new(),
                    }),
                )
            })
            .collect();

        let metrics = Metrics::new()?;
        metrics.configured_targets.set(targets.len() as i64);

        Ok(Self {
            targets,
            security: config.security,
            request_timeout: config.request_timeout,
            metrics,
            metrics_token: config.metrics_token,
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
        let connection = self
            .connection
            .get_or_try_init(|| async {
                let client = redis::Client::open(self.config.connection_string.as_str())
                    .with_context(|| {
                        format!("Invalid Redis URL for target {}", self.config.rrb_id)
                    })?;

                client.get_connection_manager().await.with_context(|| {
                    format!("Failed to connect to Redis target {}", self.config.rrb_id)
                })
            })
            .await?;

        Ok(connection.clone())
    }
}
