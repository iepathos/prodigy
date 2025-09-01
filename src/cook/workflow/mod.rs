//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

mod executor;
mod on_failure;
mod traits;
pub mod validation;

pub use executor::{
    CaptureOutput, CommandType, ExtendedWorkflowConfig, HandlerStep, StepResult, WorkflowContext,
    WorkflowExecutor as WorkflowExecutorImpl, WorkflowMode, WorkflowStep,
};
pub use on_failure::OnFailureConfig;
pub use traits::WorkflowExecutor;
pub use validation::{
    CompletionStrategy, GapDetail, OnIncompleteConfig, Severity, ValidationConfig,
    ValidationResult, ValidationStatus, ValidationType,
};
