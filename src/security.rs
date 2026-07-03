use std::collections::HashSet;
use std::fmt;

use anyhow::{anyhow, bail};
use serde_json::Value;

#[derive(Clone)]
pub struct SecurityPolicy {
    pub allowed_commands: HashSet<String>,
    pub blocked_commands: HashSet<String>,
    pub max_pipeline_commands: usize,
    pub max_command_args: usize,
    pub max_arg_bytes: usize,
    pub upstash_ratelimit: bool,
}

#[derive(Clone)]
pub struct RedisCommand {
    pub name: String,
    pub args: Vec<Vec<u8>>,
}

impl fmt::Debug for SecurityPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityPolicy")
            .field("allowed_command_count", &self.allowed_commands.len())
            .field("blocked_command_count", &self.blocked_commands.len())
            .field("max_pipeline_commands", &self.max_pipeline_commands)
            .field("max_command_args", &self.max_command_args)
            .field("max_arg_bytes", &self.max_arg_bytes)
            .field("upstash_ratelimit", &self.upstash_ratelimit)
            .finish()
    }
}

impl fmt::Debug for RedisCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisCommand")
            .field("name", &self.name)
            .field("arg_count", &self.args.len())
            .finish()
    }
}

impl SecurityPolicy {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.allowed_commands.is_empty() {
            bail!("RRB_ALLOWED_COMMANDS must not be empty. The bridge fails closed by default.");
        }

        let hard_denied_allowed = self
            .allowed_commands
            .iter()
            .filter(|command| is_hard_denied_command(command, self.upstash_ratelimit))
            .cloned()
            .collect::<Vec<_>>();

        if !hard_denied_allowed.is_empty() {
            bail!(
                "hard-denied Redis commands cannot be allowed: {}",
                hard_denied_allowed.join(",")
            );
        }

        Ok(())
    }

    pub fn parse_single_command(&self, value: &Value) -> anyhow::Result<RedisCommand> {
        let array = value
            .as_array()
            .ok_or_else(|| anyhow!("Invalid command array. Expected a JSON array at the root."))?;

        self.parse_command_array(array)
    }

    pub fn parse_command_list(&self, value: &Value) -> anyhow::Result<Vec<RedisCommand>> {
        let commands = value.as_array().ok_or_else(|| {
            anyhow!("Invalid command array. Expected an array of command arrays at the root.")
        })?;

        if commands.len() > self.max_pipeline_commands {
            bail!(
                "Pipeline is too large. Maximum allowed commands: {}.",
                self.max_pipeline_commands
            );
        }

        commands
            .iter()
            .map(|command| {
                let command_array = command.as_array().ok_or_else(|| {
                    anyhow!(
                        "Invalid command array. Expected an array of command arrays at the root."
                    )
                })?;
                self.parse_command_array(command_array)
            })
            .collect()
    }

    fn parse_command_array(&self, array: &[Value]) -> anyhow::Result<RedisCommand> {
        if array.is_empty() {
            bail!("Invalid command array. Command cannot be empty.");
        }

        if array.len() > self.max_command_args.saturating_add(1) {
            bail!(
                "Command has too many arguments. Maximum allowed arguments: {}.",
                self.max_command_args
            );
        }

        let command_name = array[0]
            .as_str()
            .ok_or_else(|| {
                anyhow!("Invalid command array. First item must be a Redis command string.")
            })?
            .trim()
            .to_ascii_uppercase();

        if command_name.is_empty() {
            bail!("Invalid command array. Command cannot be empty.");
        }

        if command_name == "SCRIPT" && self.upstash_ratelimit {
            validate_allowed_script_subcommand(array)?;
        } else {
            reject_hard_denied_command(&command_name, self.upstash_ratelimit)?;
        }

        if self.allowed_commands.is_empty() {
            bail!("No Redis commands are allowed by policy.");
        }

        if !self.allowed_commands.contains(&command_name) {
            bail!("Redis command is not allowed: {command_name}");
        }

        if self.blocked_commands.contains(&command_name) {
            bail!("Redis command is blocked by policy: {command_name}");
        }

        let args = array[1..]
            .iter()
            .map(|value| json_value_to_arg(value, self.max_arg_bytes))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(RedisCommand {
            name: command_name,
            args,
        })
    }
}

