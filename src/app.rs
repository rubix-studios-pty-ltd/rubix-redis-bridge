use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use redis::aio::ConnectionManager;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use tokio::sync::{OnceCell, Semaphore};
use tokio::time::timeout;
use tracing::{error, warn};

use crate::config::{BridgeConfig, RedisTargetConfig};
use crate::redis_value::encode_redis_value;
use crate::security::{RedisCommand, SecurityPolicy};

pub struct AppState {
    targets: HashMap<String, Arc<RedisTarget>>,
    security: SecurityPolicy,
    request_timeout: Duration,
}

struct RedisTarget {
    config: RedisTargetConfig,
    connection: OnceCell<ConnectionManager>,
    operation_limit: Semaphore,
}

impl fmt::Debug for AppState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppState")
            .field("target_count", &self.targets.len())
            .field("security", &self.security)
            .field("request_timeout", &self.request_timeout)
            .finish()
    }
}

impl fmt::Debug for RedisTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisTarget")
            .field("rrb_id", &self.config.rrb_id)
            .field("connection_string", &"[redacted]")
            .field("max_connections", &self.config.max_connections)
            .field("connection_initialized", &self.connection.get().is_some())
            .field("operation_limit", &self.config.max_connections)
            .finish()
    }
}

impl AppState {
    pub fn new(config: BridgeConfig) -> anyhow::Result<Self> {
        let targets = config
            .targets
            .into_iter()
            .map(|(token, target_config)| {
                (
                    token,
                    Arc::new(RedisTarget {
                        operation_limit: Semaphore::new(target_config.max_connections),
                        config: target_config,
                        connection: OnceCell::new(),
                    }),
                )
            })
            .collect();

        Ok(Self {
            targets,
            security: config.security,
            request_timeout: config.request_timeout,
        })
    }

    fn resolve_target(&self, headers: &HeaderMap) -> Result<Arc<RedisTarget>, ApiError> {
        let auth_header = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("Missing/Invalid authorization header"))?;

        let Some((scheme, token)) = auth_header.split_once(char::is_whitespace) else {
            return Err(ApiError::unauthorized(
                "Missing/Invalid authorization header",
            ));
        };

        if !scheme.eq_ignore_ascii_case("Bearer") || token.trim().is_empty() {
            return Err(ApiError::unauthorized(
                "Missing/Invalid authorization header",
            ));
        }

        let token = token.trim();

        let mut matched_target = None;

        for (stored_token, target) in &self.targets {
            if stored_token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1 {
                matched_target = Some(target.clone());
            }
        }

        matched_target.ok_or_else(|| ApiError::unauthorized("Invalid token"))
    }

    fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    fn base64(headers: &HeaderMap) -> bool {
        headers
            .get("upstash-encoding")
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(',')
                    .any(|entry| entry.trim().eq_ignore_ascii_case("base64"))
            })
            .unwrap_or(false)
    }
}

impl RedisTarget {
    async fn connection(&self) -> anyhow::Result<ConnectionManager> {
        let connection = self
            .connection
            .get_or_try_init(|| async {
                let client = redis::Client::open(self.config.connection_string.as_str())
                    .with_context(|| {
                        format!("Invalid Redis URL for target {}", self.config.rrb_id)
                    })?;

                client.get_connection_manager().await.with_context(|| {
                    format!("Failed to connect to Redis target {}", self.config.rrb_id)
                })
            })
            .await?;

        Ok(connection.clone())
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn gateway_timeout(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::GATEWAY_TIMEOUT,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        json_response(self.status, json!({ "error": self.message }))
    }
}

pub async fn root() -> impl IntoResponse {
    json_response(StatusCode::OK, json!("Rubix Redis Bridge"))
}

pub async fn healthz() -> impl IntoResponse {
    json_response(StatusCode::OK, json!({ "status": "ok" }))
}

pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    if state.targets.is_empty() {
        return ApiError::service_unavailable("No Redis targets configured").into_response();
    }

