use serde::Serialize;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

static TRACKER: OnceLock<Option<PendoTracker>> = OnceLock::new();

struct PendoTracker {
    client: reqwest::Client,
    integration_key: String,
    endpoint: String,
}

#[derive(Serialize)]
struct TrackPayload {
    #[serde(rename = "type")]
    event_type: &'static str,
    event: String,
    #[serde(rename = "visitorId")]
    visitor_id: &'static str,
    #[serde(rename = "accountId")]
    account_id: String,
    timestamp: u64,
    properties: serde_json::Value,
}

pub(crate) fn init() {
    TRACKER.get_or_init(|| {
        let integration_key = std::env::var("PENDO_INTEGRATION_KEY").ok()?;
        let integration_key = integration_key.trim().to_string();
        if integration_key.is_empty() {
            return None;
        }

        let data_host = std::env::var("PENDO_DATA_HOST")
            .unwrap_or_else(|_| "https://data.pendo.io".to_string());

        Some(PendoTracker {
            client: reqwest::Client::new(),
            integration_key,
            endpoint: format!("{}/data/track", data_host.trim_end_matches('/')),
        })
    });
}

pub(crate) fn track(event: &str, account_id: &str, properties: serde_json::Value) {
    let Some(Some(tracker)) = TRACKER.get() else {
        return;
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let payload = TrackPayload {
        event_type: "track",
        event: event.to_owned(),
        visitor_id: "system",
        account_id: account_id.to_owned(),
        timestamp,
        properties,
    };

    let client = tracker.client.clone();
    let endpoint = tracker.endpoint.clone();
    let integration_key = tracker.integration_key.clone();

    tokio::spawn(async move {
        let result = client
            .post(&endpoint)
            .header("x-pendo-integration-key", &integration_key)
            .json(&payload)
            .send()
            .await;

        if let Err(error) = result {
            warn!(%error, "Failed to send Pendo track event");
        }
    });
}