fn reject_hard_denied_command(command_name: &str, upstash_ratelimit: bool) -> anyhow::Result<()> {
    if is_hard_denied_command(command_name, upstash_ratelimit) {
        bail!("Redis command is hard-denied by bridge policy: {command_name}");
    }

    Ok(())
}

fn is_hard_denied_command(command_name: &str, upstash_ratelimit: bool) -> bool {
    if upstash_ratelimit && matches!(command_name, "EVAL" | "EVALSHA" | "SCRIPT") {
        return false;
    }

    matches!(
        command_name,
        // EVAL/EVALSHA/SCRIPT can be enabled only by the Upstash Ratelimit compatibility profile
        // and the normal command allowlist.
        "EVAL"
            | "EVAL_RO"
            | "EVALSHA"
            | "EVALSHA_RO"
            | "FCALL"
            | "FCALL_RO"
            | "FUNCTION"
            | "SCRIPT"
            // Administrative or multiplexed command families. These are denied at the family
            // level because subcommands include dangerous operations.
            | "ACL"
            | "CLIENT"
            | "CLUSTER"
            | "COMMAND"
            | "CONFIG"
            | "MODULE"
            | "XGROUP"
            // Connection/session state. These are unsafe with cloned ConnectionManager handles
            // because clones share a multiplexed underlying Redis connection.
            | "ASKING"
            | "AUTH"
            | "HELLO"
            | "QUIT"
            | "READONLY"
            | "READWRITE"
            | "RESET"
            | "SELECT"
            // Transaction/session state. Use POST /multi-exec instead of raw MULTI/EXEC.
            | "DISCARD"
            | "EXEC"
            | "MULTI"
            | "UNWATCH"
            | "WATCH"
            // Destructive, replication, persistence, observability, and blocking commands.
            | "BGREWRITEAOF"
            | "BGSAVE"
            | "BLMOVE"
            | "BLMPOP"
            | "BLPOP"
            | "BRPOP"
            | "BRPOPLPUSH"
            | "BZPOPMAX"
            | "BZPOPMIN"
            | "BZMPOP"
            | "DBSIZE"
            | "DEBUG"
            | "FLUSHALL"
            | "FLUSHDB"
            | "INFO"
            | "KEYS"
            | "LASTSAVE"
            | "LATENCY"
            | "MEMORY"
            | "MIGRATE"
            | "MONITOR"
            | "PSUBSCRIBE"
            | "PSYNC"
            | "PUNSUBSCRIBE"
            | "PUBLISH"
            | "PUBSUB"
            | "REPLCONF"
            | "REPLICAOF"
            | "RESTORE"
            | "ROLE"
            | "SAVE"
            | "SHUTDOWN"
            | "SLAVEOF"
            | "SLOWLOG"
            | "SORT"
            | "SORT_RO"
            | "SSUBSCRIBE"
            | "SUNSUBSCRIBE"
            | "SUBSCRIBE"
            | "SWAPDB"
            | "SYNC"
            | "UNSUBSCRIBE"
            | "WAIT"
            | "WAITAOF"
            | "XREAD"
            | "XREADGROUP"
    )
}

fn validate_allowed_script_subcommand(array: &[Value]) -> anyhow::Result<()> {
    let subcommand = array
        .get(1)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_uppercase())
        .ok_or_else(|| anyhow!("SCRIPT requires a subcommand."))?;

    match subcommand.as_str() {
        "LOAD" => {
            if array.len() != 3 {
                bail!("SCRIPT LOAD requires exactly one script argument.");
            }

            Ok(())
        }
        "EXISTS" => {
            if array.len() < 3 {
                bail!("SCRIPT EXISTS requires at least one SHA argument.");
            }

            Ok(())
        }
        "FLUSH" => {
            if array.len() > 3 {
                bail!("SCRIPT FLUSH accepts at most one optional mode argument.");
            }

            if let Some(mode) = array.get(2) {
                let mode = mode
                    .as_str()
                    .map(|value| value.trim().to_ascii_uppercase())
                    .ok_or_else(|| anyhow!("SCRIPT FLUSH mode must be a string."))?;

                if !matches!(mode.as_str(), "SYNC" | "ASYNC") {
                    bail!("SCRIPT FLUSH mode must be SYNC or ASYNC.");
                }
            }

            Ok(())
        }
        "KILL" | "DEBUG" => {
            bail!("SCRIPT {subcommand} is blocked by bridge policy.")
        }
        _ => bail!("SCRIPT subcommand is not allowed by bridge policy: {subcommand}"),
    }
}

