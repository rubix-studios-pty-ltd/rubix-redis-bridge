mod headers;
mod parse;
mod trusted;

pub(crate) use trusted::TrustedProxies;

#[cfg(test)]
#[path = "headers_tests.rs"]
mod headers_tests;

#[cfg(test)]
#[path = "parse_tests.rs"]
mod parse_tests;

#[cfg(test)]
#[path = "trusted_tests.rs"]
mod trusted_tests;
