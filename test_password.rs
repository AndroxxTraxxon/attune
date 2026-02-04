//! Quick password hash verification test

use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};

fn main() {
    let password = "admin";
    let hash = "$argon2id$v=19$m=19456,t=2,p=1$9Z0VWE8xbJMGPJ8kQ3qRmA$iGBqNEdvklvGLJH8TdUv6u+5c8WU8P9v7UzxQXmkFsE";

    println!("Testing password verification:");
    println!("  Password: {}", password);
    println!("  Hash: {}", hash);

    match PasswordHash::new(hash) {
        Ok(parsed_hash) => {
            println!("  ✓ Hash parsed successfully");

            let argon2 = Argon2::default();
            match argon2.verify_password(password.as_bytes(), &parsed_hash) {
                Ok(_) => {
                    println!("  ✓ Password verification SUCCESSFUL");
                    println!("\nThe password 'admin' matches the hash!");
                }
                Err(e) => {
                    println!("  ✗ Password verification FAILED: {:?}", e);
                    println!("\nThe password 'admin' does NOT match the hash!");

                    // Try to generate correct hash
                    println!("\nGenerating new hash for 'admin':");
                    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
                    let salt = SaltString::generate(&mut OsRng);
                    match argon2.hash_password(password.as_bytes(), &salt) {
                        Ok(new_hash) => {
                            println!("  New hash: {}", new_hash);
                        }
                        Err(e) => {
                            println!("  Failed to generate hash: {:?}", e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("  ✗ Failed to parse hash: {:?}", e);
        }
    }
}
