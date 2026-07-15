use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Json, Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use futures_util::stream;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::security::CommandArg;

use super::{ApiError, AppState};

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub async fn subscribe(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(channel): Path<String>,
    headers: HeaderMap,
    body: Result<Json<Vec<CommandArg>>, JsonRejection>,
) -> Response {
    let client_ip = state.client_ip(&headers, addr);

    let route = match state.realtime_auth(&headers, client_ip) {
        Ok(route) => route,
        Err(error) => {
            state.metrics().request_denied("subscribe", "auth");
            return error.into_response();
        }
    };

    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => {
            state.metrics().request_denied("subscribe", "invalid_json");
            return ApiError::bad_request("Invalid JSON body").into_response();
        }
    };

    if !body.is_empty() {
        state.metrics().request_denied("subscribe", "invalid_body");
        return ApiError::bad_request("Subscription body must be an empty command array")
            .into_response();
    }

    if let Err(error) = validate_channel(&channel, state.security().max_arg_bytes) {
        state.metrics().request_denied("subscribe", "channel");
        return error.into_response();
    }

    let permit = match state.acquire_realtime() {
        Ok(permit) => permit,
        Err(error) => {
            warn!(%error, "Realtime connection limiter saturated");
            state.metrics().request_denied("subscribe", "capacity");
            return ApiError::too_many_requests("Realtime connection capacity exhausted")
                .into_response();
        }
    };

    let target = route.target();
    let target_id = target.id().to_owned();
    let setup = timeout(state.request_timeout(), async {
        let mut pubsub = target.pubsub().await?;
        pubsub
            .subscribe(channel.as_str())
            .await
            .with_context(|| format!("Failed to subscribe Redis target {target_id}"))?;
        Ok::<_, anyhow::Error>(pubsub)
    })
    .await;

    let pubsub = match setup {
        Ok(Ok(pubsub)) => pubsub,
        Ok(Err(error)) => {
            error!(%error, target = %target_id, "Redis realtime subscription failed");
            state.metrics().request_denied("subscribe", "backend");
            return ApiError::unavailable("Redis backend unavailable").into_response();
        }
        Err(_) => {
            warn!(
                target = %target_id,
                timeout_ms = state.request_timeout().as_millis(),
                "Redis realtime subscription setup timed out"
            );
            state.metrics().request_denied("subscribe", "timeout");
            return ApiError::timeout("Redis subscription setup timed out").into_response();
        }
    };

    let connection_guard = state.metrics().realtime_connection(target_id.clone());
    let subscribed = format!("subscribe,{channel},1");
    let (sink, messages) = pubsub.split();
    let initial =
        stream::once(async move { Ok::<Event, Infallible>(Event::default().data(subscribed)) });
    let messages = stream::unfold(
        (messages, sink, permit, connection_guard),
        |(mut messages, sink, permit, connection_guard)| async move {
            let message = messages.next().await?;
            let message_channel = message.get_channel_name();
            let payload = String::from_utf8_lossy(message.get_payload_bytes());
            let data = format!("message,{message_channel},{payload}");
            let event = Ok::<Event, Infallible>(Event::default().data(data));

            Some((event, (messages, sink, permit, connection_guard)))
        },
    );
    let stream = initial.chain(messages);

    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(KEEPALIVE_INTERVAL)
                .text("keep-alive"),
        )
        .into_response();

    response.headers_mut().insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );

    response
}

pub(crate) fn validate_channel(channel: &str, max_bytes: usize) -> Result<(), ApiError> {
    if channel.is_empty() {
        return Err(ApiError::bad_request("Realtime channel cannot be empty"));
    }

    if channel.len() > max_bytes {
        return Err(ApiError::bad_request(format!(
            "Realtime channel is too large. Maximum allowed bytes: {max_bytes}."
        )));
    }

    if channel
        .chars()
        .any(|character| matches!(character, ',' | '\r' | '\n' | '\0'))
    {
        return Err(ApiError::bad_request(
            "Realtime channel contains a character unsupported by the Upstash event protocol",
        ));
    }

    Ok(())
}
