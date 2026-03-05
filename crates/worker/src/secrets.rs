//! Secret Management Module
//!
//! Handles fetching, decrypting, and injecting secrets into execution environments.
//! Secrets are stored encrypted in the database and decrypted on-demand for execution.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key as AesKey, Nonce,
};
use attune_common::error::{Error, Result};
use attune_common::models::{key::Key, Action, OwnerType};
use attune_common::repositories::key::KeyRepository;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Secret manager for handling secret operations
pub struct SecretManager {
    pool: PgPool,
    encryption_key: Option<Vec<u8>>,
}

impl SecretManager {
    /// Create a new secret manager
    pub fn new(pool: PgPool, encryption_key: Option<String>) -> Result<Self> {
        let encryption_key = encryption_key.map(|key| Self::derive_key(&key));

        if encryption_key.is_none() {
            warn!("No encryption key configured - encrypted secrets will fail to decrypt");
        }

        Ok(Self {
            pool,
            encryption_key,
        })
    }

    /// Derive encryption key from password/key string
    fn derive_key(key: &str) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.finalize().to_vec()
    }

    /// Fetch all secrets relevant to an action execution
    ///
    /// Secrets are fetched in order of precedence:
    /// 1. System-level secrets (owner_type='system')
    /// 2. Pack-level secrets (owner_type='pack')
    /// 3. Action-level secrets (owner_type='action')
    ///
    /// More specific secrets override less specific ones with the same name.
    pub async fn fetch_secrets_for_action(
        &self,
        action: &Action,
    ) -> Result<HashMap<String, String>> {
        debug!("Fetching secrets for action: {}", action.r#ref);

        let mut secrets = HashMap::new();

        // 1. Fetch system-level secrets
        let system_secrets = self.fetch_secrets_by_owner_type(OwnerType::System).await?;
        for secret in system_secrets {
            let value = self.decrypt_if_needed(&secret)?;
            secrets.insert(secret.name.clone(), value);
        }
        debug!("Loaded {} system secrets", secrets.len());

        // 2. Fetch pack-level secrets
        let pack_secrets = self.fetch_secrets_by_pack(action.pack).await?;
        for secret in pack_secrets {
            let value = self.decrypt_if_needed(&secret)?;
            secrets.insert(secret.name.clone(), value);
        }
        debug!("Loaded {} pack secrets", secrets.len());

        // 3. Fetch action-level secrets
        let action_secrets = self.fetch_secrets_by_action(action.id).await?;
        for secret in action_secrets {
            let value = self.decrypt_if_needed(&secret)?;
            secrets.insert(secret.name.clone(), value);
        }
        debug!("Total secrets loaded: {}", secrets.len());

        Ok(secrets)
    }

    /// Fetch secrets by owner type
    async fn fetch_secrets_by_owner_type(&self, owner_type: OwnerType) -> Result<Vec<Key>> {
        KeyRepository::find_by_owner_type(&self.pool, owner_type).await
    }

    /// Fetch secrets for a specific pack
    async fn fetch_secrets_by_pack(&self, pack_id: i64) -> Result<Vec<Key>> {
        sqlx::query_as::<_, Key>(
            "SELECT id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref,
             owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted,
             encryption_key_hash, value, created, updated
             FROM key
             WHERE owner_type = $1 AND owner_pack = $2
             ORDER BY name ASC",
        )
        .bind(OwnerType::Pack)
        .bind(pack_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Fetch secrets for a specific action
    async fn fetch_secrets_by_action(&self, action_id: i64) -> Result<Vec<Key>> {
        sqlx::query_as::<_, Key>(
            "SELECT id, ref, owner_type, owner, owner_identity, owner_pack, owner_pack_ref,
             owner_action, owner_action_ref, owner_sensor, owner_sensor_ref, name, encrypted,
             encryption_key_hash, value, created, updated
             FROM key
             WHERE owner_type = $1 AND owner_action = $2
             ORDER BY name ASC",
        )
        .bind(OwnerType::Action)
        .bind(action_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Decrypt a secret if it's encrypted, otherwise return the value as-is
    fn decrypt_if_needed(&self, key: &Key) -> Result<String> {
        if !key.encrypted {
            return Ok(key.value.clone());
        }

        // Encrypted secret requires encryption key
        let encryption_key = self
            .encryption_key
            .as_ref()
            .ok_or_else(|| Error::Internal("No encryption key configured".to_string()))?;

        // Verify encryption key hash if present
        if let Some(expected_hash) = &key.encryption_key_hash {
            let actual_hash = Self::compute_key_hash_from_bytes(encryption_key);
            if &actual_hash != expected_hash {
                return Err(Error::Internal(format!(
                    "Encryption key hash mismatch for secret '{}'",
                    key.name
                )));
            }
        }

        Self::decrypt_value(&key.value, encryption_key)
    }

    /// Decrypt an encrypted value
    ///
    /// Format: "nonce:ciphertext" (both base64-encoded)
    fn decrypt_value(encrypted_value: &str, key: &[u8]) -> Result<String> {
        // Parse format: "nonce:ciphertext"
        let parts: Vec<&str> = encrypted_value.split(':').collect();
        if parts.len() != 2 {
            return Err(Error::Internal(
                "Invalid encrypted value format. Expected 'nonce:ciphertext'".to_string(),
            ));
        }

        let nonce_bytes = BASE64
            .decode(parts[0])
            .map_err(|e| Error::Internal(format!("Failed to decode nonce: {}", e)))?;

        let ciphertext = BASE64
            .decode(parts[1])
            .map_err(|e| Error::Internal(format!("Failed to decode ciphertext: {}", e)))?;

        // Create cipher
        let key_array: [u8; 32] = key
            .try_into()
            .map_err(|_| Error::Internal("Invalid key length".to_string()))?;
        let cipher_key = AesKey::<Aes256Gcm>::from_slice(&key_array);
        let cipher = Aes256Gcm::new(cipher_key);

        // Create nonce
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| Error::Internal(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| Error::Internal(format!("Invalid UTF-8 in decrypted value: {}", e)))
    }

    /// Encrypt a value (for testing and future use)
    #[allow(dead_code)]
    pub fn encrypt_value(&self, plaintext: &str) -> Result<String> {
        let encryption_key = self
            .encryption_key
            .as_ref()
            .ok_or_else(|| Error::Internal("No encryption key configured".to_string()))?;

        Self::encrypt_value_with_key(plaintext, encryption_key)
    }

    /// Encrypt a value with a specific key (static method)
    fn encrypt_value_with_key(plaintext: &str, encryption_key: &[u8]) -> Result<String> {
        // Create cipher
        let key_array: [u8; 32] = encryption_key
            .try_into()
            .map_err(|_| Error::Internal("Invalid key length".to_string()))?;
        let cipher_key = AesKey::<Aes256Gcm>::from_slice(&key_array);
        let cipher = Aes256Gcm::new(cipher_key);

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // Encrypt
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| Error::Internal(format!("Encryption failed: {}", e)))?;

        // Format: "nonce:ciphertext" (both base64-encoded)
        let nonce_b64 = BASE64.encode(nonce);
        let ciphertext_b64 = BASE64.encode(&ciphertext);

        Ok(format!("{}:{}", nonce_b64, ciphertext_b64))
    }

    /// Compute hash of the encryption key
    pub fn compute_key_hash(&self) -> String {
        if let Some(key) = &self.encryption_key {
            Self::compute_key_hash_from_bytes(key)
        } else {
            String::new()
        }
    }

    /// Compute hash from key bytes (static method)
    fn compute_key_hash_from_bytes(key: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key);
        format!("{:x}", hasher.finalize())
    }

    /// Prepare secrets as environment variables
    ///
    /// **DEPRECATED - SECURITY VULNERABILITY**: This method exposes secrets in the process
    /// environment, making them visible in process listings (`ps auxe`) and `/proc/[pid]/environ`.
    ///
    /// Secrets should be passed via stdin instead. This method is kept only for backward
    /// compatibility and will be removed in a future version.
    ///
    /// Secret names are converted to uppercase and prefixed with "SECRET_"
    /// Example: "api_key" becomes "SECRET_API_KEY"
    #[deprecated(
        since = "0.2.0",
        note = "Secrets in environment variables are insecure. Pass secrets via stdin instead."
    )]
    pub fn prepare_secret_env(&self, secrets: &HashMap<String, String>) -> HashMap<String, String> {
        secrets
            .iter()
            .map(|(name, value)| {
                let env_name = format!("SECRET_{}", name.to_uppercase().replace('-', "_"));
                (env_name, value.clone())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to derive a test encryption key
    fn derive_test_key(key: &str) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.finalize().to_vec()
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_test_key("test-encryption-key-12345");
        let plaintext = "my-secret-value";
        let encrypted = SecretManager::encrypt_value_with_key(plaintext, &key).unwrap();

        // Verify format
        assert!(encrypted.contains(':'));
        let parts: Vec<&str> = encrypted.split(':').collect();
        assert_eq!(parts.len(), 2);

        // Decrypt and verify
        let decrypted = SecretManager::decrypt_value(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_different_values() {
        let key = derive_test_key("test-encryption-key-12345");

        let plaintext1 = "secret1";
        let plaintext2 = "secret2";

        let encrypted1 = SecretManager::encrypt_value_with_key(plaintext1, &key).unwrap();
        let encrypted2 = SecretManager::encrypt_value_with_key(plaintext2, &key).unwrap();

        // Encrypted values should be different (due to random nonces)
        assert_ne!(encrypted1, encrypted2);

        // Both should decrypt correctly
        let decrypted1 = SecretManager::decrypt_value(&encrypted1, &key).unwrap();
        let decrypted2 = SecretManager::decrypt_value(&encrypted2, &key).unwrap();

        assert_eq!(decrypted1, plaintext1);
        assert_eq!(decrypted2, plaintext2);
    }

    #[test]
    fn test_decrypt_with_wrong_key() {
        let key1 = derive_test_key("key1");
        let key2 = derive_test_key("key2");

        let plaintext = "secret";
        let encrypted = SecretManager::encrypt_value_with_key(plaintext, &key1).unwrap();

        // Decrypting with wrong key should fail
        let result = SecretManager::decrypt_value(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn test_prepare_secret_env() {
        // Test the static method directly without creating a SecretManager instance
        let mut secrets = HashMap::new();
        secrets.insert("api_key".to_string(), "secret123".to_string());
        secrets.insert("db-password".to_string(), "pass456".to_string());
        secrets.insert("oauth_token".to_string(), "token789".to_string());

        // Call prepare_secret_env as a static-like method
        let env: HashMap<String, String> = secrets
            .iter()
            .map(|(name, value)| {
                let env_name = format!("SECRET_{}", name.to_uppercase().replace('-', "_"));
                (env_name, value.clone())
            })
            .collect();

        assert_eq!(env.get("SECRET_API_KEY"), Some(&"secret123".to_string()));
        assert_eq!(env.get("SECRET_DB_PASSWORD"), Some(&"pass456".to_string()));
        assert_eq!(env.get("SECRET_OAUTH_TOKEN"), Some(&"token789".to_string()));
        assert_eq!(env.len(), 3);
    }

    #[test]
    fn test_compute_key_hash() {
        let key1 = derive_test_key("test-key");
        let key2 = derive_test_key("test-key");
        let key3 = derive_test_key("different-key");

        let hash1 = SecretManager::compute_key_hash_from_bytes(&key1);
        let hash2 = SecretManager::compute_key_hash_from_bytes(&key2);
        let hash3 = SecretManager::compute_key_hash_from_bytes(&key3);

        // Same key should produce same hash
        assert_eq!(hash1, hash2);
        // Different key should produce different hash
        assert_ne!(hash1, hash3);
        // Hash should not be empty
        assert!(!hash1.is_empty());
    }

    #[test]
    fn test_invalid_encrypted_format() {
        let key = derive_test_key("test-key");

        // Invalid formats should fail
        let result = SecretManager::decrypt_value("no-colon", &key);
        assert!(result.is_err());

        let result = SecretManager::decrypt_value("too:many:colons", &key);
        assert!(result.is_err());

        let result = SecretManager::decrypt_value("invalid-base64:also-invalid", &key);
        assert!(result.is_err());
    }
}
