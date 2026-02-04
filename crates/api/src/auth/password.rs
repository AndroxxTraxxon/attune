//! Password hashing and verification using Argon2

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("Failed to hash password: {0}")]
    HashError(String),
    #[error("Failed to verify password: {0}")]
    VerifyError(String),
    #[error("Invalid password hash format")]
    InvalidHash,
}

/// Hash a password using Argon2id
///
/// # Arguments
/// * `password` - The plaintext password to hash
///
/// # Returns
/// * `Result<String, PasswordError>` - The hashed password string (PHC format)
///
/// # Example
/// ```
/// use attune_api::auth::password::hash_password;
///
/// let hash = hash_password("my_secure_password").expect("Failed to hash password");
/// assert!(!hash.is_empty());
/// ```
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| PasswordError::HashError(e.to_string()))
}

/// Verify a password against a hash using Argon2id
///
/// # Arguments
/// * `password` - The plaintext password to verify
/// * `hash` - The password hash string (PHC format)
///
/// # Returns
/// * `Result<bool, PasswordError>` - True if password matches, false otherwise
///
/// # Example
/// ```
/// use attune_api::auth::password::{hash_password, verify_password};
///
/// let hash = hash_password("my_secure_password").expect("Failed to hash");
/// let is_valid = verify_password("my_secure_password", &hash).expect("Failed to verify");
/// assert!(is_valid);
/// ```
pub fn verify_password(password: &str, hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHash)?;

    let argon2 = Argon2::default();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(_) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PasswordError::VerifyError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "my_secure_password_123";
        let hash = hash_password(password).expect("Failed to hash password");

        // Verify correct password
        assert!(verify_password(password, &hash).expect("Failed to verify"));

        // Verify incorrect password
        assert!(!verify_password("wrong_password", &hash).expect("Failed to verify"));
    }

    #[test]
    fn test_hash_produces_different_salts() {
        let password = "same_password";
        let hash1 = hash_password(password).expect("Failed to hash");
        let hash2 = hash_password(password).expect("Failed to hash");

        // Hashes should be different due to different salts
        assert_ne!(hash1, hash2);

        // But both should verify correctly
        assert!(verify_password(password, &hash1).expect("Failed to verify"));
        assert!(verify_password(password, &hash2).expect("Failed to verify"));
    }

    #[test]
    fn test_invalid_hash_format() {
        let result = verify_password("password", "not_a_valid_hash");
        assert!(matches!(result, Err(PasswordError::InvalidHash)));
    }
}
