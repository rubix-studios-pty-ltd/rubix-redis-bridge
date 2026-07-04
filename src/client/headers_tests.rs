use std::net::IpAddr;

use axum::http::{HeaderMap, HeaderValue};

use super::forwarded_ip;

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

#[test]
fn cloudflare_header() {
    let mut headers = HeaderMap::new();

    headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.10"));
    headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.20"));

    assert_eq!(forwarded_ip(&headers), Some(ip("203.0.113.10")));
}

#[test]
fn x_forwarded_for() {
    let mut headers = HeaderMap::new();

    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("203.0.113.10, 198.51.100.20"),
    );

    assert_eq!(forwarded_ip(&headers), Some(ip("203.0.113.10")));
}

#[test]
fn forwarded_header() {
    let mut headers = HeaderMap::new();

    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=203.0.113.10;proto=https;host=example.com"),
    );

    assert_eq!(forwarded_ip(&headers), Some(ip("203.0.113.10")));
}

#[test]
fn ipv6_forwarded_header() {
    let mut headers = HeaderMap::new();

    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=\"[2001:db8:abcd::1]:443\";proto=https"),
    );

    assert_eq!(forwarded_ip(&headers), Some(ip("2001:db8:abcd::1")));
}
