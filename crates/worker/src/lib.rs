//! Attune Worker Service Library
//!
//! This library provides the core functionality for the Attune Worker Service,
//! which executes actions in various runtime environments.

pub mod artifacts;
pub mod env_setup;
pub mod executor;
pub mod heartbeat;
pub mod registration;
pub mod runtime;
pub mod secrets;
pub mod service;
pub mod version_verify;

// Re-export commonly used types
pub use executor::ActionExecutor;
pub use heartbeat::HeartbeatManager;
pub use registration::WorkerRegistration;
pub use runtime::{
    ExecutionContext, ExecutionResult, LocalRuntime, NativeRuntime, ProcessRuntime, Runtime,
    RuntimeError, RuntimeResult,
};
pub use secrets::SecretManager;
pub use service::WorkerService;
// Re-export test executor from common (shared business logic)
pub use attune_common::test_executor::{TestConfig, TestExecutor};
