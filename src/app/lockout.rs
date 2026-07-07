use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::warn;

#[derive(Debug)]
struct AuthState {
    failures: usize,
    window_started_at: Instant,
    last_seen_at: Instant,
    locked_until: Option<Instant>,
}

impl AuthState {
    fn new(now: Instant) -> Self {
        Self {
            failures: 0,
            window_started_at: now,
            last_seen_at: now,
            locked_until: None,
        }
    }

    fn is_stale(&self, now: Instant, failure_window: Duration) -> bool {
        match self.locked_until {
            Some(until) => until <= now,
            None => now.duration_since(self.last_seen_at) >= failure_window,
        }
    }

    fn reset_window(&mut self, now: Instant) {
        self.failures = 0;
        self.window_started_at = now;
        self.last_seen_at = now;
        self.locked_until = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthFailure {
    Disabled,
    Tracked,
    Locked,
    AlreadyLocked,
    EntryLimitReached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuthSnapshot {
    pub(crate) tracked_ips: usize,
    pub(crate) locked_ips: usize,
}

#[derive(Debug)]
pub(crate) struct AuthLockout {
    max_failures: usize,
    failure_window: Duration,
    lockout_duration: Duration,
    max_entries: usize,
    entries: Mutex<HashMap<IpAddr, AuthState>>,
}

impl AuthLockout {
    pub(crate) fn new(
        max_failures: usize,
        failure_window: Duration,
        lockout_duration: Duration,
        max_entries: usize,
    ) -> Self {
        Self {
            max_failures,
            failure_window,
            lockout_duration,
            max_entries,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.max_failures > 0
    }

    #[cfg(test)]
    fn is_locked_at(&self, ip: IpAddr, now: Instant) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");

        Self::cleanup_stale_entries(&mut entries, now, self.failure_window);

        entries
            .get(&ip)
            .and_then(|state| state.locked_until)
            .is_some_and(|until| until > now)
    }

    pub(crate) fn record_failure(&self, ip: IpAddr) -> AuthFailure {
        self.record_failure_at(ip, Instant::now())
    }

    fn record_failure_at(&self, ip: IpAddr, now: Instant) -> AuthFailure {
        if !self.is_enabled() {
            return AuthFailure::Disabled;
        }

        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");

        if entries.len() >= self.max_entries && !entries.contains_key(&ip) {
            Self::cleanup_stale_entries(&mut entries, now, self.failure_window);

            if entries.len() >= self.max_entries {
                warn!(
                    ip = %ip,
                    max_entries = self.max_entries,
                    "Auth lockout entry limit reached; not tracking new failed client IP"
                );

                return AuthFailure::EntryLimitReached;
            }
        }

        let state = entries
            .entry(ip)
            .or_insert_with(|| AuthState::new(now));

        if let Some(until) = state.locked_until {
            if until > now {
                return AuthFailure::AlreadyLocked;
            }

            state.reset_window(now);
        }

        if now.duration_since(state.window_started_at) >= self.failure_window {
            state.reset_window(now);
        }

        state.failures += 1;
        state.last_seen_at = now;

        if state.failures >= self.max_failures {
            let locked_until = now + self.lockout_duration;
            state.locked_until = Some(locked_until);

            warn!(
                ip = %ip,
                failures = state.failures,
                window_seconds = self.failure_window.as_secs(),
                lockout_seconds = self.lockout_duration.as_secs(),
                "Client IP locked out after failed authentication attempts"
            );

            return AuthFailure::Locked;
        }

        AuthFailure::Tracked
    }

    pub(crate) fn record_success(&self, ip: IpAddr) {
        if !self.is_enabled() {
            return;
        }

        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");
        entries.remove(&ip);
    }

    pub(crate) fn snapshot(&self) -> AuthSnapshot {
        self.snapshot_at(Instant::now())
    }

    fn snapshot_at(&self, now: Instant) -> AuthSnapshot {
        if !self.is_enabled() {
            return AuthSnapshot {
                tracked_ips: 0,
                locked_ips: 0,
            };
        }

        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");

        Self::cleanup_stale_entries(&mut entries, now, self.failure_window);

        let locked_ips = entries
            .values()
            .filter(|state| matches!(state.locked_until, Some(until) if until > now))
            .count();

        AuthSnapshot {
            tracked_ips: entries.len(),
            locked_ips,
        }
    }

    fn cleanup_stale_entries(
        entries: &mut HashMap<IpAddr, AuthState>,
        now: Instant,
        failure_window: Duration,
    ) {
        entries.retain(|_, state| !state.is_stale(now, failure_window));
    }
}

#[cfg(test)]
#[path = "lockout_tests.rs"]
mod tests;
