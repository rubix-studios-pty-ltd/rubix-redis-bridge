use std::collections::HashSet;

use anyhow::{anyhow, bail};

pub(super) fn parse_env_or_default<T>(key: &str, default: T) -> anyhow::Result<T>
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

pub(super) fn parse_env_first<T>(keys: &[&str], default: T) -> anyhow::Result<T>
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

pub(super) fn parse_csv_env_first(keys: &[&str]) -> anyhow::Result<Option<HashSet<String>>> {
    for key in keys {
        match std::env::var(key) {
            Ok(value) => return Ok(Some(parse_csv(&value))),
            Err(std::env::VarError::NotPresent) => {}
            Err(error) => return Err(anyhow!("Failed to read {key}: {error}")),
        }
    }

    Ok(None)
}

pub(crate) fn parse_csv(value: &str) -> HashSet<String> {
    value
        .split(',')
        .map(|item| item.trim().to_ascii_uppercase())
        .filter(|item| !item.is_empty())
        .collect()
}

pub(super) fn parse_bool_env(key: &str, default: bool) -> anyhow::Result<bool> {
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

pub(super) fn parse_command_list(commands: &[&str]) -> HashSet<String> {
    commands
        .iter()
        .map(|command| command.trim().to_ascii_uppercase())
        .filter(|command| !command.is_empty())
        .collect()
}

pub(super) fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| std::env::var(key).ok())
}

pub(super) fn env_or(key: &str, fallback: impl Into<String>) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.into())
}
