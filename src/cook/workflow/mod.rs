//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

pub mod checkpoint;
#[cfg(test)]
mod checkpoint_tests;
#[cfg(test)]
mod conditional_tests;
mod executor;
pub mod normalized;
mod on_failure;
pub mod resume;
mod traits;
pub mod validation;
pub mod variables;

pub use checkpoint::ResumeOptions;
pub use checkpoint::{CheckpointManager, WorkflowCheckpoint};
pub use executor::{
    CaptureOutput, CommandType, ExtendedWorkflowConfig, HandlerStep, StepResult, WorkflowContext,
    WorkflowExecutor as WorkflowExecutorImpl, WorkflowMode, WorkflowStep,
};
pub use normalized::{
    ExecutionMode, MapReduceConfig, NormalizedStep, NormalizedWorkflow, StepCommand, StepHandlers,
    WorkflowType,
};
pub use on_failure::OnFailureConfig;
pub use resume::{ResumeExecutor, ResumeResult};
pub use traits::{StepExecutor, WorkflowExecutor};
pub use validation::{
    GapDetail, OnIncompleteConfig, Severity, ValidationConfig, ValidationResult, ValidationStatus,
};
pub use variables::{ExecutionInput, StandardVariables, VariableContext};
