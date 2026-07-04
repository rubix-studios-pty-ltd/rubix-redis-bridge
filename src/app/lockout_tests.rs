use std::net::IpAddr;
use std::time::{Duration, Instant};

use super::AuthLockout;

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

fn lockout() -> AuthLockout {
    AuthLockout::new(3, Duration::from_secs(60), Duration::from_secs(300), 1024)
}

#[test]
fn locks_after_failures_inside_window() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(10)));
    assert!(lockout.record_failure_at(ip, now + Duration::from_secs(20)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(21)));
}

#[test]
fn resets_failures_after_window() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(10)));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(61)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(62)));
}

#[test]
fn removes_entry_after_success() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    lockout.record_success(ip);
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(2)));
}

#[test]
fn unlocks_after_lockout_duration() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert!(lockout.record_failure_at(ip, now + Duration::from_secs(2)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(3)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(303)));
}

#[test]
fn stops_tracking_new_ips_when_entry_limit_is_full() {
    let lockout = AuthLockout::new(3, Duration::from_secs(60), Duration::from_secs(300), 1);
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip("203.0.113.10"), now));
    assert!(!lockout.record_failure_at(ip("203.0.113.11"), now + Duration::from_secs(1)));
    assert!(
        lockout
            .entries
            .lock()
            .unwrap()
            .contains_key(&ip("203.0.113.10"))
    );
    assert!(
        !lockout
            .entries
            .lock()
            .unwrap()
            .contains_key(&ip("203.0.113.11"))
    );
}

#[test]
fn cleanup_allows_new_ip_after_window_expiry() {
    let lockout = AuthLockout::new(3, Duration::from_secs(60), Duration::from_secs(300), 1);
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip("203.0.113.10"), now));
    assert!(!lockout.record_failure_at(ip("203.0.113.11"), now + Duration::from_secs(61)));
    assert!(
        !lockout
            .entries
            .lock()
            .unwrap()
            .contains_key(&ip("203.0.113.10"))
    );
    assert!(
        lockout
            .entries
            .lock()
            .unwrap()
            .contains_key(&ip("203.0.113.11"))
    );
}

#[test]
fn disabled_lockout_never_tracks_or_locks() {
    let lockout = AuthLockout::new(0, Duration::from_secs(60), Duration::from_secs(300), 1024);

    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(2)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(3)));
    assert!(lockout.entries.lock().unwrap().is_empty());
}

#[test]
fn returns_locked_for_repeated_failures_while_locked() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert!(lockout.record_failure_at(ip, now + Duration::from_secs(2)));

    assert!(lockout.record_failure_at(ip, now + Duration::from_secs(3)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(4)));
}

#[test]
fn boundary_failure_after_window_starts_new_window() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(10)));

    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(60)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(61)));

    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(61)));
    assert!(lockout.record_failure_at(ip, now + Duration::from_secs(62)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(63)));
}

#[test]
fn record_success_clears_locked_state() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert!(!lockout.record_failure_at(ip, now));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert!(lockout.record_failure_at(ip, now + Duration::from_secs(2)));

    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(3)));

    lockout.record_success(ip);

    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(4)));
    assert!(!lockout.record_failure_at(ip, now + Duration::from_secs(5)));
}
