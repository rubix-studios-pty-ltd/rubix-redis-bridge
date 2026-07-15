use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::app::{ApiError, AppState};
use crate::client::TrustedProxies;
use crate::config::{AuthToken, Bridge, Redis, TokenCaps, TokenHash};
use crate::security::SecurityPolicy;

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    headers
}

fn error_status(error: ApiError) -> StatusCode {
    error.into_response().status()
}

fn test_state(auth_lockout_failures: usize) -> AppState {
    let targets = vec![Redis {
        rrb_id: "test_redis".to_string(),
        connection_string: "redis://default:password@127.0.0.1:6379".to_string(),
        operation_limit: 1,
        connection_shards: 1,
        tokens: vec![AuthToken {
            id: "test_token".to_string(),
            name: Some("Test token".to_string()),
            hash: TokenHash::sha256("valid-token"),
            enabled: true,
            token_type: TokenCaps::default(),
        }],
    }];

    let mut allowed_commands = HashSet::new();
    allowed_commands.insert("PING".to_string());

    AppState::new(Bridge {
        host: "127.0.0.1".to_string(),
        port: 8080,
        targets,
        hash_token: None,
        security: SecurityPolicy {
            allowed_commands,
            blocked_commands: HashSet::new(),
            max_pipeline_commands: 10,
            max_command_args: 16,
            max_arg_bytes: 1024,
        },
        max_body_bytes: 1024,
        max_concurrency: 16,
        max_realtime_concurrency: 1,
        request_timeout: Duration::from_millis(500),
        acquire_timeout: Duration::from_millis(100),
        max_response_bytes: 1024 * 1024,
        metrics_token: Some("metrics-token".to_string()),
        auth_lockout_failures,
        auth_lockout_window: Duration::from_secs(60),
        auth_lockout_duration: Duration::from_secs(300),
        auth_lockout_max_entries: 1024,
        trust_proxy_headers: false,
        trusted_proxies: TrustedProxies::default(),
    })
    .unwrap()
}

fn test_hmac() -> AppState {
    let targets = vec![Redis {
        rrb_id: "test_redis".to_string(),
        connection_string: "redis://default:password@127.0.0.1:6379".to_string(),
        operation_limit: 1,
        connection_shards: 1,
        tokens: vec![AuthToken {
            id: "test_token".to_string(),
            name: Some("Test token".to_string()),
            hash: TokenHash::hmac_sha256("hash-key", "valid-token"),
            enabled: true,
            token_type: TokenCaps::default(),
        }],
    }];

    let mut allowed_commands = HashSet::new();
    allowed_commands.insert("PING".to_string());

    AppState::new(Bridge {
        host: "127.0.0.1".to_string(),
        port: 8080,
        targets,
        hash_token: Some("hash-key".to_string()),
        security: SecurityPolicy {
            allowed_commands,
            blocked_commands: HashSet::new(),
            max_pipeline_commands: 10,
            max_command_args: 16,
            max_arg_bytes: 1024,
        },
        max_body_bytes: 1024,
        max_concurrency: 16,
        max_realtime_concurrency: 1,
        request_timeout: Duration::from_millis(500),
        acquire_timeout: Duration::from_millis(100),
        max_response_bytes: 1024 * 1024,
        metrics_token: Some("metrics-token".to_string()),
        auth_lockout_failures: 3,
        auth_lockout_window: Duration::from_secs(60),
        auth_lockout_duration: Duration::from_secs(300),
        auth_lockout_max_entries: 1024,
        trust_proxy_headers: false,
        trusted_proxies: TrustedProxies::default(),
    })
    .unwrap()
}

#[test]
fn lock_ip_repeat_failures() {
    let state = test_state(3);
    let ip = ip("203.0.113.10");
    let wrong_headers = auth_headers("wrong-token");

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[test]
fn valid_token_bypasses_lockout() {
    let state = test_state(3);
    let ip = ip("203.0.113.10");
    let wrong_headers = auth_headers("wrong-token");
    let valid_headers = auth_headers("valid-token");

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::TOO_MANY_REQUESTS
    );

    assert!(state.bridge_auth(&valid_headers, ip).is_ok());
}

