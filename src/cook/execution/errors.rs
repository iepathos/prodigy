//! Structured error types for MapReduce operations
//!
//! Provides comprehensive error categorization, rich context for debugging,
//! actionable error messages, and enables programmatic error handling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

/// Main error type for MapReduce operations
#[derive(Debug, Error)]
pub enum MapReduceError {
    // Job-level errors
    #[error("Job {job_id} initialization failed: {reason}")]
    JobInitializationFailed {
        job_id: String,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Job {job_id} already exists")]
    JobAlreadyExists { job_id: String },

    #[error("Job {job_id} not found")]
    JobNotFound { job_id: String },

    #[error("Job {job_id} checkpoint corrupted at version {version}")]
    CheckpointCorrupted {
        job_id: String,
        version: u32,
        details: String,
    },

    // Agent-level errors
    #[error("Agent {agent_id} failed processing item {item_id}: {reason}")]
    AgentFailed {
        job_id: String,
        agent_id: String,
        item_id: String,
        reason: String,
        worktree: Option<String>,
        duration_ms: u64,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Agent {agent_id} timeout after {duration_secs}s")]
    AgentTimeout {
        job_id: String,
        agent_id: String,
        item_id: String,
        duration_secs: u64,
        last_operation: String,
    },

    #[error("Agent {agent_id} resource exhaustion: {resource:?}")]
    ResourceExhausted {
        job_id: String,
        agent_id: String,
        resource: ResourceType,
        limit: String,
        usage: String,
    },

    // Worktree errors
    #[error("Worktree creation failed for agent {agent_id}: {reason}")]
    WorktreeCreationFailed {
        agent_id: String,
        reason: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Worktree merge conflict for agent {agent_id} on branch {branch}")]
    WorktreeMergeConflict {
        agent_id: String,
        branch: String,
        conflicts: Vec<String>,
    },

    // Command execution errors
    #[error("Command execution failed: {command}")]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        working_dir: PathBuf,
    },

    #[error("Shell substitution failed: missing variable {variable}")]
    ShellSubstitutionFailed {
        variable: String,
        command: String,
        available_vars: Vec<String>,
    },

    // I/O errors
    #[error("Failed to persist checkpoint for job {job_id}")]
    CheckpointPersistFailed {
        job_id: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to load work items from {path}")]
    WorkItemLoadFailed {
        path: PathBuf,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // Configuration errors
    #[error("Invalid MapReduce configuration: {reason}")]
    InvalidConfiguration {
        reason: String,
        field: String,
        value: String,
    },

    #[error("JSON path expression invalid: {expression}")]
    InvalidJsonPath {
        expression: String,
        #[source]
        source: serde_json::Error,
    },

    // Concurrency errors
    #[error("Deadlock detected in job {job_id}")]
    DeadlockDetected {
        job_id: String,
        waiting_agents: Vec<String>,
    },

    #[error("Concurrent modification of job {job_id} state")]
    ConcurrentModification { job_id: String, operation: String },

    // General error for migration compatibility
    #[error("{message}")]
    General {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// Resource types that can be exhausted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Memory,
    DiskSpace,
    FileDescriptors,
    ThreadPool,
    NetworkConnections,
}

/// Error context with metadata for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub correlation_id: String,
    pub timestamp: DateTime<Utc>,
    pub hostname: String,
    pub thread_id: String,
    pub span_trace: Vec<SpanInfo>,
}

/// Span information for tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanInfo {
    pub name: String,
    pub start: DateTime<Utc>,
    pub attributes: HashMap<String, String>,
}

/// Error with full context
#[derive(Debug)]
pub struct ContextualError {
    pub error: MapReduceError,
    pub context: ErrorContext,
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (correlation_id: {})",
            self.context.timestamp, self.error, self.context.correlation_id
        )
    }
}

impl std::error::Error for ContextualError {}

impl MapReduceError {
    /// Add context to the error
    pub fn with_context(self, context: ErrorContext) -> ContextualError {
        ContextualError {
            error: self,
            context,
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::AgentTimeout { .. }
                | Self::ResourceExhausted { .. }
                | Self::WorktreeCreationFailed { .. }
                | Self::CheckpointPersistFailed { .. }
        )
    }

