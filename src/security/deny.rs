use std::collections::HashSet;
use std::sync::OnceLock;

use anyhow::bail;

use super::commands::{DENIED_COMMANDS, RATELIMIT_COMMANDS};

pub(crate) fn reject_command(command_name: &str, upstash_ratelimit: bool) -> anyhow::Result<()> {
    if is_denied_command(command_name, upstash_ratelimit) {
        bail!("Redis command is hard-denied by bridge policy: {command_name}");
    }

    Ok(())
}

pub(crate) fn is_denied_command(command_name: &str, upstash_ratelimit: bool) -> bool {
    if upstash_ratelimit && ratelimit_commands().contains(command_name) {
        return false;
    }

    denied_commands().contains(command_name)
}

pub(crate) fn denied_commands() -> &'static HashSet<&'static str> {
    static COMMANDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

    COMMANDS.get_or_init(|| DENIED_COMMANDS.iter().copied().collect())
}

fn ratelimit_commands() -> &'static HashSet<&'static str> {
    static COMMANDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

    COMMANDS.get_or_init(|| RATELIMIT_COMMANDS.iter().copied().collect())
}
