//! HMAC-only CryptoProvider for jsonwebtoken v10.
//!
//! The `jsonwebtoken` crate v10 requires a `CryptoProvider` to be installed
//! before any signing/verification operations. The built-in `rust_crypto`
//! feature pulls in the `rsa` crate, which has an unpatched advisory
//! (RUSTSEC-2023-0071 — Marvin Attack timing sidechannel).
//!
//! Since Attune only uses HMAC-SHA2 (HS256/HS384/HS512) for JWT signing,
//! this module provides a minimal CryptoProvider that supports only those
//! algorithms, avoiding the `rsa` dependency entirely.
//!
//! Call [`install()`] once at process startup (before any JWT operations).

use hmac::{digest::KeyInit, Hmac, Mac};
use jsonwebtoken::crypto::{CryptoProvider, JwkUtils, JwtSigner, JwtVerifier};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use sha2::{Sha256, Sha384, Sha512};
use signature::{Signer, Verifier};
use std::sync::Once;

type HmacSha256 = Hmac<Sha256>;
type HmacSha384 = Hmac<Sha384>;
type HmacSha512 = Hmac<Sha512>;

// ---------------------------------------------------------------------------
// Signers
// ---------------------------------------------------------------------------

macro_rules! define_hmac_signer {
    ($name:ident, $alg:expr, $hmac_type:ty) => {
        struct $name($hmac_type);

        impl $name {
            fn new(key: &EncodingKey) -> jsonwebtoken::errors::Result<Self> {
                let inner = <$hmac_type>::new_from_slice(key.try_get_hmac_secret()?)
                    .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidKeyFormat)?;
                Ok(Self(inner))
            }
        }

        impl Signer<Vec<u8>> for $name {
            fn try_sign(&self, msg: &[u8]) -> std::result::Result<Vec<u8>, signature::Error> {
                let mut mac = self.0.clone();
                mac.update(msg);
                Ok(mac.finalize().into_bytes().to_vec())
            }
        }

        impl JwtSigner for $name {
            fn algorithm(&self) -> Algorithm {
                $alg
            }
        }
    };
}

define_hmac_signer!(Hs256Signer, Algorithm::HS256, HmacSha256);
define_hmac_signer!(Hs384Signer, Algorithm::HS384, HmacSha384);
define_hmac_signer!(Hs512Signer, Algorithm::HS512, HmacSha512);

// ---------------------------------------------------------------------------
// Verifiers
// ---------------------------------------------------------------------------

macro_rules! define_hmac_verifier {
    ($name:ident, $alg:expr, $hmac_type:ty) => {
        struct $name($hmac_type);

        impl $name {
            fn new(key: &DecodingKey) -> jsonwebtoken::errors::Result<Self> {
                let inner = <$hmac_type>::new_from_slice(key.try_get_hmac_secret()?)
                    .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidKeyFormat)?;
                Ok(Self(inner))
            }
        }

        impl Verifier<Vec<u8>> for $name {
            fn verify(
                &self,
                msg: &[u8],
                sig: &Vec<u8>,
            ) -> std::result::Result<(), signature::Error> {
                let mut mac = self.0.clone();
                mac.update(msg);
                mac.verify_slice(sig).map_err(signature::Error::from_source)
            }
        }

        impl JwtVerifier for $name {
            fn algorithm(&self) -> Algorithm {
                $alg
            }
        }
    };
}

define_hmac_verifier!(Hs256Verifier, Algorithm::HS256, HmacSha256);
define_hmac_verifier!(Hs384Verifier, Algorithm::HS384, HmacSha384);
define_hmac_verifier!(Hs512Verifier, Algorithm::HS512, HmacSha512);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

fn hmac_signer_factory(
    algorithm: &Algorithm,
    key: &EncodingKey,
) -> jsonwebtoken::errors::Result<Box<dyn JwtSigner>> {
    match algorithm {
        Algorithm::HS256 => Ok(Box::new(Hs256Signer::new(key)?)),
        Algorithm::HS384 => Ok(Box::new(Hs384Signer::new(key)?)),
        Algorithm::HS512 => Ok(Box::new(Hs512Signer::new(key)?)),
        _other => Err(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm.into()),
    }
}

fn hmac_verifier_factory(
    algorithm: &Algorithm,
    key: &DecodingKey,
) -> jsonwebtoken::errors::Result<Box<dyn JwtVerifier>> {
    match algorithm {
        Algorithm::HS256 => Ok(Box::new(Hs256Verifier::new(key)?)),
        Algorithm::HS384 => Ok(Box::new(Hs384Verifier::new(key)?)),
        Algorithm::HS512 => Ok(Box::new(Hs512Verifier::new(key)?)),
        _other => Err(jsonwebtoken::errors::ErrorKind::InvalidAlgorithm.into()),
    }
}

/// HMAC-only [`CryptoProvider`]. Supports HS256, HS384, HS512 only.
/// JWK utility functions (RSA/EC key extraction) are stubbed out since
/// Attune never uses asymmetric JWKs.
static HMAC_PROVIDER: CryptoProvider = CryptoProvider {
    signer_factory: hmac_signer_factory,
    verifier_factory: hmac_verifier_factory,
    jwk_utils: JwkUtils::new_unimplemented(),
};

static INIT: Once = Once::new();

/// Install the HMAC-only crypto provider for jsonwebtoken.
///
/// Safe to call multiple times — only the first call takes effect.
/// Must be called before any JWT encode/decode operations.
pub fn install() {
    INIT.call_once(|| {
        // install_default returns Err if already installed (e.g., by a feature-based
        // provider). That's fine — we only care that *some* provider is present.
        let _ = HMAC_PROVIDER.install_default();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_idempotent() {
        install();
        install(); // second call should not panic
    }

    #[test]
    fn test_hmac_sign_and_verify() {
        install();

        let secret = b"test-secret-key";
        let encoding_key = EncodingKey::from_secret(secret);
        let decoding_key = DecodingKey::from_secret(secret);

        let message = b"hello world";

        let signer =
            hmac_signer_factory(&Algorithm::HS256, &encoding_key).expect("should create signer");
        let sig = signer.try_sign(message).expect("should sign");

        let verifier = hmac_verifier_factory(&Algorithm::HS256, &decoding_key)
            .expect("should create verifier");
        verifier
            .verify(message, &sig)
            .expect("signature should verify");
    }

    #[test]
    fn test_unsupported_algorithm_rejected() {
        install();

        let key = EncodingKey::from_secret(b"key");
        let result = hmac_signer_factory(&Algorithm::RS256, &key);
        assert!(result.is_err());
    }
}
