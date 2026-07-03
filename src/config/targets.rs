use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, anyhow, bail};

use super::env::{env_first, env_or, parse_env_first};
use super::{RedisTargetConfig, default_max_connections, default_rrb_id};

pub(super) fn load_targets() -> anyhow::Result<HashMap<String, RedisTargetConfig>> {
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
    fn derives_custom_id() {
        assert_ne!(derived_rrb_id("abc123"), default_rrb_id());
    }
}
