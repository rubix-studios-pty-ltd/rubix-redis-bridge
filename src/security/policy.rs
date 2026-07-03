use std::collections::HashSet;
use std::fmt;

use anyhow::{anyhow, bail};
use serde_json::Value;

use super::args::json_to_arg;
use super::deny::{is_denied_command, reject_command};
use super::script::validate_subcommand;
use super::types::RedisCommand;

#[derive(Clone)]
pub struct SecurityPolicy {
    pub allowed_commands: HashSet<String>,
    pub blocked_commands: HashSet<String>,
    pub max_pipeline_commands: usize,
    pub max_command_args: usize,
    pub max_arg_bytes: usize,
    pub upstash_ratelimit: bool,
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

impl SecurityPolicy {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.allowed_commands.is_empty() {
            bail!("RRB_ALLOWED_COMMANDS must not be empty. The bridge fails closed by default.");
        }

        let hard_denied_allowed = self
            .allowed_commands
            .iter()
            .filter(|command| is_denied_command(command, self.upstash_ratelimit))
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

    pub fn parse_command(&self, value: &Value) -> anyhow::Result<RedisCommand> {
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
            validate_subcommand(array)?;
        } else {
            reject_command(&command_name, self.upstash_ratelimit)?;
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
            .map(|value| json_to_arg(value, self.max_arg_bytes))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(RedisCommand {
            name: command_name,
            args,
        })
    }
}
