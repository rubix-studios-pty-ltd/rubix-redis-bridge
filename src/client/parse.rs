use std::net::{IpAddr, SocketAddr};

pub(crate) fn parse_ip_candidate(value: &str) -> Option<IpAddr> {
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

pub(crate) fn prefix_mask_v4(prefix: u8) -> u32 {
    if prefix == 0 {
        return 0;
    }

    u32::MAX << (32 - prefix)
}

pub(crate) fn prefix_mask_v6(prefix: u8) -> u128 {
    if prefix == 0 {
        return 0;
    }

    u128::MAX << (128 - prefix)
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
