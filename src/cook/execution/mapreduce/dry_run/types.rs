//! Type definitions for dry-run mode
//!
//! This module defines the types used throughout the dry-run validation system.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use thiserror::Error;

/// Execution mode for MapReduce operations
#[derive(Debug, Clone, Default)]
pub enum ExecutionMode {
    /// Normal execution mode
    #[default]
    Normal,
    /// Dry-run validation mode
    DryRun(DryRunConfig),
}

/// Configuration for dry-run mode
#[derive(Debug, Clone, Default)]
pub struct DryRunConfig {
    /// Show work item preview
    pub show_work_items: bool,
    /// Show variable interpolation preview
    pub show_variables: bool,
    /// Show resource estimates
    pub show_resources: bool,
    /// Limit work item preview to N items
    pub sample_size: Option<usize>,
}

/// Error type for dry-run validation
#[derive(Error, Debug)]
pub enum DryRunError {
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Input source error: {0}")]
    InputError(String),

    #[error("JSONPath error: {0}")]
    JsonPathError(String),

    #[error("Variable interpolation error: {0}")]
    VariableError(String),

    #[error("Command validation error: {0}")]
    CommandError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Complete dry-run validation report
#[derive(Debug, Serialize)]
pub struct DryRunReport {
    /// Overall validation results
    pub validation_results: ValidationResults,
    /// Work item preview if requested
    pub work_item_preview: WorkItemPreview,
    /// Resource usage estimates
    pub resource_estimates: ResourceEstimates,
    /// Variable interpolation preview
    pub variable_preview: VariablePreview,
    /// Non-critical warnings
    pub warnings: Vec<ValidationWarning>,
    /// Critical errors that would prevent execution
    pub errors: Vec<ValidationError>,
    /// Execution time estimate
    pub estimated_duration: Duration,
}

/// Validation results for all phases
#[derive(Debug, Serialize)]
pub struct ValidationResults {
    /// Setup phase validation
    pub setup_phase: Option<PhaseValidation>,
    /// Map phase validation
    pub map_phase: PhaseValidation,
    /// Reduce phase validation
    pub reduce_phase: Option<PhaseValidation>,
    /// Overall validation success
    pub is_valid: bool,
}

/// Validation result for a single phase
#[derive(Debug, Serialize)]
pub struct PhaseValidation {
    /// Whether the phase is valid
    pub valid: bool,
    /// Number of commands in the phase
    pub command_count: usize,
    /// Estimated duration for the phase
    pub estimated_duration: Duration,
    /// Whether all dependencies are met
    pub dependencies_met: bool,
    /// Validation issues found
    pub issues: Vec<ValidationIssue>,
}

/// Work item preview information
#[derive(Debug, Serialize, Default)]
pub struct WorkItemPreview {
    /// Total number of work items
    pub total_count: usize,
    /// Sample of work items (limited by sample_size)
    pub sample_items: Vec<Value>,
    /// Distribution across agents
    pub distribution: HashMap<usize, usize>, // agent_id -> item_count
    /// Filtered count (if filter is applied)
    pub filtered_count: Option<usize>,
    /// Sort order description
    pub sort_description: Option<String>,
}

/// Resource usage estimates
#[derive(Debug, Serialize)]
pub struct ResourceEstimates {
    /// Memory usage estimate
    pub memory_usage: MemoryEstimate,
    /// Disk usage estimate
    pub disk_usage: DiskEstimate,
    /// Network usage estimate
    pub network_usage: NetworkEstimate,
    /// Number of worktrees to be created
    pub worktree_count: usize,
    /// Checkpoint storage requirements
    pub checkpoint_storage: StorageEstimate,
}

/// Memory usage estimate
#[derive(Debug, Serialize)]
pub struct MemoryEstimate {
    /// Total memory in MB
    pub total_mb: usize,
    /// Memory per agent in MB
    pub per_agent_mb: usize,
    /// Peak concurrent agents
    pub peak_concurrent_agents: usize,
}

/// Disk usage estimate
#[derive(Debug, Serialize)]
pub struct DiskEstimate {
    /// Total disk space in MB
    pub total_mb: usize,
    /// Space per worktree in MB
    pub per_worktree_mb: usize,
    /// Temporary file space in MB
    pub temp_space_mb: usize,
}

/// Network usage estimate
#[derive(Debug, Serialize)]
pub struct NetworkEstimate {
    /// Estimated data transfer in MB
    pub data_transfer_mb: usize,
    /// Number of API calls
    pub api_calls: usize,
    /// Parallel network operations
    pub parallel_operations: usize,
}

/// Storage estimate for checkpoints
#[derive(Debug, Serialize)]
pub struct StorageEstimate {
    /// Size per checkpoint in KB
    pub checkpoint_size_kb: usize,
    /// Number of checkpoints
    pub checkpoint_count: usize,
    /// Total storage in MB
    pub total_mb: usize,
}

/// Variable interpolation preview
#[derive(Debug, Serialize, Default)]
pub struct VariablePreview {
    /// Variables available in setup phase
    pub setup_variables: HashMap<String, String>,
    /// Sample of item variables (map phase)
    pub item_variables: Vec<HashMap<String, String>>,
    /// Variables available in reduce phase
    pub reduce_variables: HashMap<String, String>,
    /// Undefined variable references
    pub undefined_references: Vec<String>,
}

/// Input source validation result
#[derive(Debug, Serialize)]
pub struct InputValidation {
    /// Input source path or command
    pub source: String,
    /// Whether the input is valid
    pub valid: bool,
    /// Size of input data in bytes
    pub size_bytes: usize,
    /// Estimated number of items
    pub item_count_estimate: usize,
    /// Data structure description
    pub data_structure: String,
}

/// JSONPath validation result
#[derive(Debug, Serialize)]
pub struct JsonPathValidation {
    /// JSONPath expression
    pub path: String,
    /// Whether the path is valid
    pub valid: bool,
    /// Number of matches found
    pub match_count: usize,
    /// Sample of matched values
    pub sample_matches: Vec<Value>,
    /// Data types found in matches
    pub data_types: HashMap<String, usize>,
}

/// Command validation result
#[derive(Debug, Serialize)]
pub struct CommandValidation {
    /// Type of command
    pub command_type: CommandType,
    /// Whether the command is valid
    pub valid: bool,
    /// Validation issues
    pub issues: Vec<ValidationIssue>,
    /// Variable references in the command
    pub variable_references: Vec<VariableReference>,
    /// Estimated execution duration
    pub estimated_duration: Duration,
}

/// Type of workflow command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandType {
    Claude,
    Shell,
    Foreach,
}

