//! Cryptographic utilities for encrypting and decrypting sensitive data
//!
//! This module provides functions for encrypting and decrypting secret values
//! using AES-256-GCM encryption with randomly generated nonces.

use crate::{Error, Result};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Digest, Sha256};

/// Size of the nonce in bytes (96 bits for AES-GCM)
const NONCE_SIZE: usize = 12;

/// Encrypt a plaintext value using AES-256-GCM
///
/// The encryption key is derived from the provided key string using SHA-256.
/// A random nonce is generated for each encryption operation.
/// The returned ciphertext is base64-encoded and contains: nonce || encrypted_data || tag
///
/// # Arguments
/// * `plaintext` - The plaintext value to encrypt
/// * `encryption_key` - The encryption key (will be hashed with SHA-256)
///
/// # Returns
/// Base64-encoded encrypted value
pub fn encrypt(plaintext: &str, encryption_key: &str) -> Result<String> {
    if encryption_key.len() < 32 {
        return Err(Error::encryption(
            "Encryption key must be at least 32 characters",
        ));
    }

    // Derive a 256-bit key from the encryption key using SHA-256
    let key_bytes = derive_key(encryption_key);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    // Generate a random nonce
    let nonce_bytes = generate_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt the plaintext
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| Error::encryption(format!("Encryption failed: {}", e)))?;

    // Combine nonce + ciphertext and encode as base64
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&result))
}

/// Decrypt a ciphertext value using AES-256-GCM
///
/// The ciphertext should be base64-encoded and contain: nonce || encrypted_data || tag
///
/// # Arguments
/// * `ciphertext` - Base64-encoded encrypted value
/// * `encryption_key` - The encryption key (will be hashed with SHA-256)
///
/// # Returns
/// Decrypted plaintext value
pub fn decrypt(ciphertext: &str, encryption_key: &str) -> Result<String> {
    if encryption_key.len() < 32 {
        return Err(Error::encryption(
            "Encryption key must be at least 32 characters",
        ));
    }

    // Decode base64
    let encrypted_data = BASE64
        .decode(ciphertext)
        .map_err(|e| Error::encryption(format!("Invalid base64: {}", e)))?;

    if encrypted_data.len() < NONCE_SIZE {
        return Err(Error::encryption("Invalid ciphertext: too short"));
    }

    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext_bytes) = encrypted_data.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Derive the key
    let key_bytes = derive_key(encryption_key);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    // Decrypt
    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext_bytes)
        .map_err(|e| Error::encryption(format!("Decryption failed: {}", e)))?;

    String::from_utf8(plaintext_bytes)
        .map_err(|e| Error::encryption(format!("Invalid UTF-8 in decrypted data: {}", e)))
}

/// Derive a 256-bit key from the encryption key string using SHA-256
fn derive_key(encryption_key: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(encryption_key.as_bytes());
    let result = hasher.finalize();
    result.into()
}

/// Generate a random 96-bit nonce for AES-GCM
fn generate_nonce() -> [u8; NONCE_SIZE] {
    use aes_gcm::aead::rand_core::RngCore;
    let mut nonce = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Hash an encryption key to store as a reference
///
/// This is used to verify that the correct encryption key is being used
/// without storing the key itself.
pub fn hash_encryption_key(encryption_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(encryption_key.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "this_is_a_test_key_that_is_32_chars_long!!!!";

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = "my_secret_password";
        let encrypted = encrypt(plaintext, TEST_KEY).expect("Encryption should succeed");
        let decrypted = decrypt(&encrypted, TEST_KEY).expect("Decryption should succeed");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_produces_different_output() {
        let plaintext = "my_secret_password";
        let encrypted1 = encrypt(plaintext, TEST_KEY).expect("Encryption should succeed");
        let encrypted2 = encrypt(plaintext, TEST_KEY).expect("Encryption should succeed");

        // Should produce different ciphertext due to random nonce
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same value
        let decrypted1 = decrypt(&encrypted1, TEST_KEY).expect("Decryption should succeed");
        let decrypted2 = decrypt(&encrypted2, TEST_KEY).expect("Decryption should succeed");
        assert_eq!(decrypted1, decrypted2);
        assert_eq!(plaintext, decrypted1);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let plaintext = "my_secret_password";
        let encrypted = encrypt(plaintext, TEST_KEY).expect("Encryption should succeed");

        let wrong_key = "wrong_key_that_is_also_32_chars_long!!!";
        let result = decrypt(&encrypted, wrong_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_with_short_key_fails() {
        let plaintext = "my_secret_password";
        let short_key = "short";
        let result = encrypt(plaintext, short_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let result = decrypt("not valid base64!!!", TEST_KEY);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short_fails() {
        let result = decrypt(&BASE64.encode(b"short"), TEST_KEY);
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_encryption_key() {
        let hash1 = hash_encryption_key(TEST_KEY);
        let hash2 = hash_encryption_key(TEST_KEY);

        // Same key should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be 64 hex characters (SHA-256)
        assert_eq!(hash1.len(), 64);

        // Different key should produce different hash
        let different_key = "different_key_that_is_32_chars_long!!";
        let hash3 = hash_encryption_key(different_key);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_encrypt_empty_string() {
        let plaintext = "";
        let encrypted = encrypt(plaintext, TEST_KEY).expect("Encryption should succeed");
        let decrypted = decrypt(&encrypted, TEST_KEY).expect("Decryption should succeed");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_unicode() {
        let plaintext = "🔐 Secret émojis and spëcial çhars! 日本語";
        let encrypted = encrypt(plaintext, TEST_KEY).expect("Encryption should succeed");
        let decrypted = decrypt(&encrypted, TEST_KEY).expect("Decryption should succeed");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_derive_key_consistency() {
        let key1 = derive_key(TEST_KEY);
        let key2 = derive_key(TEST_KEY);
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32); // 256 bits
    }
}
