mod app;
mod client;
mod commands;
mod config;
mod metrics;
mod security;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::app::{AppState, command, healthz, metrics, multi_exec, pipeline, readyz, root};
use crate::config::BridgeConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = BridgeConfig::from_env().context("Failed to load bridge configuration")?;
    let bind: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .context("Invalid bind address")?;

    let body_limit = config.max_body_bytes;
    let max_concurrency = config.max_concurrency;
    let state = Arc::new(AppState::new(config)?);

    info!(
        bind = %bind,
        target_count = state.target_count(),
        max_concurrency,
        body_limit,
        "Redis bridge starting"
    );

    let health = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics));

    let api = Router::new()
        .route("/", get(root).post(command))
        .route("/pipeline", post(pipeline))
        .route("/multi-exec", post(multi_exec))
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(ConcurrencyLimitLayer::new(max_concurrency));

    let app = health
        .merge(api)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("Failed to bind to {bind}"))?;

    info!(%bind, "Redis bridge listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("Server error")?;

    Ok(())
}

fn init_tracing() {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "rubix_redis_bridge=info,tower_http=info,axum=warn".to_string());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(signal) => signal,
        Err(error) => {
            warn!(%error, "Failed to install SIGTERM handler; Falling back to SIGINT only");
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result {
                warn!(%error, "Failed while waiting for SIGINT");
            } else {
                info!("Received SIGINT; Starting graceful shutdown");
            }
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM; Starting graceful shutdown");
        }
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        warn!(%error, "Failed to install ctrl_c signal handler");
    }
}
