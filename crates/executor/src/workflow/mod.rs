//! Workflow orchestration module
//!
//! This module provides workflow execution, orchestration, parsing, validation,
//! and template rendering capabilities for the Attune workflow orchestration system.
//!
//! # Modules
//!
//! - `parser`: Parse YAML workflow definitions into structured types
//! - `graph`: Build executable task graphs from workflow definitions
//! - `context`: Manage workflow execution context and variables
//! - `task_executor`: Execute individual workflow tasks
//! - `coordinator`: Orchestrate workflow execution with state management
//! - `template`: Template engine for variable interpolation (Jinja2-like syntax)
//!
//! # Example
//!
//! ```no_run
//! use attune_executor::workflow::{parse_workflow_yaml, WorkflowCoordinator};
//!
//! // Parse a workflow YAML file
//! let yaml = r#"
//! ref: my_pack.my_workflow
//! label: My Workflow
//! version: 1.0.0
//! tasks:
//!   - name: hello
//!     action: core.echo
//!     input:
//!       message: "{{ parameters.name }}"
//! "#;
//!
//! let workflow = parse_workflow_yaml(yaml).expect("Failed to parse workflow");
//! ```

// Phase 2: Workflow Execution Engine
pub mod context;
pub mod coordinator;
pub mod graph;
pub mod task_executor;
pub mod template;

// Re-export workflow utilities from common crate
pub use attune_common::workflow::{
    parse_workflow_file, parse_workflow_yaml, workflow_to_json, BackoffStrategy, DecisionBranch,
    LoadedWorkflow, LoaderConfig, ParseError, ParseResult, PublishDirective, RegistrationOptions,
    RegistrationResult, RetryConfig, Task, TaskType, ValidationError, ValidationResult,
    WorkflowDefinition, WorkflowFile, WorkflowLoader, WorkflowRegistrar, WorkflowValidator,
};

// Re-export Phase 2 components
pub use context::{ContextError, ContextResult, WorkflowContext};
pub use coordinator::{
    WorkflowCoordinator, WorkflowExecutionHandle, WorkflowExecutionResult, WorkflowExecutionState,
    WorkflowExecutionStatus,
};
pub use graph::{GraphError, GraphResult, TaskGraph, TaskNode, TaskTransitions};
pub use task_executor::{
    TaskExecutionError, TaskExecutionResult, TaskExecutionStatus, TaskExecutor,
};
pub use template::{TemplateEngine, TemplateError, TemplateResult, VariableContext, VariableScope};
