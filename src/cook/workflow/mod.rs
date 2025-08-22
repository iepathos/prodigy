//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

mod executor;
mod on_failure;
mod traits;

pub use executor::{
    CaptureOutput, CommandType, ExtendedWorkflowConfig, HandlerStep, StepResult, WorkflowContext,
    WorkflowExecutor as WorkflowExecutorImpl, WorkflowMode, WorkflowStep,
};
pub use on_failure::OnFailureConfig;
pub use traits::WorkflowExecutor;