fn json_value_to_arg(value: &Value, max_arg_bytes: usize) -> anyhow::Result<Vec<u8>> {
    let bytes = match value {
        Value::String(value) => value.as_bytes().to_vec(),
        Value::Number(value) => value.to_string().into_bytes(),
        Value::Bool(value) => value.to_string().into_bytes(),
        Value::Null => bail!("Invalid Redis argument. Null values are not supported."),
        Value::Array(_) | Value::Object(_) => {
            bail!("Invalid Redis argument. Nested arrays and objects are not supported.")
        }
    };

    if bytes.len() > max_arg_bytes {
        bail!(
            "Redis argument is too large. Maximum allowed bytes per argument: {}.",
            max_arg_bytes
        );
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SecurityPolicy {
        SecurityPolicy {
            allowed_commands: ["GET", "SET"].into_iter().map(String::from).collect(),
            blocked_commands: HashSet::new(),
            max_pipeline_commands: 10,
            max_command_args: 4,
            max_arg_bytes: 16,
            upstash_ratelimit: false,
        }
    }

    #[test]
    fn denies_shared_connection_commands() {
        let mut policy = policy();
        policy.allowed_commands.extend(
            [
                "SELECT",
                "HELLO",
                "RESET",
                "ASKING",
                "READONLY",
                "READWRITE",
            ]
            .into_iter()
            .map(String::from),
        );

        for command in [
            "SELECT",
            "HELLO",
            "RESET",
            "ASKING",
            "READONLY",
            "READWRITE",
        ] {
            let err = policy
                .parse_single_command(&serde_json::json!([command]))
                .unwrap_err();
            assert!(err.to_string().contains("hard-denied"));
        }
    }

    #[test]
    fn rejects_empty_allowlist() {
        let mut policy = policy();
        policy.allowed_commands.clear();

        assert!(policy.validate().is_err());
    }

    #[test]
    fn rejects_large_argument() {
        let err = policy()
            .parse_single_command(&serde_json::json!([
                "SET",
                "key",
                "this-value-is-too-large"
            ]))
            .unwrap_err();

        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn uses_saturating_arg_limit() {
        let mut policy = policy();
        policy.max_command_args = usize::MAX;

        assert!(
            policy
                .parse_single_command(&serde_json::json!(["GET", "key"]))
                .is_ok()
        );
    }

    #[test]
    fn validate_rejects_hard_denied_commands() {
        let mut policy = policy();
        policy.allowed_commands.insert("FCALL".to_string());

        let err = policy.validate().unwrap_err();

        assert!(err.to_string().contains("hard-denied"));
    }

    #[test]
    fn allows_script_flush_ratelimit_enabled() {
        let mut policy = policy();
        policy.upstash_ratelimit = true;
        policy.allowed_commands.insert("SCRIPT".to_string());

        let command = policy
            .parse_single_command(&serde_json::json!(["SCRIPT", "FLUSH"]))
            .unwrap();

        assert_eq!(command.name, "SCRIPT");
    }

    #[test]
    fn allows_script_flush_sync_ratelimit_enabled() {
        let mut policy = policy();
        policy.upstash_ratelimit = true;
        policy.allowed_commands.insert("SCRIPT".to_string());

        let command = policy
            .parse_single_command(&serde_json::json!(["SCRIPT", "FLUSH", "SYNC"]))
            .unwrap();

        assert_eq!(command.name, "SCRIPT");
    }

    #[test]
    fn rejects_script_flush_invalid_ratelimit_enabled() {
        let mut policy = policy();
        policy.upstash_ratelimit = true;
        policy.allowed_commands.insert("SCRIPT".to_string());

        let err = policy
            .parse_single_command(&serde_json::json!(["SCRIPT", "FLUSH", "BAD"]))
            .unwrap_err();

        assert!(err.to_string().contains("SYNC or ASYNC"));
    }

    #[test]
    fn rejects_script_kill_ratelimit_enabled() {
        let mut policy = policy();
        policy.upstash_ratelimit = true;
        policy.allowed_commands.insert("SCRIPT".to_string());

        let err = policy
            .parse_single_command(&serde_json::json!(["SCRIPT", "KILL"]))
            .unwrap_err();

        assert!(err.to_string().contains("blocked by bridge policy"));
    }
}
