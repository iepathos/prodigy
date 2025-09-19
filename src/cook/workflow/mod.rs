//! Workflow execution module
//!
//! Handles command execution with git commit verification and iteration logic.

pub mod checkpoint;
#[cfg(test)]
mod checkpoint_tests;
#[cfg(test)]
mod commit_tracking_tests;
pub mod composition;
#[cfg(test)]
mod conditional_tests;
pub mod error_policy;
#[cfg(test)]
mod error_policy_tests;
pub mod error_recovery;
#[cfg(test)]
mod error_recovery_tests;
mod executor;
#[cfg(test)]
mod executor_tests;
pub mod git_context;
#[cfg(test)]
mod git_context_tests;
pub mod normalized;
mod on_failure;
pub mod progress;
pub mod progress_config;
pub mod resume;
pub mod step_validation;
#[cfg(test)]
mod step_validation_tests;
mod traits;
pub mod validation;
pub mod variable_checkpoint;
#[cfg(test)]
mod variable_checkpoint_tests;
pub mod variables;

pub use checkpoint::ResumeOptions;
pub use checkpoint::{CheckpointManager, WorkflowCheckpoint};
pub use composition::{
    ComposableWorkflow, ComposedWorkflow, CompositionMetadata, Parameter, ParameterDefinitions,
    ParameterType, SubWorkflow, SubWorkflowExecutor, SubWorkflowResult, TemplateRegistry,
    TemplateSource, TemplateStorage, WorkflowComposer, WorkflowImport, WorkflowTemplate,
};
pub use error_policy::{
    BackoffStrategy, CircuitBreaker, CircuitBreakerConfig, CircuitState, ErrorCollectionStrategy,
    ErrorMetrics, ErrorPolicyExecutor, FailureAction, FailurePattern, ItemFailureAction,
    RetryConfig, WorkflowErrorPolicy,
};
pub use executor::{
    CaptureOutput, CommandType, ExtendedWorkflowConfig, HandlerStep, StepResult, WorkflowContext,
    WorkflowExecutor as WorkflowExecutorImpl, WorkflowMode, WorkflowStep,
};
pub use git_context::{GitChangeTracker, StepChanges, VariableFormat};
pub use normalized::{
    ExecutionMode, MapReduceConfig, NormalizedStep, NormalizedWorkflow, StepCommand, StepHandlers,
    WorkflowType,
};
pub use on_failure::{FailureHandlerConfig, HandlerCommand, HandlerStrategy, OnFailureConfig};
pub use progress_config::{LogLevel, ProgressConfig, ProgressDisplayMode};
pub use resume::{ResumeExecutor, ResumeResult};
pub use step_validation::{
    StepValidationConfig, StepValidationExecutor, StepValidationResult, StepValidationSpec,
};
pub use traits::{StepExecutor, WorkflowExecutor};
pub use validation::{
    GapDetail, OnIncompleteConfig, Severity, ValidationConfig, ValidationResult, ValidationStatus,
};
pub use variables::{
    CaptureFormat, CaptureStreams, CapturedValue, CommandResult, ExecutionInput, StandardVariables,
    VariableContext, VariableStore,
};
