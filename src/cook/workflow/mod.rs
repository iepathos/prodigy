//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

mod executor;
pub mod normalized;
mod on_failure;
mod traits;
pub mod validation;

pub use executor::{
    CaptureOutput, CommandType, ExtendedWorkflowConfig, HandlerStep, StepResult, WorkflowContext,
    WorkflowExecutor as WorkflowExecutorImpl, WorkflowMode, WorkflowStep,
};
pub use normalized::{
    ExecutionMode, MapReduceConfig, NormalizedStep, NormalizedWorkflow, StepCommand, StepHandlers,
    WorkflowType,
};
pub use on_failure::OnFailureConfig;
pub use traits::WorkflowExecutor;
pub use validation::{
    GapDetail, OnIncompleteConfig, Severity, ValidationConfig, ValidationResult, ValidationStatus,
};
