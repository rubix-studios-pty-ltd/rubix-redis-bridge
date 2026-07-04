use std::net::IpAddr;

use anyhow::{anyhow, bail};
use axum::http::HeaderMap;
use tracing::warn;

use super::headers::forwarded_ip;
use super::parse::{prefix_mask_v4, prefix_mask_v6};

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

        if let Some(client_ip) = forwarded_ip(headers) {
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

#[cfg(test)]
#[path = "trusted_tests.rs"]
mod tests;
