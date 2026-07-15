use std::fmt;

use anyhow::bail;

#[derive(Clone, Eq, PartialEq)]
pub struct TokenCaps {
    command: bool,
    ratelimit: bool,
    realtime: bool,
}

impl TokenCaps {
    pub(crate) fn command() -> Self {
        Self {
            command: true,
            ratelimit: false,
            realtime: false,
        }
    }

    pub(crate) fn parse(value: &str, key: &str) -> anyhow::Result<Self> {
        let mut token_type = Self {
            command: false,
            ratelimit: false,
            realtime: false,
        };

        for item in value.split(',') {
            let item = item.trim().to_ascii_lowercase();

            if item.is_empty() {
                continue;
            }

            match item.as_str() {
                "command" => token_type.command = true,
                "ratelimit" => token_type.ratelimit = true,
                "realtime" => token_type.realtime = true,
                other => {
                    bail!(
                        "{key} contains unsupported token type: {other}. Supported token types: command,ratelimit,realtime"
                    );
                }
            }
        }

        if !token_type.command && !token_type.ratelimit && !token_type.realtime {
            bail!("{key} must include at least one token type");
        }

        Ok(token_type)
    }

    pub(crate) fn allows_command(&self) -> bool {
        self.command
    }

    pub(crate) fn allows_ratelimit(&self) -> bool {
        self.ratelimit
    }

    pub(crate) fn allows_realtime(&self) -> bool {
        self.realtime
    }

    pub(crate) fn allows_command_route(&self) -> bool {
        self.command || self.ratelimit || self.realtime
    }

    pub(crate) fn config_value(&self) -> String {
        let mut values = Vec::new();

        if self.command {
            values.push("command");
        }

        if self.ratelimit {
            values.push("ratelimit");
        }

        if self.realtime {
            values.push("realtime");
        }

        values.join(",")
    }
}

impl Default for TokenCaps {
    fn default() -> Self {
        Self::command()
    }
}

impl fmt::Debug for TokenCaps {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.config_value())
    }
}
