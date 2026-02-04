//! Authentication and authorization module

pub mod jwt;
pub mod middleware;
pub mod password;

pub use jwt::{generate_token, validate_token, Claims};
pub use middleware::{AuthMiddleware, RequireAuth};
pub use password::{hash_password, verify_password};
