//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

mod executor;
mod traits;

pub use executor::{
    ExtendedWorkflowConfig, StepResult, WorkflowContext, WorkflowExecutor as WorkflowExecutorImpl,
    WorkflowMode, WorkflowStep,
};
pub use traits::WorkflowExecutor;
