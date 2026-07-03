use std::sync::Arc;

use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

use super::error::ApiError;
use super::state::{AppState, RedisTarget};

impl AppState {
    pub(crate) fn unauthorized(&self, message: impl Into<String>) -> ApiError {
        self.metrics.inc_auth_failed();
        ApiError::unauthorized(message)
    }

    fn bearer_token<'a>(&self, headers: &'a HeaderMap) -> Result<&'a str, ApiError> {
        let auth_header = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| self.unauthorized("Invalid authorization header"))?;

        let Some((scheme, token)) = auth_header.split_once(char::is_whitespace) else {
            return Err(self.unauthorized("Invalid authorization header"));
        };

        let token = token.trim();

        if !scheme.eq_ignore_ascii_case("Bearer") || token.is_empty() {
            return Err(self.unauthorized("Invalid authorization header"));
        }

        Ok(token)
    }

    pub(crate) fn metrics_auth(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let Some(metrics_token) = self.metrics_token.as_deref() else {
            return Err(self.unauthorized("Metrics authentication is not configured"));
        };

        let token = self.bearer_token(headers)?;

        if metrics_token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() != 1 {
            return Err(self.unauthorized("Invalid token"));
        }

        Ok(())
    }

    pub(crate) fn bridge_auth(&self, headers: &HeaderMap) -> Result<Arc<RedisTarget>, ApiError> {
        let token = self.bearer_token(headers)?;
        let mut matched_target = None;

        for (stored_token, target) in &self.targets {
            if stored_token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1 {
                matched_target = Some(target.clone());
            }
        }

        matched_target.ok_or_else(|| self.unauthorized("Invalid token"))
    }
}
