use std::net::IpAddr;

use axum::http::HeaderMap;

use super::parse::parse_ip_candidate;

pub(crate) fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    header_ip(headers, "cf-connecting-ip")
        .or_else(|| header_ip(headers, "true-client-ip"))
        .or_else(|| x_forwarded_for_ip(headers))
        .or_else(|| header_ip(headers, "x-real-ip"))
        .or_else(|| forwarded_header_ip(headers))
}

fn header_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_ip_candidate)
}

fn x_forwarded_for_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').find_map(parse_ip_candidate))
}

fn forwarded_header_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("forwarded")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value.split(',').find_map(|entry| {
                entry.split(';').find_map(|part| {
                    let (key, value) = part.trim().split_once('=')?;

                    if !key.trim().eq_ignore_ascii_case("for") {
                        return None;
                    }

                    parse_ip_candidate(value)
                })
            })
        })
}