#[test]
fn clear_failures_after_success() {
    let state = test_state(3);
    let ip = ip("203.0.113.10");
    let wrong_headers = auth_headers("wrong-token");
    let valid_headers = auth_headers("valid-token");

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert!(state.bridge_auth(&valid_headers, ip).is_ok());

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[test]
fn count_missing_header_lockout() {
    let state = test_state(3);
    let ip = ip("203.0.113.10");
    let headers = HeaderMap::new();

    assert_eq!(
        error_status(state.bridge_auth(&headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.bridge_auth(&headers, ip).unwrap_err()),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[test]
fn lock_invalid_metrics_auth() {
    let state = test_state(3);
    let ip = ip("203.0.113.10");
    let wrong_headers = auth_headers("wrong-metrics-token");

    assert_eq!(
        error_status(state.metrics_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.metrics_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        error_status(state.metrics_auth(&wrong_headers, ip).unwrap_err()),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[test]
fn lockout_returns_unauthorized() {
    let state = test_state(0);
    let ip = ip("203.0.113.10");
    let wrong_headers = auth_headers("wrong-token");

    for _ in 0..10 {
        assert_eq!(
            error_status(state.bridge_auth(&wrong_headers, ip).unwrap_err()),
            StatusCode::UNAUTHORIZED
        );
    }
}

#[test]
fn accepts_hmac_hashed_token() {
    let state = test_hmac();
    let ip = ip("203.0.113.10");
    let valid_headers = auth_headers("valid-token");

    assert!(state.bridge_auth(&valid_headers, ip).is_ok());
}

#[test]
fn accepts_realtime_routes_for_realtime_token() {
    let targets = vec![Redis {
        rrb_id: "test_redis".to_string(),
        connection_string: "redis://default:password@127.0.0.1:6379".to_string(),
        operation_limit: 1,
        connection_shards: 1,
        tokens: vec![AuthToken {
            id: "test_token".to_string(),
            name: Some("Test token".to_string()),
            hash: TokenHash::sha256("valid-token"),
            enabled: true,
            token_type: TokenCaps::parse("realtime", "test").unwrap(),
        }],
    }];

    let mut allowed_commands = HashSet::new();
    allowed_commands.insert("PING".to_string());

    let state = AppState::new(Bridge {
        host: "127.0.0.1".to_string(),
        port: 8080,
        targets,
        hash_token: None,
        security: SecurityPolicy {
            allowed_commands,
            blocked_commands: HashSet::new(),
            max_pipeline_commands: 10,
            max_command_args: 16,
            max_arg_bytes: 1024,
        },
        max_body_bytes: 1024,
        max_concurrency: 16,
        max_realtime_concurrency: 1,
        request_timeout: Duration::from_millis(500),
        acquire_timeout: Duration::from_millis(100),
        max_response_bytes: 1024 * 1024,
        metrics_token: Some("metrics-token".to_string()),
        auth_lockout_failures: 3,
        auth_lockout_window: Duration::from_secs(60),
        auth_lockout_duration: Duration::from_secs(300),
        auth_lockout_max_entries: 1024,
        trust_proxy_headers: false,
        trusted_proxies: TrustedProxies::default(),
    })
    .unwrap();

    let ip = ip("203.0.113.10");
    let valid_headers = auth_headers("valid-token");

    assert!(state.bridge_auth(&valid_headers, ip).is_ok());
    assert!(state.command_auth(&valid_headers, ip).is_ok());
    assert!(state.realtime_auth(&valid_headers, ip).is_ok());
}

#[test]
fn rejects_realtime_route_for_command_token() {
    let state = test_state(3);
    let ip = ip("203.0.113.10");
    let valid_headers = auth_headers("valid-token");

    assert_eq!(
        error_status(state.realtime_auth(&valid_headers, ip).unwrap_err()),
        StatusCode::FORBIDDEN
    );
}

#[test]
fn realtime_capacity_is_independent_and_released() {
    let state = test_state(3);
    let permit = state.acquire_realtime().unwrap();

    assert!(state.acquire_realtime().is_err());

    drop(permit);

    assert!(state.acquire_realtime().is_ok());
}
