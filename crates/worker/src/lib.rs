//! Attune Worker Service Library
//!
//! This library provides the core functionality for the Attune Worker Service,
//! which executes actions in various runtime environments.

pub mod artifacts;
pub mod executor;
pub mod heartbeat;
pub mod registration;
pub mod runtime;
pub mod secrets;
pub mod service;
pub mod test_executor;

// Re-export commonly used types
pub use executor::ActionExecutor;
pub use heartbeat::HeartbeatManager;
pub use registration::WorkerRegistration;
pub use runtime::{
    ExecutionContext, ExecutionResult, LocalRuntime, NativeRuntime, PythonRuntime, Runtime,
    RuntimeError, RuntimeResult, ShellRuntime,
};
pub use secrets::SecretManager;
pub use service::WorkerService;
pub use test_executor::{TestConfig, TestExecutor};
