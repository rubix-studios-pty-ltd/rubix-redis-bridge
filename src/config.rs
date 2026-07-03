use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, anyhow, bail};
use serde::Deserialize;

use crate::commands::{ALLOWED_COMMANDS, DENIED_COMMANDS, RATELIMIT_COMMANDS};
use crate::security::SecurityPolicy;

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

        let mut allowed_commands = parse_csv_env_first(&["RRB_ALLOWED_COMMANDS"])?
            .unwrap_or_else(|| parse_command_list(ALLOWED_COMMANDS));

        let mut blocked_commands = parse_csv_env_first(&["RRB_BLOCKED_COMMANDS"])?
            .unwrap_or_else(|| parse_command_list(DENIED_COMMANDS));

        if let Some(extra_blocked_commands) = parse_csv_env_first(&["RRB_BLOCKED_COMMANDS"])? {
            blocked_commands.extend(extra_blocked_commands);
        }

        let upstash_ratelimit = parse_bool_env("RRB_UPSTASH_RATELIMIT", false)?;

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

fn load_targets() -> anyhow::Result<HashMap<String, RedisTargetConfig>> {
    let mode = env_or("RRB_MODE", "file");

    match mode.as_str() {
        "env" => load_env_target(),
        "file" => load_file_targets(),
        other => Err(anyhow!("Unsupported bridge mode: {other}. Use env or file")),
    }
}

fn load_env_target() -> anyhow::Result<HashMap<String, RedisTargetConfig>> {
    let token = env_first(&["RRB_TOKEN"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("RRB_TOKEN is required when mode=env"))?;

    let connection_string = env_first(&["RRB_CONNECTION_STRING", "REDIS_URL"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("RRB_CONNECTION_STRING or REDIS_URL is required when mode=env"))?;

    let max_connections = parse_env_first(&["RRB_MAX_CONNECTIONS"], default_max_connections())?;

    if max_connections == 0 {
        bail!("RRB_MAX_CONNECTIONS must be greater than zero");
    }

    let mut targets = HashMap::new();
    targets.insert(
        token,
        RedisTargetConfig {
            rrb_id: "env_config_connection".to_string(),
            connection_string,
            max_connections,
        },
    );

    Ok(targets)
}

fn load_file_targets() -> anyhow::Result<HashMap<String, RedisTargetConfig>> {
    let path = env_first(&["RRB_CONFIG_FILE", "TOKEN_RESOLUTION_FILE_PATH"])
        .unwrap_or_else(|| "/app/rrb-config/tokens.json".to_string());

    file_permissions(&path);

    let data = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read token config file: {path}"))?;

    let raw_targets: HashMap<String, RedisTargetConfig> = serde_json::from_str(&data)
        .with_context(|| format!("Failed to parse token config file: {path}"))?;

    let mut targets = HashMap::with_capacity(raw_targets.len());
    for (token, mut target_config) in raw_targets {
        let token = token.trim().to_string();
        if token.is_empty() {
            bail!("Token config file contains an empty token");
        }
        if target_config.rrb_id.trim().is_empty() || target_config.rrb_id == default_rrb_id() {
            target_config.rrb_id = derived_rrb_id(&token);
        }
        if target_config.connection_string.trim().is_empty() {
            bail!(
                "Token config file contains an empty Redis connection string for target {}",
                target_config.rrb_id
            );
        }
        if target_config.max_connections == 0 {
            bail!(
                "Token config file contains max_connections=0 for target {}",
                target_config.rrb_id
            );
        }
        if targets.insert(token, target_config).is_some() {
            bail!("Token config file contains duplicate tokens after trimming whitespace");
        }
    }

    Ok(targets)
}

fn parse_env_or_default<T>(key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
{
    match std::env::var(key) {
        Ok(value) => value
            .parse::<T>()
            .map_err(|_| anyhow!("Invalid value for {key}: {value:?}")),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(anyhow!("Failed to read {key}: {error}")),
    }
}

fn parse_env_first<T>(keys: &[&str], default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
{
    for key in keys {
        match std::env::var(key) {
            Ok(value) => {
                return value
                    .parse::<T>()
                    .map_err(|_| anyhow!("Invalid value for {key}: {value:?}"));
            }
            Err(std::env::VarError::NotPresent) => {}
            Err(error) => return Err(anyhow!("Failed to read {key}: {error}")),
        }
    }

    Ok(default)
}

fn parse_csv_env_first(keys: &[&str]) -> anyhow::Result<Option<HashSet<String>>> {
    for key in keys {
        match std::env::var(key) {
            Ok(value) => return Ok(Some(parse_csv(&value))),
            Err(std::env::VarError::NotPresent) => {}
            Err(error) => return Err(anyhow!("Failed to read {key}: {error}")),
        }
    }

    Ok(None)
}

fn parse_csv(value: &str) -> HashSet<String> {
    value
        .split(',')
        .map(|item| item.trim().to_ascii_uppercase())
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_bool_env(key: &str, default: bool) -> anyhow::Result<bool> {
    match std::env::var(key) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            other => bail!("{key} must be a boolean value, got: {other}"),
        },
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(anyhow!("Failed to read {key}: {error}")),
    }
}

fn parse_command_list(commands: &[&str]) -> HashSet<String> {
    commands
        .iter()
        .map(|command| command.trim().to_ascii_uppercase())
        .filter(|command| !command.is_empty())
        .collect()
}

fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| std::env::var(key).ok())
}

fn env_or(key: &str, fallback: impl Into<String>) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.into())
}

fn default_rrb_id() -> String {
    "redis_target".to_string()
}

fn default_max_connections() -> usize {
    3
}

fn derived_rrb_id(token: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    format!("redis_target_{:016x}", hasher.finish())
}

#[cfg(unix)]
fn file_permissions(path: &str) {
    use tracing::warn;

    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        warn!(
            path = %path,
            mode = %format_args!("{mode:o}"),
            "Token config file is not private; Use owner-only permissions such as 0600"
        );
    }
}

#[cfg(not(unix))]
fn file_permissions(_path: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_csv_commands() {
        let commands = parse_csv("get, Set , DEL");
        assert!(commands.contains("GET"));
        assert!(commands.contains("SET"));
        assert!(commands.contains("DEL"));
    }

    #[test]
    fn derives_custom_id() {
        assert_ne!(derived_rrb_id("abc123"), default_rrb_id());
    }
}
