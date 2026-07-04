use std::fmt;

use anyhow::bail;
use serde::Deserialize;

#[derive(Clone)]
pub struct RedisCommand {
    pub name: String,
    pub args: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CommandArg {
    String(String),
    Number(serde_json::Number),
    Bool(bool),
    Null,
}

impl CommandArg {
    pub(crate) fn as_command_name(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub(crate) fn to_arg_bytes(&self, max_arg_bytes: usize) -> anyhow::Result<Vec<u8>> {
        let bytes = match self {
            Self::String(value) => value.as_bytes().to_vec(),
            Self::Number(value) => value.to_string().into_bytes(),
            Self::Bool(value) => value.to_string().into_bytes(),
            Self::Null => bail!("Invalid Redis argument. Null values are not supported."),
        };

        if bytes.len() > max_arg_bytes {
            bail!(
                "Redis argument is too large. Maximum allowed bytes per argument: {}.",
                max_arg_bytes
            );
        }

        Ok(bytes)
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