    json_response(
        StatusCode::OK,
        json!({
            "status": "ready",
            "target_count": state.targets.len()
        }),
    )
}

pub async fn command(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);
    let target = match state.resolve_target(&headers) {
        Ok(target) => target,
        Err(error) => return error.into_response(),
    };

    let Json(value) = match body {
        Ok(body) => body,
        Err(_) => return ApiError::bad_request("Invalid JSON body").into_response(),
    };

    let command = match state.security.parse_single_command(&value) {
        Ok(command) => command,
        Err(error) => return ApiError::bad_request(error.to_string()).into_response(),
    };

    match execute_command(target, command, base64_encoding, state.request_timeout()).await {
        Ok(value) => json_response(StatusCode::OK, json!({ "result": value })),
        Err(error) => error.into_response(),
    }
}

pub async fn pipeline(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);
    let target = match state.resolve_target(&headers) {
        Ok(target) => target,
        Err(error) => return error.into_response(),
    };

    let Json(value) = match body {
        Ok(body) => body,
        Err(_) => return ApiError::bad_request("Invalid JSON body").into_response(),
    };

    let commands = match state.security.parse_command_list(&value) {
        Ok(commands) => commands,
        Err(error) => return ApiError::bad_request(error.to_string()).into_response(),
    };

    match execute_pipeline(target, commands, base64_encoding, state.request_timeout()).await {
        Ok(response_items) => json_response(StatusCode::OK, Value::Array(response_items)),
        Err(error) => error.into_response(),
    }
}

async fn execute_pipeline(
    target: Arc<RedisTarget>,
    commands: Vec<RedisCommand>,
    base64_encoding: bool,
    request_timeout: Duration,
) -> Result<Vec<Value>, ApiError> {
    let target_id = target.config.rrb_id.clone();
    let target_id_for_task = target_id.clone();

    let result = timeout(request_timeout, async move {
        let _permit = target.operation_limit.acquire().await.map_err(|error| {
            error!(%error, target = %target_id_for_task, "Redis operation limiter closed");
            ApiError::service_unavailable("Redis backend unavailable")
        })?;

        let mut connection = target.connection().await.map_err(|error| {
            error!(%error, target = %target_id_for_task, "Redis connection failed");
            ApiError::service_unavailable("Redis backend unavailable")
        })?;

        let mut pipe = redis::pipe();

        for command in commands {
            pipe.cmd(command.name.as_str());

            for arg in command.args {
                pipe.arg(arg.as_slice());
            }
        }

        let result: redis::RedisResult<Vec<redis::RedisResult<redis::Value>>> =
            pipe.ignore_errors().query_async(&mut connection).await;

        result
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| match item {
                        Ok(value) => json!({
                            "result": encode_redis_value(value, base64_encoding)
                        }),
                        Err(error) => json!({
                            "error": clean_redis_error(error.to_string())
                        }),
                    })
                    .collect()
            })
            .map_err(redis_error_to_api_error)
    })
    .await;

    match result {
        Ok(result) => result,
        Err(_) => {
            warn!(
                target = %target_id,
                timeout_ms = request_timeout.as_millis(),
                "Redis pipeline timed out"
            );

            Err(ApiError::gateway_timeout("Redis pipeline timed out"))
        }
    }
}

pub async fn multi_exec(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);
    let target = match state.resolve_target(&headers) {
        Ok(target) => target,
        Err(error) => return error.into_response(),
    };

    let Json(value) = match body {
        Ok(body) => body,
        Err(_) => return ApiError::bad_request("Invalid JSON body").into_response(),
    };

    let commands = match state.security.parse_command_list(&value) {
        Ok(commands) => commands,
        Err(error) => return ApiError::bad_request(error.to_string()).into_response(),
    };

    match execute_transaction(target, commands, base64_encoding, state.request_timeout()).await {
        Ok(values) => {
            let response_items = values
                .into_iter()
                .map(|value| json!({ "result": value }))
                .collect();
            json_response(StatusCode::OK, Value::Array(response_items))
        }
        Err(error) => error.into_response(),
    }
}

