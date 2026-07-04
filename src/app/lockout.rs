use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::warn;

#[derive(Debug)]
struct AuthFailureState {
    failures: usize,
    locked_until: Option<Instant>,
}

#[derive(Debug)]
pub(crate) struct AuthLockout {
    max_failures: usize,
    lockout_duration: Duration,
    entries: Mutex<HashMap<IpAddr, AuthFailureState>>,
}

impl AuthLockout {
    pub(crate) fn new(max_failures: usize, lockout_duration: Duration) -> Self {
        Self {
            max_failures,
            lockout_duration,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.max_failures > 0
    }

    pub(crate) fn is_locked(&self, ip: IpAddr) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let now = Instant::now();
        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");

        let Some(state) = entries.get(&ip) else {
            return false;
        };

        match state.locked_until {
            Some(until) if until > now => true,
            Some(_) => {
                entries.remove(&ip);
                false
            }
            None => false,
        }
    }

    pub(crate) fn record_failure(&self, ip: IpAddr) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let now = Instant::now();
        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");
        let state = entries.entry(ip).or_insert(AuthFailureState {
            failures: 0,
            locked_until: None,
        });

        if let Some(until) = state.locked_until {
            if until > now {
                return true;
            }

            state.failures = 0;
            state.locked_until = None;
        }

        state.failures += 1;

        if state.failures >= self.max_failures {
            let locked_until = now + self.lockout_duration;
            state.locked_until = Some(locked_until);
        
            warn!(
                ip = %ip,
                failures = state.failures,
                lockout_seconds = self.lockout_duration.as_secs(),
                "Client IP locked out after failed authentication attempts"
            );
        
            return true;
        }

        false
    }

    pub(crate) fn record_success(&self, ip: IpAddr) {
        if !self.is_enabled() {
            return;
        }

        let mut entries = self.entries.lock().expect("auth lockout mutex poisoned");
        entries.remove(&ip);
    }
}
