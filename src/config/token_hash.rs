use std::fmt;

use anyhow::{anyhow, bail};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

const SHA256_DIGEST_BYTES: usize = 32;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct TokenHash {
    digest: [u8; SHA256_DIGEST_BYTES],
}

impl TokenHash {
    pub fn sha256(token: &str) -> Self {
        let digest = Sha256::digest(token.as_bytes());

        Self {
            digest: digest.into(),
        }
    }

    pub fn hmac_sha256(key: &str, token: &str) -> Self {
        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            .expect("HMAC-SHA256 accepts keys of any size");

        mac.update(token.as_bytes());

        Self {
            digest: mac.finalize().into_bytes().into(),
        }
    }

    pub fn hmac_sha256_parse(value: &str) -> anyhow::Result<Self> {
        let value = value.trim();

        if value.len() != SHA256_DIGEST_BYTES * 2 {
            bail!(
                "Token hash digest must be {} hex characters",
                SHA256_DIGEST_BYTES * 2
            );
        }

        let mut digest = [0u8; SHA256_DIGEST_BYTES];

        hex::decode_to_slice(value, &mut digest)
            .map_err(|_| anyhow!("Token hash digest contains non-hex characters"))?;

        Ok(Self { digest })
    }
}

impl fmt::Debug for TokenHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenHash")
            .field("algorithm", &"hmac-sha256")
            .field("digest", &"[redacted]")
            .finish()
    }
}

#[cfg(test)]
#[path = "token_hash_tests.rs"]
mod tests;
