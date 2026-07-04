use std::net::{IpAddr, SocketAddr};

use anyhow::{anyhow, bail};
use axum::http::HeaderMap;
use tracing::warn;

#[derive(Clone, Debug, Default)]
pub(crate) struct TrustedProxies {
    entries: Vec<TrustedProxy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrustedProxy {
    network: IpAddr,
    prefix: u8,
}

impl TrustedProxies {
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        let mut entries = Vec::new();

        for raw_entry in value.split(',') {
            let entry = raw_entry.trim();

            if entry.is_empty() {
                continue;
            }

            entries.push(TrustedProxy::parse(entry)?);
        }

        Ok(Self { entries })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn resolve(&self, headers: &HeaderMap, peer_ip: IpAddr) -> IpAddr {
        if !self.is_trusted(peer_ip) {
            return peer_ip;
        }

        if let Some(client_ip) = forwarded_client_ip(headers) {
            return client_ip;
        }

        warn!(
            peer_ip = %peer_ip,
            "Trusted proxy request did not include a valid client IP header"
        );

        peer_ip
    }

    fn is_trusted(&self, ip: IpAddr) -> bool {
        self.entries.iter().any(|entry| entry.contains(ip))
    }
}

impl TrustedProxy {
    fn parse(value: &str) -> anyhow::Result<Self> {
        let Some((ip, prefix)) = value.split_once('/') else {
            let ip = value
                .parse::<IpAddr>()
                .map_err(|_| anyhow!("Invalid trusted proxy IP: {value}"))?;

            let prefix = match ip {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            };

            return Ok(Self {
                network: ip,
                prefix,
            });
        };

        let network = ip
            .parse::<IpAddr>()
            .map_err(|_| anyhow!("Invalid trusted proxy CIDR address: {value}"))?;

        let prefix = prefix
            .parse::<u8>()
            .map_err(|_| anyhow!("Invalid trusted proxy CIDR prefix: {value}"))?;

        match network {
            IpAddr::V4(_) if prefix > 32 => bail!("Invalid IPv4 trusted proxy CIDR: {value}"),
            IpAddr::V6(_) if prefix > 128 => bail!("Invalid IPv6 trusted proxy CIDR: {value}"),
            _ => {}
        }

        Ok(Self { network, prefix })
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                let network = u32::from(network);
                let ip = u32::from(ip);
                let mask = prefix_mask_v4(self.prefix);

                network & mask == ip & mask
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                let network = u128::from(network);
                let ip = u128::from(ip);
                let mask = prefix_mask_v6(self.prefix);

                network & mask == ip & mask
            }
            _ => false,
        }
    }
}

fn forwarded_client_ip(headers: &HeaderMap) -> Option<IpAddr> {
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

fn parse_ip_candidate(value: &str) -> Option<IpAddr> {
    let value = value.trim().trim_matches('"');

    if value.is_empty() || value.eq_ignore_ascii_case("unknown") {
        return None;
    }

    if let Ok(ip) = value.parse::<IpAddr>() {
        return Some(ip);
    }

    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Some(addr.ip());
    }

    if let Some(rest) = value.strip_prefix('[') {
        let (ip, _) = rest.split_once(']')?;
        return ip.parse::<IpAddr>().ok();
    }

    if value.matches(':').count() == 1 {
        let (ip, _) = value.rsplit_once(':')?;
        return ip.parse::<IpAddr>().ok();
    }

    None
}

fn prefix_mask_v4(prefix: u8) -> u32 {
    if prefix == 0 {
        return 0;
    }

    u32::MAX << (32 - prefix)
}

fn prefix_mask_v6(prefix: u8) -> u128 {
    if prefix == 0 {
        return 0;
    }

    u128::MAX << (128 - prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn trusts_exact_proxy_ip() {
        let trusted = TrustedProxies::parse("127.0.0.1").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));

        assert_eq!(
            trusted.resolve(&headers, ip("127.0.0.1")),
            ip("203.0.113.10")
        );
    }

    #[test]
    fn ignores_forwarded_headers_from_untrusted_peer() {
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
    fn prefers_cloudflare_header() {
        let trusted = TrustedProxies::parse("10.0.0.0/8").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.10"));
        headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.20"));

        assert_eq!(
            trusted.resolve(&headers, ip("10.0.0.3")),
            ip("203.0.113.10")
        );
    }

    #[test]
    fn parses_forwarded_header() {
        let trusted = TrustedProxies::parse("10.0.0.0/8").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "forwarded",
            HeaderValue::from_static("for=203.0.113.10;proto=https;host=example.com"),
        );

        assert_eq!(
            trusted.resolve(&headers, ip("10.0.0.3")),
            ip("203.0.113.10")
        );
    }

    #[test]
    fn parses_bracketed_ipv6_forwarded_header() {
        let trusted = TrustedProxies::parse("2001:db8::/32").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "forwarded",
            HeaderValue::from_static("for=\"[2001:db8:abcd::1]:443\";proto=https"),
        );

        assert_eq!(
            trusted.resolve(&headers, ip("2001:db8::10")),
            ip("2001:db8:abcd::1")
        );
    }

    #[test]
    fn rejects_invalid_cidr() {
        assert!(TrustedProxies::parse("192.0.2.0/33").is_err());
        assert!(TrustedProxies::parse("2001:db8::/129").is_err());
    }
}
