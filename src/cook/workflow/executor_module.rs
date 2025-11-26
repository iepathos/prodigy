//! Workflow executor module
//!
//! This module is being refactored to follow functional programming principles:
//! - Pure functions separated from I/O operations (pure.rs)
//! - Command execution separated from orchestration (commands.rs)
//! - Specialized commands in their own module (specialized_commands.rs)
//! - Failure handling and retry logic isolated (failure_handler.rs)
//! - Clear module boundaries with single responsibilities
//! - Improved testability and composability

pub mod commands;
pub mod failure_handler;
pub mod orchestration;
pub mod pure;
pub mod specialized_commands;

// Re-export commonly used items from pure module
pub use pure::{
    build_iteration_context, calculate_effective_max_iterations, determine_execution_flags,
    determine_iteration_continuation, get_step_display_name, validate_workflow_config,
    ExecutionFlags, IterationContinuation,
};

// Re-export commonly used items from commands module
pub use commands::{execute_claude_command, execute_shell_command, format_command_description};

// Re-export specialized command functions
pub use specialized_commands::{execute_foreach_command, execute_write_file_command};

// Re-export commonly used items from failure_handler module
pub use failure_handler::{
    append_handler_output, build_retry_exhausted_message, calculate_retry_delay,
    create_retry_attempt, determine_recovery_strategy, format_retry_message,
    format_retry_success_message, is_handler_failure_fatal, mark_step_recovered,
    should_attempt_retry, should_retry_error, RetryContext,
};

// Re-export commonly used items from orchestration module
pub use orchestration::{
    build_checkpoint_step, build_session_step_result, calculate_progress_percentage,
    create_normalized_workflow, create_workflow_hash, format_iteration_progress,
    format_skip_step, format_step_progress, format_workflow_start, should_continue_iteration,
};
