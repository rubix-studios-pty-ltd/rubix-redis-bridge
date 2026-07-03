mod env;
mod targets;

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use anyhow::bail;
use serde::Deserialize;

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

        let mut allowed_commands = parse_csv_env_first(&["RRB_ALLOWED_COMMANDS"])?
            .unwrap_or_else(|| parse_command_list(ALLOWED_COMMANDS));

        let mut blocked_commands = parse_csv_env_first(&["RRB_BLOCKED_COMMANDS"])?
            .unwrap_or_else(|| parse_command_list(DENIED_COMMANDS));

        if let Some(extra_blocked_commands) = parse_csv_env_first(&["RRB_BLOCKED_COMMANDS"])? {
            blocked_commands.extend(extra_blocked_commands);
        }

        if upstash_ratelimit {
            for &command in RATELIMIT_COMMANDS {
                allowed_commands.insert(command.to_string());
                blocked_commands.remove(command);
            }
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
        })
    }
}

fn default_rrb_id() -> String {
    "redis_target".to_string()
}

fn default_max_connections() -> usize {
    3
}
