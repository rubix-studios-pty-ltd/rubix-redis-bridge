use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Json, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;

use crate::metrics::Metrics;
use crate::redis::{
    CommandResponse, PipelineResponse, TransactionResponse, execute_command, execute_pipeline,
    execute_transaction,
};
use crate::security::CommandArg;

use super::error::ApiError;
use super::response::{ResponseError, json_response, serialized_response};
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

fn response_or_denied(
    state: &AppState,
    route: &str,
    result: Result<Response, ResponseError>,
) -> Response {
    match result {
        Ok(response) => response,
        Err(error) => {
            state.metrics().request_denied(route, error.metric_reason());
            error.into_api_error().into_response()
        }
    }
}

pub async fn metrics(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let client_ip = state.client_ip(&headers, addr);

    if let Err(error) = state.metrics_auth(&headers, client_ip) {
        state.metrics().request_denied("metrics", "auth");
        return error.into_response();
    }

    state.refresh_lockout_metrics();

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
    body: Result<Json<Vec<CommandArg>>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);
    let client_ip = state.client_ip(&headers, addr);

    let route = match state.command_auth(&headers, client_ip) {
        Ok(target) => target,
        Err(error) => {
            state.metrics().request_denied("command", "auth");
            return error.into_response();
        }
    };

    let Json(command_body) = match body {
        Ok(body) => body,
        Err(_) => {
            state.metrics().request_denied("command", "invalid_json");
            return ApiError::bad_request("Invalid JSON body").into_response();
        }
    };

    let command = match state
        .security()
        .parse_command(&command_body, route.token_type())
    {
        Ok(command) => command,
        Err(error) => {
            let denial_reason = error.to_string();
            state
                .metrics()
                .command_denied(route.target().id(), "single");
            state.metrics().request_denied("command", "policy");
            crate::pendo::track(
                "command_denied_by_policy",
                "system",
                route.target().id(),
                serde_json::json!({
                    "target_id": route.target().id(),
                    "operation_type": "single",
                    "denial_reason": &denial_reason,
                }),
            );
            return ApiError::bad_request(denial_reason).into_response();
        }
    };

    let command_name = command.name.clone();

    match execute_command(
        route.target(),
        command,
        state.request_timeout(),
        state.acquire_timeout(),
        state.metrics().clone(),
    )
    .await
    {
        Ok(value) => {
            let target = route.target();
            crate::pendo::track(
                "command_executed",
                "system",
                target.id(),
                serde_json::json!({
                    "target_id": target.id(),
                    "command_name": &command_name,
                    "base64_encoding": base64_encoding,
                    "operation_type": "command",
                }),
            );
            response_or_denied(
                &state,
                "command",
                serialized_response(
                    StatusCode::OK,
                    &CommandResponse {
                        result: &value,
                        base64_encoding,
                    },
                    state.max_response_bytes(),
                ),
            )
        }
        Err(error) => error.into_response(),
    }
}

pub async fn pipeline(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Vec<Vec<CommandArg>>>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);
    let client_ip = state.client_ip(&headers, addr);

    let route = match state.command_auth(&headers, client_ip) {
        Ok(target) => target,
        Err(error) => {
            state.metrics().request_denied("pipeline", "auth");
            return error.into_response();
        }
    };

    let Json(command_body) = match body {
        Ok(body) => body,
        Err(_) => {
            state.metrics().request_denied("pipeline", "invalid_json");
            return ApiError::bad_request("Invalid JSON body").into_response();
        }
    };

    let commands = match state
        .security()
        .parse_command_list(&command_body, route.token_type())
    {
        Ok(commands) => commands,
        Err(error) => {
            let denial_reason = error.to_string();
            state
                .metrics()
                .command_denied(route.target().id(), "pipeline");
            state.metrics().request_denied("pipeline", "policy");
            crate::pendo::track(
                "command_denied_by_policy",
                "system",
                route.target().id(),
                serde_json::json!({
                    "target_id": route.target().id(),
                    "operation_type": "pipeline",
                    "denial_reason": &denial_reason,
                }),
            );
            return ApiError::bad_request(denial_reason).into_response();
        }
    };

    let command_count = commands.len();

    match execute_pipeline(
        route.target(),
        commands,
        state.request_timeout(),
        state.acquire_timeout(),
        state.metrics().clone(),
    )
    .await
    {
        Ok(response_items) => {
            let target = route.target();
            crate::pendo::track(
                "pipeline_executed",
                "system",
                target.id(),
                serde_json::json!({
                    "target_id": target.id(),
                    "command_count": command_count,
                    "base64_encoding": base64_encoding,
                    "operation_type": "pipeline",
                }),
            );
            response_or_denied(
                &state,
                "pipeline",
                serialized_response(
                    StatusCode::OK,
                    &PipelineResponse {
                        items: &response_items,
                        base64_encoding,
                    },
                    state.max_response_bytes(),
                ),
            )
        }
        Err(error) => error.into_response(),
    }
}

pub async fn multi_exec(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Vec<Vec<CommandArg>>>, JsonRejection>,
) -> Response {
    let base64_encoding = AppState::base64(&headers);
    let client_ip = state.client_ip(&headers, addr);

    let route = match state.command_auth(&headers, client_ip) {
        Ok(target) => target,
        Err(error) => {
            state.metrics().request_denied("multi_exec", "auth");
            return error.into_response();
        }
    };

    let Json(command_body) = match body {
        Ok(body) => body,
        Err(_) => {
            state.metrics().request_denied("multi_exec", "invalid_json");
            return ApiError::bad_request("Invalid JSON body").into_response();
        }
    };

    let commands = match state
        .security()
        .parse_command_list(&command_body, route.token_type())
    {
        Ok(commands) => commands,
        Err(error) => {
            let denial_reason = error.to_string();
            state
                .metrics()
                .command_denied(route.target().id(), "multi_exec");
            state.metrics().request_denied("multi_exec", "policy");
            crate::pendo::track(
                "command_denied_by_policy",
                "system",
                route.target().id(),
                serde_json::json!({
                    "target_id": route.target().id(),
                    "operation_type": "multi_exec",
                    "denial_reason": &denial_reason,
                }),
            );
            return ApiError::bad_request(denial_reason).into_response();
        }
    };

    let command_count = commands.len();

    match execute_transaction(
        route.target(),
        commands,
        state.request_timeout(),
        state.acquire_timeout(),
        state.metrics().clone(),
    )
    .await
    {
        Ok(values) => {
            let target = route.target();
            crate::pendo::track(
                "transaction_executed",
                "system",
                target.id(),
                serde_json::json!({
                    "target_id": target.id(),
                    "command_count": command_count,
                    "base64_encoding": base64_encoding,
                    "operation_type": "multi_exec",
                }),
            );
            response_or_denied(
                &state,
                "multi_exec",
                serialized_response(
                    StatusCode::OK,
                    &TransactionResponse {
                        values: &values,
                        base64_encoding,
                    },
                    state.max_response_bytes(),
                ),
            )
        }
        Err(error) => error.into_response(),
    }
}
