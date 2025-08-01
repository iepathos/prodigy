//! Workflow execution module

pub mod executor;

pub use executor::{WorkflowExecutor, ExtendedWorkflowConfig, WorkflowStep};

// Re-export the old WorkflowExecutor with a different name for compatibility
pub use super::workflow::WorkflowExecutor as LegacyWorkflowExecutor;