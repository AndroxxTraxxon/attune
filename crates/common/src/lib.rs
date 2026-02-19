//! Common utilities, models, and database layer for Attune services
//!
//! This crate provides shared functionality used across all Attune services including:
//! - Database models and schema
//! - Error types
//! - Configuration
//! - Utilities

pub mod config;
pub mod crypto;
pub mod db;
pub mod error;
pub mod models;
pub mod mq;
pub mod pack_environment;
pub mod pack_registry;
pub mod repositories;
pub mod runtime_detection;
pub mod schema;
pub mod template_resolver;
pub mod test_executor;
pub mod utils;
pub mod workflow;

// Re-export commonly used types
pub use error::{Error, Result};
pub use template_resolver::{resolve_templates, TemplateContext};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
