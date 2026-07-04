use std::net::IpAddr;
use std::sync::Arc;

use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

use super::error::ApiError;
use super::lockout::AuthFailureResult;
use super::state::{AppState, RedisTarget};

impl AppState {
    pub(crate) fn unauthorized(&self, ip: IpAddr, message: impl Into<String>) -> ApiError {
        self.metrics.auth_failed();

        let result = self.auth_lockout.record_failure(ip);

        match result {
            AuthFailureResult::Locked => {
                self.metrics.lockout_created();
                self.refresh_lockout_metrics();
                ApiError::too_many_requests("Too many failed authentication attempts")
            }
            AuthFailureResult::AlreadyLocked => {
                self.metrics.locked_request();
                self.refresh_lockout_metrics();
                ApiError::too_many_requests("Too many failed authentication attempts")
            }
            AuthFailureResult::EntryLimitReached => {
                self.metrics.lockout_entry_limit();
                self.refresh_lockout_metrics();
                ApiError::unauthorized(message)
            }
            _ => {
                self.refresh_lockout_metrics();
                ApiError::unauthorized(message)
            }
        }
    }

    fn check_lockout(&self, ip: IpAddr) -> Result<(), ApiError> {
        if self.auth_lockout.is_locked(ip) {
            self.metrics.locked_request();
            self.refresh_lockout_metrics();

            return Err(ApiError::too_many_requests(
                "Too many failed authentication attempts",
            ));
        }

        self.refresh_lockout_metrics();

        Ok(())
    }

    fn bearer_token<'a>(&self, headers: &'a HeaderMap, ip: IpAddr) -> Result<&'a str, ApiError> {
        let auth_header = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| self.unauthorized(ip, "Invalid authorization header"))?;

        let Some((scheme, token)) = auth_header.split_once(char::is_whitespace) else {
            return Err(self.unauthorized(ip, "Invalid authorization header"));
        };

        let token = token.trim();

        if !scheme.eq_ignore_ascii_case("Bearer") || token.is_empty() {
            return Err(self.unauthorized(ip, "Invalid authorization header"));
        }

        Ok(token)
    }

    pub(crate) fn refresh_lockout_metrics(&self) {
        let snapshot = self.auth_lockout.snapshot();

        self.metrics
            .set_lockout_state(snapshot.tracked_ips, snapshot.locked_ips);
    }

    pub(crate) fn metrics_auth(&self, headers: &HeaderMap, ip: IpAddr) -> Result<(), ApiError> {
        self.check_lockout(ip)?;

        let Some(metrics_token) = self.metrics_token.as_deref() else {
            return Err(ApiError::unauthorized(
                "Metrics authentication is not configured",
            ));
        };

        let token = self.bearer_token(headers, ip)?;

        if metrics_token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() != 1 {
            return Err(self.unauthorized(ip, "Invalid token"));
        }

        self.auth_lockout.record_success(ip);
        self.refresh_lockout_metrics();

        Ok(())
    }

    pub(crate) fn bridge_auth(
        &self,
        headers: &HeaderMap,
        ip: IpAddr,
    ) -> Result<Arc<RedisTarget>, ApiError> {
        self.check_lockout(ip)?;

        let token = self.bearer_token(headers, ip)?;
        let mut matched_target = None;

        for (stored_token, target) in &self.targets {
            if stored_token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1 {
                matched_target = Some(target.clone());
            }
        }

        let Some(target) = matched_target else {
            return Err(self.unauthorized(ip, "Invalid token"));
        };

        self.auth_lockout.record_success(ip);
        self.refresh_lockout_metrics();

        Ok(target)
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
