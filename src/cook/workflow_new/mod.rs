//! Workflow execution module

pub mod executor;

pub use executor::{ExtendedWorkflowConfig, WorkflowExecutor, WorkflowStep};

// Re-export the old WorkflowExecutor with a different name for compatibility
pub use super::workflow::WorkflowExecutor as LegacyWorkflowExecutor;
