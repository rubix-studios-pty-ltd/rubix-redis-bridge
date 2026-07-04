use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::app::error::ApiError;
use crate::client::TrustedProxies;
use crate::config::{BridgeConfig, RedisTargetConfig};
use crate::security::SecurityPolicy;

use super::AppState;

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
    let mut targets = HashMap::new();

    targets.insert(
        "valid-token".to_string(),
        RedisTargetConfig {
            rrb_id: "test_redis".to_string(),
            connection_string: "redis://default:password@127.0.0.1:6379".to_string(),
            max_connections: 1,
        },
    );

    let mut allowed_commands = HashSet::new();
    allowed_commands.insert("PING".to_string());

    AppState::new(BridgeConfig {
        host: "127.0.0.1".to_string(),
        port: 8080,
        targets,
        security: SecurityPolicy {
            allowed_commands,
            blocked_commands: HashSet::new(),
            max_pipeline_commands: 10,
            max_command_args: 16,
            max_arg_bytes: 1024,
            upstash_ratelimit: false,
        },
        max_body_bytes: 1024,
        max_concurrency: 16,
        request_timeout: Duration::from_millis(500),
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
fn reject_token_ip_locked() {
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

    assert_eq!(
        error_status(state.bridge_auth(&valid_headers, ip).unwrap_err()),
        StatusCode::TOO_MANY_REQUESTS
    );
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
