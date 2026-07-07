use axum::http::HeaderMap;
use std::net::IpAddr;
use subtle::ConstantTimeEq;

use crate::app::ApiError;
use crate::app::{AppState, AuthRoute};
use crate::config::TokenHash;

use super::lockout::AuthFailure;

impl AppState {
    pub(crate) fn unauthorized(&self, ip: IpAddr, message: impl Into<String>) -> ApiError {
        self.metrics.auth_failed();

        let result = self.auth_lockout.record_failure(ip);

        match result {
            AuthFailure::Locked => {
                self.metrics.lockout_created();
                self.refresh_lockout_metrics();
                ApiError::too_many_requests("Too many failed authentication attempts")
            }
            AuthFailure::AlreadyLocked => {
                self.metrics.locked_request();
                self.refresh_lockout_metrics();
                ApiError::too_many_requests("Too many failed authentication attempts")
            }
            AuthFailure::EntryLimitReached => {
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
    ) -> Result<AuthRoute, ApiError> {
        let token = self.bearer_token(headers, ip)?;

        let token_hash = match self.hash_token.as_deref() {
            Some(key) => TokenHash::hmac_sha256(key, token),
            None => TokenHash::sha256(token),
        };

        let Some(route) = self.token_routes.get(&token_hash).cloned() else {
            return Err(self.unauthorized(ip, "Invalid token"));
        };

        self.auth_lockout.record_success(ip);
        self.refresh_lockout_metrics();

        Ok(route)
    }

    pub(crate) fn command_auth(
        &self,
        headers: &HeaderMap,
        ip: IpAddr,
    ) -> Result<AuthRoute, ApiError> {
        let route = self.bridge_auth(headers, ip)?;

        if !route.allows_command_route() {
            return Err(ApiError::forbidden(
                "Bearer token is not allowed to access command routes",
            ));
        }

        Ok(route)
    }
}
