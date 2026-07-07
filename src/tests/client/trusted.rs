use std::net::IpAddr;

use axum::http::{HeaderMap, HeaderValue};

use crate::client::TrustedProxies;

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

#[test]
fn trusts_proxy_ip() {
    let trusted = TrustedProxies::parse("127.0.0.1").unwrap();
    let mut headers = HeaderMap::new();

    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));

    assert_eq!(
        trusted.resolve(&headers, ip("127.0.0.1")),
        ip("203.0.113.10")
    );
}

#[test]
fn ignores_untrusted_peer() {
    let trusted = TrustedProxies::parse("127.0.0.1").unwrap();
    let mut headers = HeaderMap::new();

    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));

    assert_eq!(
        trusted.resolve(&headers, ip("198.51.100.20")),
        ip("198.51.100.20")
    );
}

#[test]
fn trusts_proxy_cidr() {
    let trusted = TrustedProxies::parse("172.16.0.0/12").unwrap();
    let mut headers = HeaderMap::new();

    headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.10"));

    assert_eq!(
        trusted.resolve(&headers, ip("172.20.0.5")),
        ip("203.0.113.10")
    );
}

#[test]
fn rejects_invalid_cidr() {
    assert!(TrustedProxies::parse("192.0.2.0/33").is_err());
    assert!(TrustedProxies::parse("2001:db8::/129").is_err());
}

#[test]
fn trusts_loopback_peer_ip() {
    let trusted = TrustedProxies::parse("127.0.0.1,::1").unwrap();
    let headers = HeaderMap::new();

    assert_eq!(trusted.resolve(&headers, ip("127.0.0.1")), ip("127.0.0.1"));

    assert_eq!(trusted.resolve(&headers, ip("::1")), ip("::1"));
}