impl fmt::Display for CommandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandType::Claude => write!(f, "claude"),
            CommandType::Shell => write!(f, "shell"),
            CommandType::Foreach => write!(f, "foreach"),
        }
    }
}

/// Reference to a variable in a command
#[derive(Debug, Serialize)]
pub struct VariableReference {
    /// Variable name
    pub name: String,
    /// Context where the variable is used
    pub context: VariableContext,
}

/// Context for variable usage
#[derive(Debug, Serialize)]
pub enum VariableContext {
    Item,
    Map,
    Setup,
    Shell,
    Merge,
    Unknown,
}

/// Validation issue (can be error or warning)
#[derive(Debug, Clone, Serialize)]
pub enum ValidationIssue {
    Error(String),
    Warning(String),
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationIssue::Error(msg) => write!(f, "ERROR: {}", msg),
            ValidationIssue::Warning(msg) => write!(f, "WARNING: {}", msg),
        }
    }
}

/// Validation warning (non-critical)
#[derive(Debug, Clone, Serialize)]
pub struct ValidationWarning {
    pub phase: String,
    pub message: String,
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.phase, self.message)
    }
}

/// Validation error (critical)
#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub phase: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.phase, self.message)
    }
}

/// Duration estimate for execution
#[derive(Debug, Serialize)]
pub struct DurationEstimate {
    /// Total estimated duration
    pub total: Duration,
    /// Setup phase duration
    pub setup_phase: Duration,
    /// Map phase duration
    pub map_phase: Duration,
    /// Reduce phase duration
    pub reduce_phase: Duration,
}