async fn execute_command(
    target: Arc<RedisTarget>,
    command: RedisCommand,
    base64_encoding: bool,
    request_timeout: Duration,
) -> Result<Value, ApiError> {
    let target_id = target.config.rrb_id.clone();
    let target_id_for_task = target_id.clone();

    let result = timeout(request_timeout, async move {
        let _permit = target.operation_limit.acquire().await.map_err(|error| {
            error!(%error, target = %target_id_for_task, "Redis operation limiter closed");
            ApiError::service_unavailable("Redis backend unavailable")
        })?;

        let mut connection = target.connection().await.map_err(|error| {
            error!(%error, target = %target_id_for_task, "Redis connection failed");
            ApiError::service_unavailable("Redis backend unavailable")
        })?;

        let mut redis_command = redis::cmd(command.name.as_str());
        for arg in command.args {
            redis_command.arg(arg.as_slice());
        }

        let result: redis::RedisResult<redis::Value> =
            redis_command.query_async(&mut connection).await;
        result
            .map(|value| encode_redis_value(value, base64_encoding))
            .map_err(redis_error_to_api_error)
    })
    .await;

    match result {
        Ok(result) => result,
        Err(_) => {
            warn!(target = %target_id, timeout_ms = request_timeout.as_millis(), "Redis command timed out");
            Err(ApiError::gateway_timeout("Redis command timed out"))
        }
    }
}

async fn execute_transaction(
    target: Arc<RedisTarget>,
    commands: Vec<RedisCommand>,
    base64_encoding: bool,
    request_timeout: Duration,
) -> Result<Vec<Value>, ApiError> {
    let target_id = target.config.rrb_id.clone();
    let target_id_for_task = target_id.clone();

    let result = timeout(request_timeout, async move {
        let _permit = target.operation_limit.acquire().await.map_err(|error| {
            error!(%error, target = %target_id_for_task, "Redis operation limiter closed");
            ApiError::service_unavailable("Redis backend unavailable")
        })?;

        let mut connection = target.connection().await.map_err(|error| {
            error!(%error, target = %target_id_for_task, "Redis connection failed");
            ApiError::service_unavailable("Redis backend unavailable")
        })?;

        let mut pipe = redis::pipe();
        pipe.atomic();

        for command in commands {
            pipe.cmd(command.name.as_str());
            for arg in command.args {
                pipe.arg(arg.as_slice());
            }
        }

        let result: redis::RedisResult<Vec<redis::Value>> = pipe.query_async(&mut connection).await;
        result
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| encode_redis_value(value, base64_encoding))
                    .collect()
            })
            .map_err(redis_error_to_api_error)
    })
    .await;

    match result {
        Ok(result) => result,
        Err(_) => {
            warn!(target = %target_id, timeout_ms = request_timeout.as_millis(), "Redis transaction timed out");
            Err(ApiError::gateway_timeout("Redis transaction timed out"))
        }
    }
}

fn redis_error_to_api_error(error: redis::RedisError) -> ApiError {
    if is_backend_unavailable_error(&error) {
        error!(kind = ?error.kind(), code = ?error.code(), %error, "Redis backend unavailable");
        return ApiError::service_unavailable("Redis backend unavailable");
    }

    ApiError::bad_request(clean_redis_error(error.to_string()))
}

fn is_backend_unavailable_error(error: &redis::RedisError) -> bool {
    if error.is_io_error()
        || error.is_connection_dropped()
        || error.is_connection_refusal()
        || error.is_timeout()
    {
        return true;
    }

    if matches!(
        error.kind(),
        redis::ErrorKind::AuthenticationFailed | redis::ErrorKind::InvalidClientConfig
    ) {
        return true;
    }

    matches!(
        error.code(),
        Some("LOADING" | "TRYAGAIN" | "CLUSTERDOWN" | "MASTERDOWN")
    )
}

fn clean_redis_error(message: String) -> String {
    message
        .strip_prefix("ResponseError: ")
        .unwrap_or(message.as_str())
        .to_string()
}

fn json_response(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}