    /// Get recovery hint for the error
    pub fn recovery_hint(&self) -> Option<String> {
        match self {
            Self::ResourceExhausted { resource, .. } => Some(format!(
                "Increase {:?} limit or reduce parallelism",
                resource
            )),
            Self::WorktreeMergeConflict { .. } => {
                Some("Manual conflict resolution required".to_string())
            }
            Self::ShellSubstitutionFailed {
                variable,
                available_vars,
                ..
            } => Some(format!(
                "Variable '{}' not found. Available: {:?}",
                variable, available_vars
            )),
            Self::InvalidConfiguration { field, .. } => {
                Some(format!("Check configuration field '{}'", field))
            }
            Self::AgentTimeout { .. } => {
                Some("Consider increasing timeout or optimizing agent commands".to_string())
            }
            _ => None,
        }
    }

    /// Get variant name for error categorization
    pub fn variant_name(&self) -> String {
        match self {
            Self::JobInitializationFailed { .. } => "JobInitializationFailed",
            Self::JobAlreadyExists { .. } => "JobAlreadyExists",
            Self::JobNotFound { .. } => "JobNotFound",
            Self::CheckpointCorrupted { .. } => "CheckpointCorrupted",
            Self::AgentFailed { .. } => "AgentFailed",
            Self::AgentTimeout { .. } => "AgentTimeout",
            Self::ResourceExhausted { .. } => "ResourceExhausted",
            Self::WorktreeCreationFailed { .. } => "WorktreeCreationFailed",
            Self::WorktreeMergeConflict { .. } => "WorktreeMergeConflict",
            Self::CommandFailed { .. } => "CommandFailed",
            Self::ShellSubstitutionFailed { .. } => "ShellSubstitutionFailed",
            Self::CheckpointPersistFailed { .. } => "CheckpointPersistFailed",
            Self::WorkItemLoadFailed { .. } => "WorkItemLoadFailed",
            Self::InvalidConfiguration { .. } => "InvalidConfiguration",
            Self::InvalidJsonPath { .. } => "InvalidJsonPath",
            Self::DeadlockDetected { .. } => "DeadlockDetected",
            Self::ConcurrentModification { .. } => "ConcurrentModification",
            Self::General { .. } => "General",
        }
        .to_string()
    }

