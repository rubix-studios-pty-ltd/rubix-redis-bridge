use anyhow::{anyhow, bail};

use super::types::CommandArg;

pub(crate) fn validate_subcommand(array: &[CommandArg]) -> anyhow::Result<()> {
    let subcommand = array
        .get(1)
        .and_then(CommandArg::as_command_name)
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
                    .as_command_name()
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
