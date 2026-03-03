//! Authentication primitives shared across Attune services.
//!
//! This module provides JWT token types, generation, and validation functions
//! that are used by the API (for all token types), the worker (for execution-scoped
//! tokens), and the sensor service (for sensor tokens).

pub mod jwt;

pub use jwt::{
    extract_token_from_header, generate_access_token, generate_execution_token,
    generate_refresh_token, generate_sensor_token, generate_token, validate_token, Claims,
    JwtConfig, JwtError, TokenType,
};
