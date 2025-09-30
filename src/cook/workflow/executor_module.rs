//! Workflow executor module
//!
//! This module is being refactored to follow functional programming principles:
//! - Pure functions separated from I/O operations
//! - Clear module boundaries with single responsibilities
//! - Improved testability and composability

pub mod pure;

// Re-export commonly used items
pub use pure::{
    build_iteration_context, calculate_effective_max_iterations, determine_execution_flags,
    determine_iteration_continuation, get_step_display_name, validate_workflow_config,
    ExecutionFlags, IterationContinuation,
};
