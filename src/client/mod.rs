mod headers;
mod parse;
mod trusted;

pub(crate) use trusted::TrustedProxies;

#[cfg(test)]
pub(crate) use {
    headers::forwarded_ip,
    parse::{parse_ip_candidate, prefix_mask_v4, prefix_mask_v6},
};