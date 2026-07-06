use std::collections::HashSet;
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, anyhow, bail};
use serde::Deserialize;

use super::env::{env_first, env_or, parse_env_first};
use super::{AuthToken, Redis, TokenHash, default_connection_shards, default_max_connections};

pub(super) fn load_targets() -> anyhow::Result<Vec<Redis>> {
    let mode = env_or("RRB_MODE", "file");

    match mode.as_str() {
        "env" => load_env_target(),
        "file" => load_file_targets(),
        other => Err(anyhow!("Unsupported bridge mode: {other}. Use env or file")),
    }
}

fn load_env_target() -> anyhow::Result<Vec<Redis>> {
    let token = env_first(&["RRB_TOKEN"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("RRB_TOKEN is required when mode=env"))?;

    let connection_string = env_first(&["RRB_CONNECTION_STRING", "REDIS_URL"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("RRB_CONNECTION_STRING or REDIS_URL is required when mode=env"))?;

    let max_connections = parse_env_first(&["RRB_MAX_CONNECTIONS"], default_max_connections())?;

    let connection_shards =
        parse_env_first(&["RRB_CONNECTION_SHARDS"], default_connection_shards())?;

    if max_connections == 0 {
        bail!("RRB_MAX_CONNECTIONS must be greater than zero");
    }

    if connection_shards == 0 {
        bail!("RRB_CONNECTION_SHARDS must be greater than zero");
    }

    Ok(vec![Redis {
        rrb_id: "env".to_string(),
        connection_string,
        max_connections,
        connection_shards,
        tokens: vec![AuthToken {
            id: "env".to_string(),
            name: Some("Environment token".to_string()),
            hash: TokenHash::sha256(&token),
            enabled: true,
        }],
    }])
}

fn load_file_targets() -> anyhow::Result<Vec<Redis>> {
    let path = env_first(&["RRB_CONFIG_FILE", "TOKEN_RESOLUTION_FILE_PATH"])
        .unwrap_or_else(|| "/app/rrb-config/tokens.json".to_string());

    file_permissions(&path);

    let data = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read token config file: {path}"))?;

    let hash_token = env_first(&["RRB_HASH_TOKEN"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    parse_file_targets(&data, hash_token.as_deref())
        .with_context(|| format!("Failed to parse token config file: {path}"))
}

#[derive(Deserialize)]
struct FileConfig {
    version: u16,
    targets: Vec<FileTarget>,
}

#[derive(Deserialize)]
struct FileTarget {
    rrb_id: String,
    connection_string: String,
    #[serde(default = "default_max_connections")]
    max_connections: usize,
    #[serde(default = "default_connection_shards")]
    connection_shards: usize,
    tokens: Vec<FileAuthToken>,
}

#[derive(Deserialize)]
struct FileAuthToken {
    id: String,
    #[serde(default)]
    name: Option<String>,
    hash: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn parse_file_targets(data: &str, hash_token: Option<&str>) -> anyhow::Result<Vec<Redis>> {
    let _hash_token = hash_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("RRB_HASH_TOKEN is required when mode=file"))?;

    let file_config: FileConfig = serde_json::from_str(data)?;

    if file_config.version != 1 {
        bail!("Unsupported token config version: {}", file_config.version);
    }

    if file_config.targets.is_empty() {
        bail!("Token config file must contain at least one target");
    }

    let mut rrb_ids = HashSet::new();
    let mut token_ids = HashSet::new();
    let mut token_hashes = HashSet::new();
    let mut targets = Vec::with_capacity(file_config.targets.len());

    for target in file_config.targets {
        let rrb_id = target.rrb_id.trim().to_string();

        if rrb_id.is_empty() {
            bail!("Token config file contains an empty rrb_id");
        }

        if !rrb_ids.insert(rrb_id.clone()) {
            bail!("Token config file contains duplicate rrb_id: {rrb_id}");
        }

        let connection_string = target.connection_string.trim().to_string();

        if connection_string.is_empty() {
            bail!(
                "Token config file contains an empty Redis connection string for target {rrb_id}"
            );
        }

        if target.max_connections == 0 {
            bail!("Token config file contains max_connections=0 for target {rrb_id}");
        }

        if target.connection_shards == 0 {
            bail!("Token config file contains connection_shards=0 for target {rrb_id}");
        }

        if target.tokens.is_empty() {
            bail!("Token config file contains no tokens for target {rrb_id}");
        }

        let mut tokens = Vec::with_capacity(target.tokens.len());

        for token in target.tokens {
            let id = token.id.trim().to_string();

            if id.is_empty() {
                bail!("Token config file contains an empty token id for target {rrb_id}");
            }

            if !token_ids.insert(id.clone()) {
                bail!("Token config file contains duplicate token id: {id}");
            }

            if !token.enabled {
                tokens.push(AuthToken {
                    id,
                    name: token
                        .name
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty()),
                    hash: TokenHash::hmac_sha256_parse(&token.hash)?,
                    enabled: false,
                });
                continue;
            }

            let hash = TokenHash::hmac_sha256_parse(&token.hash)?;

            if !token_hashes.insert(hash.clone()) {
                bail!("Token config file contains duplicate token hashes");
            }

            tokens.push(AuthToken {
                id,
                name: token
                    .name
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                hash,
                enabled: true,
            });
        }

        if !tokens.iter().any(|token| token.enabled) {
            bail!("Token config file contains no enabled tokens for target {rrb_id}");
        }

        targets.push(Redis {
            rrb_id,
            connection_string,
            max_connections: target.max_connections,
            connection_shards: target.connection_shards,
            tokens,
        });
    }

    Ok(targets)
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
#[path = "targets_tests.rs"]
mod tests;
