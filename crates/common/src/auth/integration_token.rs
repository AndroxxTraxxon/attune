use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

pub const INTEGRATION_TOKEN_PREFIX: &str = "attune_it_";
const SECRET_BYTES: usize = 32;
const DISPLAY_PREFIX_LEN: usize = 18;
const DISPLAY_SUFFIX_LEN: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedIntegrationToken {
    pub secret: String,
    pub hash: String,
    pub prefix: String,
    pub suffix: String,
}

pub fn generate_integration_token() -> Result<GeneratedIntegrationToken> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; SECRET_BYTES];
    rng.fill(&mut bytes)
        .map_err(|_| Error::internal("failed to generate integration token"))?;

    let secret = format!(
        "{}{}",
        INTEGRATION_TOKEN_PREFIX,
        URL_SAFE_NO_PAD.encode(bytes)
    );
    Ok(GeneratedIntegrationToken {
        hash: hash_integration_token(&secret),
        prefix: token_display_prefix(&secret),
        suffix: token_display_suffix(&secret),
        secret,
    })
}

pub fn hash_integration_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub fn token_display_prefix(token: &str) -> String {
    token.chars().take(DISPLAY_PREFIX_LEN).collect()
}

pub fn token_display_suffix(token: &str) -> String {
    let suffix = token
        .chars()
        .rev()
        .take(DISPLAY_SUFFIX_LEN)
        .collect::<Vec<_>>();
    suffix.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_has_expected_shape() {
        let token = generate_integration_token().expect("token should generate");

        assert!(token.secret.starts_with(INTEGRATION_TOKEN_PREFIX));
        assert_eq!(token.hash, hash_integration_token(&token.secret));
        assert_eq!(token.prefix, token_display_prefix(&token.secret));
        assert_eq!(token.suffix, token_display_suffix(&token.secret));
        assert_ne!(token.secret, token.hash);
    }

    #[test]
    fn hashing_is_deterministic() {
        let token = "attune_it_example";
        assert_eq!(hash_integration_token(token), hash_integration_token(token));
        assert_ne!(
            hash_integration_token(token),
            hash_integration_token("attune_it_other")
        );
    }
}
