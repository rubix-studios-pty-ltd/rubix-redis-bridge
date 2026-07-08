use std::collections::HashSet;
use std::fmt;

use anyhow::{anyhow, bail};

use crate::config::TokenCaps;

use super::deny::{is_denied_command, ratelimit_commands, reject_command};
use super::script::validate_subcommand;
use super::types::{CommandArg, RedisCommand};

#[derive(Clone)]
pub struct SecurityPolicy {
    pub allowed_commands: HashSet<String>,
    pub blocked_commands: HashSet<String>,
    pub max_pipeline_commands: usize,
    pub max_command_args: usize,
    pub max_arg_bytes: usize,
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
            .filter(|command| is_denied_command(command, false))
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

    pub fn parse_command(
        &self,
        array: &[CommandArg],
        token_type: &TokenCaps,
    ) -> anyhow::Result<RedisCommand> {
        self.parse_command_array(array, token_type)
    }

    pub fn parse_command_list(
        &self,
        commands: &[Vec<CommandArg>],
        token_type: &TokenCaps,
    ) -> anyhow::Result<Vec<RedisCommand>> {
        if commands.len() > self.max_pipeline_commands {
            bail!(
                "Pipeline is too large. Maximum allowed commands: {}.",
                self.max_pipeline_commands
            );
        }

        commands
            .iter()
            .map(|command| self.parse_command_array(command, token_type))
            .collect()
    }

    fn parse_command_array(
        &self,
        array: &[CommandArg],
        token_type: &TokenCaps,
    ) -> anyhow::Result<RedisCommand> {
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
            .as_command_name()
            .ok_or_else(|| {
                anyhow!("Invalid command array. First item must be a Redis command string.")
            })?
            .trim()
            .to_ascii_uppercase();

        if command_name.is_empty() {
            bail!("Invalid command array. Command cannot be empty.");
        }

        let allow_ratelimit = token_type.allows_ratelimit();

        if command_name == "SCRIPT" && allow_ratelimit {
            validate_subcommand(array)?;
        } else {
            reject_command(&command_name, allow_ratelimit)?;
        }

        let standard_allowed =
            token_type.allows_command() && self.allowed_commands.contains(&command_name);

        let ratelimit_allowed =
            allow_ratelimit && ratelimit_commands().contains(command_name.as_str());

        if !standard_allowed && !ratelimit_allowed {
            bail!("Redis command is not allowed for this token type: {command_name}");
        }

        if self.blocked_commands.contains(&command_name) {
            bail!("Redis command is blocked by policy: {command_name}");
        }

        let args = array[1..]
            .iter()
            .map(|value| value.to_arg_bytes(self.max_arg_bytes))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(RedisCommand {
            name: command_name,
            args,
        })
    }
}
