//! Dry-run mode support for MapReduce workflows
//!
//! This module provides comprehensive validation and preview capabilities
//! for MapReduce workflows without executing actual commands.

pub mod command_validator;
pub mod input_validator;
pub mod output_formatter;
pub mod resource_estimator;
pub mod types;
pub mod validator;
pub mod variable_processor;

#[cfg(test)]
mod tests;

// Re-export main types
pub use output_formatter::OutputFormatter;
pub use types::{
    CommandValidation, DryRunConfig, DryRunError, DryRunReport, ExecutionMode, InputValidation,
    JsonPathValidation, PhaseValidation, ResourceEstimates, ValidationError, ValidationIssue,
    ValidationResults, ValidationWarning, VariablePreview, WorkItemPreview,
};
pub use validator::DryRunValidator;
