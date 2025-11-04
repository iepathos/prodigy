//! Data structures and types for workflow execution
//!
//! This module contains all the core data structures used by the workflow executor,
//! including step definitions, configuration types, and helper functions.
//!
//! ## Key Types
//!
//! - [`WorkflowStep`]: Configuration for individual workflow steps
//! - [`ExtendedWorkflowConfig`]: Top-level workflow configuration
//! - [`WorkflowMode`]: Execution mode (Sequential or MapReduce)
//! - [`SensitivePatternConfig`]: Configuration for masking sensitive data
//! - [`HandlerStep`]: Modular command handler configuration

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cook::workflow::on_failure::OnFailureConfig;
use crate::cook::workflow::validation::ValidationConfig;

use super::types::{deserialize_capture_output, CaptureOutput};

/// Handler step configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerStep {
    /// Handler name (e.g., "shell", "claude", "git", "cargo", "file")
    pub name: String,
    /// Attributes to pass to the handler
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

/// A workflow step with extended syntax support
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Legacy step name (for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Claude CLI command with args
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<String>,

    /// Shell command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Test command configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<crate::config::command::TestCommand>,

    /// Goal-seeking configuration for iterative refinement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_seek: Option<crate::cook::goal_seek::GoalSeekConfig>,

    /// Foreach configuration for parallel iteration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreach: Option<crate::config::command::ForeachConfig>,

    /// Write file configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_file: Option<crate::config::command::WriteFileConfig>,

    /// Legacy command field (for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Modular command handler with attributes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<HandlerStep>,

    /// Variable name to capture output into
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,

    /// Format for captured output (string, json, lines, number, boolean)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_format: Option<super::super::variables::CaptureFormat>,

    /// Which streams to capture (stdout, stderr, exit_code, etc.)
    #[serde(default)]
    pub capture_streams: super::super::variables::CaptureStreams,

    /// Output file for command results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<std::path::PathBuf>,

    /// Whether to capture command output (bool or variable name string) - DEPRECATED
    #[serde(default, deserialize_with = "deserialize_capture_output")]
    pub capture_output: CaptureOutput,

    /// Timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// Working directory for the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<PathBuf>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Conditional execution on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailureConfig>,

    /// Enhanced retry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<crate::cook::retry_v2::RetryConfig>,

    /// Conditional execution on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_success: Option<Box<WorkflowStep>>,

    /// Conditional execution based on exit codes
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub on_exit_code: HashMap<i32, Box<WorkflowStep>>,

    /// Whether this command is expected to create commits
    #[serde(default = "default_commit_required")]
    pub commit_required: bool,

    /// Commit configuration for auto-commit and validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_config: Option<crate::cook::commit_tracker::CommitConfig>,

    /// Auto-commit if changes detected
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub auto_commit: bool,

    /// Validation configuration for checking implementation completeness
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate: Option<ValidationConfig>,

    /// Step validation that runs after command execution to verify success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_validate: Option<super::super::step_validation::StepValidationSpec>,

    /// Skip step validation even if specified
    #[serde(default)]
    pub skip_validation: bool,

    /// Validation-specific timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_timeout: Option<u64>,

    /// Continue on validation failure
    #[serde(default)]
    pub ignore_validation_failure: bool,

    /// Conditional execution expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// Default value for commit_required field
pub(crate) fn default_commit_required() -> bool {
    false
}

/// Helper for serde to skip serializing false boolean values
pub(crate) fn is_false(b: &bool) -> bool {
    !b
}

/// Helper function to compile regex with fallback
pub(crate) fn compile_regex(pattern: &str) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(regex) => Some(regex),
        Err(e) => {
            eprintln!("Warning: Failed to compile regex '{}': {}", pattern, e);
            None
        }
    }
}

/// Configuration for sensitive variable patterns
#[derive(Debug, Clone)]
pub struct SensitivePatternConfig {
    /// Regex patterns to identify sensitive variable names
    pub name_patterns: Vec<Regex>,
    /// Regex patterns to identify sensitive values
    pub value_patterns: Vec<Regex>,
    /// Custom masking string (default: "***REDACTED***")
    pub mask_string: String,
}

impl Default for SensitivePatternConfig {
    fn default() -> Self {
        Self {
            // Default patterns for common sensitive variable names
            name_patterns: [
                r"(?i)(password|passwd|pwd)",
                r"(?i)(token|api[_-]?key|secret)",
                r"(?i)(auth|authorization|bearer)",
                r"(?i)(private[_-]?key|ssh[_-]?key)",
                r"(?i)(access[_-]?key|client[_-]?secret)",
            ]
            .iter()
            .filter_map(|p| compile_regex(p))
            .collect(),
            // Default patterns for common sensitive value formats
            value_patterns: [
                // GitHub/GitLab tokens (ghp_, glpat-, etc.)
                r"^(ghp_|gho_|ghu_|ghs_|ghr_|glpat-)",
                // AWS access keys
                r"^AKIA[0-9A-Z]{16}$",
                // JWT tokens
                r"^eyJ[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$",
                // Basic auth headers
                r"^(Basic|Bearer)\s+[A-Za-z0-9+/=]+$",
            ]
            .iter()
            .filter_map(|p| compile_regex(p))
            .collect(),
            mask_string: "***REDACTED***".to_string(),
        }
    }
}

/// Workflow execution mode
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowMode {
    /// Sequential execution (default)
    Sequential,
    /// MapReduce parallel execution
    MapReduce,
}

/// Extended workflow configuration
#[derive(Debug, Clone)]
pub struct ExtendedWorkflowConfig {
    /// Workflow name
    pub name: String,
    /// Execution mode
    pub mode: WorkflowMode,
    /// Steps to execute (for sequential mode or simple setup phase)
    pub steps: Vec<WorkflowStep>,
    /// Setup phase configuration (for advanced MapReduce setup with timeout and capture)
    pub setup_phase: Option<crate::cook::execution::SetupPhase>,
    /// Map phase configuration (for mapreduce mode)
    pub map_phase: Option<crate::cook::execution::MapPhase>,
    /// Reduce phase configuration (for mapreduce mode)
    pub reduce_phase: Option<crate::cook::execution::ReducePhase>,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Whether to iterate
    pub iterate: bool,
    /// Global retry defaults (applied to all steps unless overridden)
    pub retry_defaults: Option<crate::cook::retry_v2::RetryConfig>,
    /// Global environment configuration
    pub environment: Option<crate::cook::environment::EnvironmentConfig>,
    // collect_metrics removed - MMM focuses on orchestration, not metrics
}
