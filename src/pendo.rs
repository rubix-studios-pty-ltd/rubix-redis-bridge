use serde_json::{Value, json};
use std::sync::LazyLock;
use tracing::warn;

const PENDO_TRACK_URL: &str = "https://data.pendo.io/data/track";

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

static INTEGRATION_KEY: LazyLock<Option<String>> = LazyLock::new(|| {
    std::env::var("PENDO_INTEGRATION_KEY")
        .ok()
        .filter(|v| !v.is_empty())
});

/// Sends a Pendo Track Event asynchronously without blocking the caller.
/// Failures are logged but never propagate to application logic.
pub(crate) fn track(event: &str, visitor_id: &str, account_id: &str, properties: Value) {
    let Some(key) = INTEGRATION_KEY.as_deref() else {
        return;
    };

    let body = json!({
        "type": "track",
        "event": event,
        "visitorId": visitor_id,
        "accountId": account_id,
        "timestamp": timestamp_ms(),
        "properties": properties,
    });

    let key = key.to_owned();

    tokio::spawn(async move {
        let result = HTTP_CLIENT
            .post(PENDO_TRACK_URL)
            .header("x-pendo-integration-key", &key)
            .json(&body)
            .send()
            .await;

        match result {
            Ok(response) if !response.status().is_success() => {
                warn!(
                    status = %response.status(),
                    "Pendo track event request returned non-success status"
                );
            }
            Err(error) => {
                warn!(%error, "Pendo track event request failed");
            }
            _ => {}
        }
    });
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
