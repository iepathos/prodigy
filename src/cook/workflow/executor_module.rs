//! Workflow executor module
//!
//! This module is being refactored to follow functional programming principles:
//! - Pure functions separated from I/O operations (pure.rs)
//! - Command execution separated from orchestration (commands.rs)
//! - Clear module boundaries with single responsibilities
//! - Improved testability and composability

pub mod commands;
pub mod pure;

// Re-export commonly used items from pure module
pub use pure::{
    build_iteration_context, calculate_effective_max_iterations, determine_execution_flags,
    determine_iteration_continuation, get_step_display_name, validate_workflow_config,
    ExecutionFlags, IterationContinuation,
};

// Re-export commonly used items from commands module
pub use commands::{
    execute_claude_command, execute_foreach_command, execute_goal_seek_command,
    execute_shell_command, format_command_description,
};
