//! Validation module
//!
//! Contains validation utilities for API requests and parameters.

pub mod params;

pub use params::{validate_action_params, validate_trigger_params};
