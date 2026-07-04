use std::net::IpAddr;
use std::time::{Duration, Instant};

use super::{AuthFailureResult, AuthLockout};

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

fn lockout() -> AuthLockout {
    AuthLockout::new(3, Duration::from_secs(60), Duration::from_secs(300), 1024)
}

fn assert_locked(result: AuthFailureResult) {
    assert!(matches!(result, AuthFailureResult::Locked));
}

fn assert_not_locked(result: AuthFailureResult) {
    assert!(!matches!(result, AuthFailureResult::Locked));
}

#[test]
fn lock_failures_in_window() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(10)));
    assert_locked(lockout.record_failure_at(ip, now + Duration::from_secs(20)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(21)));
}

#[test]
fn clear_failures_after_window() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(10)));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(61)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(62)));
}

#[test]
fn clear_lockout_after_success() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    lockout.record_success(ip);
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(2)));
}

#[test]
fn clear_lockout_after_duration() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert_locked(lockout.record_failure_at(ip, now + Duration::from_secs(2)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(3)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(303)));
}

#[test]
fn ignore_ips_lockout_full() {
    let lockout = AuthLockout::new(3, Duration::from_secs(60), Duration::from_secs(300), 1);
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip("203.0.113.10"), now));
    assert_not_locked(lockout.record_failure_at(ip("203.0.113.11"), now + Duration::from_secs(1)));

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
fn allow_ips_lockout_cleanup() {
    let lockout = AuthLockout::new(3, Duration::from_secs(60), Duration::from_secs(300), 1);
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip("203.0.113.10"), now));
    assert_not_locked(lockout.record_failure_at(ip("203.0.113.11"), now + Duration::from_secs(61)));

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
fn skip_lockout_when_disabled() {
    let lockout = AuthLockout::new(0, Duration::from_secs(60), Duration::from_secs(300), 1024);

    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(2)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(3)));
    assert!(lockout.entries.lock().unwrap().is_empty());
}

#[test]
fn keep_locked_additional_failures() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert_locked(lockout.record_failure_at(ip, now + Duration::from_secs(2)));

    let _ = lockout.record_failure_at(ip, now + Duration::from_secs(3));

    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(4)));
}

#[test]
fn start_new_failure_window() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(10)));

    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(60)));
    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(61)));

    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(61)));
    assert_locked(lockout.record_failure_at(ip, now + Duration::from_secs(62)));
    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(63)));
}

#[test]
fn clear_locked_state_on_success() {
    let lockout = lockout();
    let ip = ip("203.0.113.10");
    let now = Instant::now();

    assert_not_locked(lockout.record_failure_at(ip, now));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(1)));
    assert_locked(lockout.record_failure_at(ip, now + Duration::from_secs(2)));

    assert!(lockout.is_locked_at(ip, now + Duration::from_secs(3)));

    lockout.record_success(ip);

    assert!(!lockout.is_locked_at(ip, now + Duration::from_secs(4)));
    assert_not_locked(lockout.record_failure_at(ip, now + Duration::from_secs(5)));
}
