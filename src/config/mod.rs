mod env;
mod targets;

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use anyhow::bail;
use serde::Deserialize;

use crate::client::TrustedProxies;
use crate::commands::{ALLOWED_COMMANDS, DENIED_COMMANDS, RATELIMIT_COMMANDS};
use crate::security::SecurityPolicy;

use self::env::{
    env_first, env_or, parse_bool_env, parse_command_list, parse_csv_env_first,
    parse_env_or_default,
};
use self::targets::load_targets;

#[derive(Clone)]
pub struct BridgeConfig {
    pub host: String,
    pub port: u16,
    pub targets: HashMap<String, RedisTargetConfig>,
    pub security: SecurityPolicy,
    pub max_body_bytes: usize,
    pub max_concurrency: usize,
    pub request_timeout: Duration,
    pub metrics_token: Option<String>,
    pub auth_lockout_failures: usize,
    pub auth_lockout_window: Duration,
    pub auth_lockout_duration: Duration,
    pub auth_lockout_max_entries: usize,
    pub trust_proxy_headers: bool,
    pub trusted_proxies: TrustedProxies,
}

#[derive(Clone, Deserialize)]
pub struct RedisTargetConfig {
    #[serde(default = "default_rrb_id")]
    pub rrb_id: String,
    pub connection_string: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

impl fmt::Debug for BridgeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BridgeConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("target_count", &self.targets.len())
            .field("security", &self.security)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("max_concurrency", &self.max_concurrency)
            .field("request_timeout", &self.request_timeout)
            .field("metrics_token_configured", &self.metrics_token.is_some())
            .field("auth_lockout_failures", &self.auth_lockout_failures)
            .field("auth_lockout_window", &self.auth_lockout_window)
            .field("auth_lockout_duration", &self.auth_lockout_duration)
            .field("auth_lockout_max_entries", &self.auth_lockout_max_entries)
            .field("trust_proxy_headers", &self.trust_proxy_headers)
            .field("trusted_proxy_count", &self.trusted_proxies.len())
            .finish()
    }
}

impl fmt::Debug for RedisTargetConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisTargetConfig")
            .field("rrb_id", &self.rrb_id)
            .field("connection_string", &"[redacted]")
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

impl BridgeConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let host = env_or("RRB_HOST", "0.0.0.0");
        let port = parse_env_or_default("RRB_PORT", 8080)?;
        let max_body_bytes = parse_env_or_default("RRB_MAX_BODY_BYTES", 1024 * 1024)?;
        let max_concurrency = parse_env_or_default("RRB_MAX_CONCURRENCY", 1024)?;
        let max_pipeline_commands = parse_env_or_default("RRB_MAX_PIPELINE_COMMANDS", 1000)?;
        let max_command_args = parse_env_or_default("RRB_MAX_COMMAND_ARGS", 256)?;
        let max_arg_bytes = parse_env_or_default("RRB_MAX_ARG_BYTES", 256 * 1024)?;
        let request_timeout_ms: u64 = parse_env_or_default("RRB_REQUEST_TIMEOUT_MS", 5_000)?;
        let upstash_ratelimit = parse_bool_env("RRB_UPSTASH_RATELIMIT", false)?;
        let auth_lockout_failures = parse_env_or_default("RRB_AUTH_LOCKOUT_FAILURES", 10)?;
        let auth_lockout_window_seconds: u64 =
            parse_env_or_default("RRB_AUTH_LOCKOUT_WINDOW_SECONDS", 300)?;
        let auth_lockout_seconds: u64 = parse_env_or_default("RRB_AUTH_LOCKOUT_SECONDS", 300)?;
        let auth_lockout_max_entries =
            parse_env_or_default("RRB_AUTH_LOCKOUT_MAX_ENTRIES", 65_536)?;
        let trust_proxy_headers = parse_bool_env("RRB_TRUST_PROXY_HEADERS", false)?;
        let trusted_proxies_value = env_first(&["RRB_TRUSTED_PROXIES"]);
        let trusted_proxies = trusted_proxies_value
            .as_deref()
            .map(TrustedProxies::parse)
            .transpose()?
            .unwrap_or_default();

        if trust_proxy_headers && trusted_proxies.is_empty() {
            bail!(
                "RRB_TRUSTED_PROXIES must include at least one IP or CIDR when RRB_TRUST_PROXY_HEADERS=true"
            );
        }

        if !trust_proxy_headers && trusted_proxies_value.is_some() {
            bail!(
                "RRB_TRUST_PROXY_HEADERS=true is required when RRB_TRUSTED_PROXIES is configured"
            );
        }

        let mut allowed_commands = parse_csv_env_first(&["RRB_ALLOWED_COMMANDS"])?
            .unwrap_or_else(|| parse_command_list(ALLOWED_COMMANDS));

        let mut blocked_commands = parse_command_list(DENIED_COMMANDS);

        if let Some(extra_blocked_commands) = parse_csv_env_first(&["RRB_BLOCKED_COMMANDS"])? {
            blocked_commands.extend(extra_blocked_commands);
        }

        if upstash_ratelimit {
            for &command in RATELIMIT_COMMANDS {
                allowed_commands.insert(command.to_string());
                blocked_commands.remove(command);
            }
        }

        if auth_lockout_failures > 0 && auth_lockout_window_seconds == 0 {
            bail!(
                "RRB_AUTH_LOCKOUT_WINDOW_SECONDS must be greater than zero when auth lockout is enabled"
            );
        }

        if auth_lockout_failures > 0 && auth_lockout_seconds == 0 {
            bail!(
                "RRB_AUTH_LOCKOUT_SECONDS must be greater than zero when auth lockout is enabled"
            );
        }

        if auth_lockout_failures > 0 && auth_lockout_max_entries == 0 {
            bail!(
                "RRB_AUTH_LOCKOUT_MAX_ENTRIES must be greater than zero when auth lockout is enabled"
            );
        }

        let security = SecurityPolicy {
            allowed_commands,
            blocked_commands,
            max_pipeline_commands,
            max_command_args,
            max_arg_bytes,
            upstash_ratelimit,
        };

        security.validate()?;

        if request_timeout_ms == 0 {
            bail!("RRB_REQUEST_TIMEOUT_MS must be greater than zero");
        }

        let metrics_token = env_first(&["RRB_METRICS_TOKEN"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let targets = load_targets()?;
        if targets.is_empty() {
            bail!("No Redis targets configured");
        }

        Ok(Self {
            host,
            port,
            targets,
            security,
            max_body_bytes,
            max_concurrency,
            request_timeout: Duration::from_millis(request_timeout_ms),
            metrics_token,
            auth_lockout_failures,
            auth_lockout_window: Duration::from_secs(auth_lockout_window_seconds),
            auth_lockout_duration: Duration::from_secs(auth_lockout_seconds),
            auth_lockout_max_entries,
            trust_proxy_headers,
            trusted_proxies,
        })
    }
}

fn default_rrb_id() -> String {
    "redis_target".to_string()
}

fn default_max_connections() -> usize {
    3
}
