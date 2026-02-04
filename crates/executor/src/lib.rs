//! Attune Executor Service Library
//!
//! This library exposes internal modules for testing purposes.
//! The actual executor service is a binary in main.rs.

pub mod completion_listener;
pub mod enforcement_processor;
pub mod event_processor;
pub mod inquiry_handler;
pub mod policy_enforcer;
pub mod queue_manager;
pub mod workflow;

// Re-export commonly used types for convenience
pub use inquiry_handler::{InquiryHandler, InquiryRequest, INQUIRY_RESULT_KEY};
pub use policy_enforcer::{
    ExecutionPolicy, PolicyEnforcer, PolicyScope, PolicyViolation, RateLimit,
};
pub use queue_manager::{ExecutionQueueManager, QueueConfig, QueueStats};
pub use workflow::{
    parse_workflow_yaml, BackoffStrategy, ParseError, TemplateEngine, VariableContext,
    WorkflowDefinition, WorkflowValidator,
};
