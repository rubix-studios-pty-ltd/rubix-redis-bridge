use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Json, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use tracing::error;

use crate::metrics::Metrics;

use super::error::ApiError;
use super::redis_exec::{execute_command, execute_pipeline, execute_transaction};
use super::response::json_response;
use super::state::AppState;

pub async fn root() -> impl IntoResponse {
    json_response(
        StatusCode::OK,
        json!({
            "status": "ok"
        }),
    )
}

pub async fn healthz() -> impl IntoResponse {
    json_response(StatusCode::OK, json!({ "status": "ok" }))
}

pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    if !state.has_targets() {
        return ApiError::unavailable("No Redis targets configured").into_response();
    }

    json_response(
        StatusCode::OK,
        json!({
            "status": "ready",
            "target_count": state.target_count()
        }),
    )
}

pub async fn metrics(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = state.metrics_auth(&headers, addr.ip()) {
        return error.into_response();
    }

    match state.metrics().render() {
        Ok(body) => (
            StatusCode::OK,
            [(CONTENT_TYPE, Metrics::content_type())],
            body,
        )
            .into_response(),
        Err(error) => {
            error!(%error, "Failed to render Prometheus metrics");
            ApiError::unavailable("Metrics unavailable").into_response()
        }
    }
}

pub async fn command(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);

    let target = match state.bridge_auth(&headers, addr.ip()) {
        Ok(target) => target,
        Err(error) => return error.into_response(),
    };

    let Json(value) = match body {
        Ok(body) => body,
        Err(_) => return ApiError::bad_request("Invalid JSON body").into_response(),
    };

    let command = match state.security().parse_command(&value) {
        Ok(command) => command,
        Err(error) => {
            state.metrics().command_denied(target.id(), "single");
            return ApiError::bad_request(error.to_string()).into_response();
        }
    };

    match execute_command(
        target,
        command,
        base64_encoding,
        state.request_timeout(),
        state.metrics().clone(),
    )
    .await
    {
        Ok(value) => json_response(StatusCode::OK, json!({ "result": value })),
        Err(error) => error.into_response(),
    }
}

pub async fn pipeline(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);

    let target = match state.bridge_auth(&headers, addr.ip()) {
        Ok(target) => target,
        Err(error) => return error.into_response(),
    };

    let Json(value) = match body {
        Ok(body) => body,
        Err(_) => return ApiError::bad_request("Invalid JSON body").into_response(),
    };

    let commands = match state.security().parse_command_list(&value) {
        Ok(commands) => commands,
        Err(error) => {
            state.metrics().command_denied(target.id(), "pipeline");
            return ApiError::bad_request(error.to_string()).into_response();
        }
    };

    match execute_pipeline(
        target,
        commands,
        base64_encoding,
        state.request_timeout(),
        state.metrics().clone(),
    )
    .await
    {
        Ok(response_items) => json_response(StatusCode::OK, Value::Array(response_items)),
        Err(error) => error.into_response(),
    }
}

pub async fn multi_exec(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);

    let target = match state.bridge_auth(&headers, addr.ip()) {
        Ok(target) => target,
        Err(error) => return error.into_response(),
    };

    let Json(value) = match body {
        Ok(body) => body,
        Err(_) => return ApiError::bad_request("Invalid JSON body").into_response(),
    };

    let commands = match state.security().parse_command_list(&value) {
        Ok(commands) => commands,
        Err(error) => {
            state.metrics().command_denied(target.id(), "multi_exec");
            return ApiError::bad_request(error.to_string()).into_response();
        }
    };

    match execute_transaction(
        target,
        commands,
        base64_encoding,
        state.request_timeout(),
        state.metrics().clone(),
    )
    .await
    {
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
