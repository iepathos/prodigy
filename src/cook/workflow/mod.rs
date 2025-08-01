//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

mod executor;

pub use executor::{WorkflowExecutor, WorkflowStep, ExtendedWorkflowConfig};