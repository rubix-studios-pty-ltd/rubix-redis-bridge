use std::collections::HashSet;

use serde_json::{Value, json};

use crate::commands::{CONNECTION_COMMANDS, DENIED_COMMANDS};

use crate::security::{CommandArg, SecurityPolicy};
use crate::security::{denied_commands, is_denied_command};

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

fn ratelimit_policy() -> SecurityPolicy {
    let mut policy = policy();

    policy.upstash_ratelimit = true;
    policy.allowed_commands.insert("SCRIPT".to_string());

    policy
}

fn command(value: Value) -> Vec<CommandArg> {
    serde_json::from_value(value).unwrap()
}

fn error_contains(policy: &SecurityPolicy, value: Value, expected: &str) {
    let command = command(value);
    let err = policy.parse_command(&command).unwrap_err();
    let message = err.to_string();

    assert!(
        message.contains(expected),
        "expected error to contain `{expected}`, got `{message}`",
    );
}

#[test]
fn accept_normal_commands() {
    for command in ["GET", "SET"] {
        assert!(!is_denied_command(command, false));
    }
}

#[test]
fn build_denied_command() {
    for command in DENIED_COMMANDS.iter().copied() {
        assert!(denied_commands().contains(command));
    }
}

#[test]
fn reject_denied_commands() {
    for command in DENIED_COMMANDS.iter().copied() {
        assert!(is_denied_command(command, false));
    }
}

#[test]
fn reject_connection_commands() {
    let mut policy = policy();

    policy
        .allowed_commands
        .extend(CONNECTION_COMMANDS.iter().copied().map(String::from));

    for &command in CONNECTION_COMMANDS {
        error_contains(&policy, json!([command]), "hard-denied");
    }
}

#[test]
fn reject_denied_commands_in_allowlist() {
    let mut policy = policy();

    policy.allowed_commands.insert("FCALL".to_string());

    let err = policy.validate().unwrap_err();

    assert!(err.to_string().contains("hard-denied"));
}

#[test]
fn reject_empty_allowlist() {
    let mut policy = policy();

    policy.allowed_commands.clear();

    assert!(policy.validate().is_err());
}

#[test]
fn reject_large_argument() {
    error_contains(
        &policy(),
        json!(["SET", "key", "this-value-is-too-large"]),
        "too large",
    );
}

#[test]
fn accept_valid_command_when_arg_max() {
    let mut policy = policy();

    policy.max_command_args = usize::MAX;

    assert!(
        policy
            .parse_command(&command(json!(["GET", "key"])))
            .is_ok()
    );
}

#[test]
fn accept_ratelimit_profile() {
    for command in ["EVAL", "EVALSHA", "SCRIPT"] {
        assert!(!is_denied_command(command, true));
    }

    for command in ["EVAL_RO", "EVALSHA_RO", "FCALL", "FUNCTION", "CONFIG"] {
        assert!(is_denied_command(command, true));
    }
}

#[test]
fn accept_ratelimit_script_flush() {
    let command = ratelimit_policy()
        .parse_command(&command(json!(["SCRIPT", "FLUSH"])))
        .unwrap();

    assert_eq!(command.name, "SCRIPT");
}

#[test]
fn accept_ratelimit_script_flush_sync() {
    let command = ratelimit_policy()
        .parse_command(&command(json!(["SCRIPT", "FLUSH", "SYNC"])))
        .unwrap();

    assert_eq!(command.name, "SCRIPT");
}

#[test]
fn reject_ratelimit_script_flush_invalid() {
    error_contains(
        &ratelimit_policy(),
        json!(["SCRIPT", "FLUSH", "BAD"]),
        "SYNC or ASYNC",
    );
}

#[test]
fn reject_ratelimit_script_kill() {
    error_contains(
        &ratelimit_policy(),
        json!(["SCRIPT", "KILL"]),
        "blocked by bridge policy",
    );
}