    /// Create a general error from anyhow for migration compatibility
    pub fn from_anyhow(err: anyhow::Error) -> Self {
        Self::General {
            message: err.to_string(),
            source: None,
        }
    }
}

/// Convert anyhow errors to MapReduceError for compatibility
impl From<anyhow::Error> for MapReduceError {
    fn from(err: anyhow::Error) -> Self {
        Self::from_anyhow(err)
    }
}

/// Convert IO errors to MapReduceError
impl From<std::io::Error> for MapReduceError {
    fn from(err: std::io::Error) -> Self {
        Self::General {
            message: format!("I/O error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

/// Convert serde_json errors to MapReduceError
impl From<serde_json::Error> for MapReduceError {
    fn from(err: serde_json::Error) -> Self {
        Self::General {
            message: format!("JSON error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

/// Aggregated error for batch operations
#[derive(Debug, Error)]
#[error("Multiple errors occurred during MapReduce execution")]
pub struct AggregatedError {
    pub errors: Vec<MapReduceError>,
    pub total_count: usize,
    pub by_type: HashMap<String, usize>,
}

impl AggregatedError {
    /// Create a new aggregated error
    pub fn new(errors: Vec<MapReduceError>) -> Self {
        let mut by_type = HashMap::new();
        for error in &errors {
            *by_type.entry(error.variant_name()).or_insert(0) += 1;
        }

        Self {
            total_count: errors.len(),
            errors,
            by_type,
        }
    }

    /// Get the most common error type
    pub fn most_common_error(&self) -> Option<&str> {
        self.by_type
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name.as_str())
    }

    /// Get a summary of errors
    pub fn summary(&self) -> String {
        let mut summary = format!("Total errors: {}\n", self.total_count);
        for (error_type, count) in &self.by_type {
            summary.push_str(&format!("  {}: {}\n", error_type, count));
        }
        summary
    }
}

/// Error report with full details
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorReport {
    pub error: String,
    pub context: ErrorContext,
    pub stack_trace: Vec<String>,
    pub related_errors: Vec<String>,
}

/// Error statistics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorStats {
    pub total_errors: usize,
    pub errors_by_type: HashMap<String, usize>,
    pub error_rate: f64,
    pub mean_time_to_recovery_ms: u64,
}

/// Result type alias for MapReduce operations
pub type MapReduceResult<T> = Result<T, MapReduceError>;

/// Error handler trait for custom error handling strategies
pub trait ErrorHandler: Send + Sync {
    /// Handle an error and determine action
    fn handle_error(&self, error: &MapReduceError) -> ErrorAction;

    /// Check if error should be retried
    fn should_retry(&self, error: &MapReduceError) -> bool {
        error.is_retryable()
    }

    /// Calculate retry delay based on error and attempt number
    fn retry_delay(&self, error: &MapReduceError, attempt: u32) -> std::time::Duration {
        use std::time::Duration;
        // Exponential backoff with jitter
        let base_delay = match error {
            MapReduceError::AgentTimeout { .. } => 30,
            MapReduceError::ResourceExhausted { .. } => 60,
            _ => 10,
        };
        Duration::from_secs(base_delay * 2_u64.pow(attempt.min(5)))
    }
}

/// Action to take for an error
#[derive(Debug, Clone)]
pub enum ErrorAction {
    Retry { delay: std::time::Duration },
    Fallback { handler: String },
    Propagate,
    Ignore,
    Abort,
}

/// Default error handler implementation
pub struct DefaultErrorHandler;

impl ErrorHandler for DefaultErrorHandler {
    fn handle_error(&self, error: &MapReduceError) -> ErrorAction {
        if error.is_retryable() {
            ErrorAction::Retry {
                delay: std::time::Duration::from_secs(10),
            }
        } else {
            ErrorAction::Propagate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_retryable() {
        let timeout_error = MapReduceError::AgentTimeout {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            item_id: "item1".to_string(),
            duration_secs: 60,
            last_operation: "processing".to_string(),
        };
        assert!(timeout_error.is_retryable());

        let config_error = MapReduceError::InvalidConfiguration {
            reason: "invalid".to_string(),
            field: "timeout".to_string(),
            value: "-1".to_string(),
        };
        assert!(!config_error.is_retryable());
    }

    #[test]
    fn test_recovery_hints() {
        let resource_error = MapReduceError::ResourceExhausted {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            resource: ResourceType::Memory,
            limit: "1GB".to_string(),
            usage: "1.2GB".to_string(),
        };
        assert!(resource_error.recovery_hint().is_some());
    }

    #[test]
    fn test_aggregated_error() {
        let errors = vec![
            MapReduceError::AgentTimeout {
                job_id: "job1".to_string(),
                agent_id: "agent1".to_string(),
                item_id: "item1".to_string(),
                duration_secs: 60,
                last_operation: "processing".to_string(),
            },
            MapReduceError::AgentTimeout {
                job_id: "job1".to_string(),
                agent_id: "agent2".to_string(),
                item_id: "item2".to_string(),
                duration_secs: 60,
                last_operation: "processing".to_string(),
            },
            MapReduceError::JobNotFound {
                job_id: "job2".to_string(),
            },
        ];

        let aggregated = AggregatedError::new(errors);
        assert_eq!(aggregated.total_count, 3);
        assert_eq!(aggregated.most_common_error(), Some("AgentTimeout"));
    }

    #[test]
    fn test_variant_names() {
        let error = MapReduceError::JobAlreadyExists {
            job_id: "test".to_string(),
        };
        assert_eq!(error.variant_name(), "JobAlreadyExists");
    }
}
