use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

const PENDO_TRACK_URL: &str = "https://data.pendo.io/data/track";

#[derive(Clone)]
pub(crate) struct PendoTracker {
    client: reqwest::Client,
    integration_key: String,
}

impl PendoTracker {
    pub(crate) fn from_env() -> Option<Self> {
        let integration_key = std::env::var("RRB_PENDO_INTEGRATION_KEY").ok()?;
        let integration_key = integration_key.trim().to_string();

        if integration_key.is_empty() {
            return None;
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        Some(Self {
            client,
            integration_key,
        })
    }

    pub(crate) fn track(&self, event: &str, visitor_id: &str, account_id: &str, properties: Value) {
        let client = self.client.clone();
        let integration_key = self.integration_key.clone();
        let event = event.to_owned();
        let visitor_id = visitor_id.to_owned();
        let account_id = account_id.to_owned();

        tokio::spawn(async move {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let body = json!({
                "type": "track",
                "event": event,
                "visitorId": visitor_id,
                "accountId": account_id,
                "timestamp": timestamp,
                "properties": properties,
            });

            if let Err(error) = client
                .post(PENDO_TRACK_URL)
                .header("Content-Type", "application/json")
                .header("x-pendo-integration-key", &integration_key)
                .json(&body)
                .send()
                .await
            {
                debug!(%error, %event, "Failed to send Pendo track event");
            }
        });
    }
}
