use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use sha1_smol::Sha1;

use super::types::RedisCommand;

const UPSTASH_HEADER: &[u8] = b"#!lua flags=allow-key-locking\n";
const REDIS_HEADER: &[u8] = b"#!lua\n";
const MAX_ALIASES: usize = 1024;

#[derive(Clone, Default)]
pub(crate) struct ScriptCompatibility {
    aliases: Arc<Mutex<Aliases>>,
}

#[derive(Default)]
struct Aliases {
    hashes: HashMap<String, String>,
    order: VecDeque<String>,
}

impl ScriptCompatibility {
    pub(crate) fn normalize(&self, command: &mut RedisCommand) {
        match command.name.as_str() {
            "EVAL" => {
                if let Some(script) = command.args.first_mut() {
                    let _ = self.normalize_script(script);
                }
            }
            "EVALSHA" => {
                if let Some(hash) = command.args.first_mut() {
                    self.normalize_hash(hash);
                }
            }
            "SCRIPT" => {
                let Some((subcommand, args)) = command.args.split_first_mut() else {
                    return;
                };

                if subcommand.eq_ignore_ascii_case(b"LOAD") {
                    if let Some(script) = args.first_mut() {
                        command.script_hash = self.normalize_script(script);
                    }
                } else if subcommand.eq_ignore_ascii_case(b"EXISTS") {
                    for hash in args {
                        self.normalize_hash(hash);
                    }
                }
            }
            _ => {}
        }
    }

    fn normalize_script(&self, script: &mut Vec<u8>) -> Option<(String, String)> {
        let body = script.strip_prefix(UPSTASH_HEADER)?;
        let original = Sha1::from(script.as_slice()).digest().to_string();

        let mut normalized = Vec::with_capacity(REDIS_HEADER.len() + body.len());
        normalized.extend_from_slice(REDIS_HEADER);
        normalized.extend_from_slice(body);

        let actual = Sha1::from(normalized.as_slice()).digest().to_string();
        *script = normalized;

        let mut aliases = self
            .aliases
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        if !aliases.hashes.contains_key(&original) {
            if aliases.hashes.len() >= MAX_ALIASES
                && let Some(oldest) = aliases.order.pop_front()
            {
                aliases.hashes.remove(&oldest);
            }

            aliases.order.push_back(original.clone());
            aliases.hashes.insert(original.clone(), actual.clone());
        }

        Some((original, actual))
    }

    fn normalize_hash(&self, hash: &mut Vec<u8>) {
        if hash.len() != 40 || !hash.iter().all(u8::is_ascii_hexdigit) {
            return;
        }

        let key = String::from_utf8_lossy(hash).to_ascii_lowercase();

        let aliases = self
            .aliases
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        if let Some(actual) = aliases.hashes.get(&key) {
            *hash = actual.as_bytes().to_vec();
        }
    }
}
