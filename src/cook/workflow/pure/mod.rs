//! Pure workflow transformations module
//!
//! This module contains pure, side-effect-free functions for workflow transformations:
//! - Command building from templates
//! - Variable expansion (${VAR}, $VAR patterns)
//! - Variable reference extraction
//! - Output parsing (regex, JSON path, line-based)
//!
//! All functions in this module are:
//! - Pure: No I/O operations, no side effects
//! - Deterministic: Same inputs always produce same outputs
//! - Testable: No mocking required for unit tests
//!
//! These functions will be consumed by the workflow executor in spec 174f
//! and effect modules in spec 174d.

pub mod command_builder;
pub mod output_parser;
pub mod resume_planning;
pub mod step_planning;
pub mod variable_expansion;

// Re-export primary functions and types
pub use command_builder::build_command;
pub use output_parser::{parse_output_variables, OutputPattern};
pub use resume_planning::{plan_resume, validate_checkpoint_compatibility, ResumePlan};
pub use step_planning::{is_safe_to_resume, plan_steps, ResumeDecision, StepPlan};
pub use variable_expansion::{expand_variables, extract_variable_references};
