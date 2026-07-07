use std::net::IpAddr;

use crate::client::{parse_ip_candidate, prefix_mask_v4, prefix_mask_v6};

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

#[test]
fn plain_ip() {
    assert_eq!(parse_ip_candidate("203.0.113.10"), Some(ip("203.0.113.10")));
    assert_eq!(parse_ip_candidate("2001:db8::1"), Some(ip("2001:db8::1")));
}

#[test]
fn socket_address() {
    assert_eq!(
        parse_ip_candidate("203.0.113.10:443"),
        Some(ip("203.0.113.10"))
    );

    assert_eq!(
        parse_ip_candidate("[2001:db8::1]:443"),
        Some(ip("2001:db8::1"))
    );
}

#[test]
fn ignores_empty_unknown() {
    assert_eq!(parse_ip_candidate(""), None);
    assert_eq!(parse_ip_candidate("unknown"), None);
    assert_eq!(parse_ip_candidate("UNKNOWN"), None);
}

#[test]
fn builds_ipv4_masks() {
    assert_eq!(prefix_mask_v4(0), 0);
    assert_eq!(prefix_mask_v4(32), u32::MAX);
    assert_eq!(prefix_mask_v4(24), 0xffffff00);
}

#[test]
fn builds_ipv6_masks() {
    assert_eq!(prefix_mask_v6(0), 0);
    assert_eq!(prefix_mask_v6(128), u128::MAX);
    assert_eq!(prefix_mask_v6(64), 0xffffffffffffffff0000000000000000);
}
