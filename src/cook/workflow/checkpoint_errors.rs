//! Enhanced error types for checkpoint and resume operations
//!
//! Provides structured, actionable error messages with context and suggestions
//! for troubleshooting checkpoint and resume failures.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during checkpoint operations
#[derive(Debug, Error)]
pub enum CheckpointError {
    /// Checkpoint file not found for the specified session
    #[error("Checkpoint not found for session: {session_id}")]
    NotFound {
        session_id: String,
        checkpoint_dir: PathBuf,
        suggestion: String,
    },

    /// Workflow file not found at expected location
    #[error("Workflow file not found at {}", format_path(workflow_path))]
    WorkflowFileNotFound {
        workflow_path: PathBuf,
        session_id: String,
        checkpoint_created: Option<chrono::DateTime<chrono::Utc>>,
        suggestion: String,
    },

    /// Checkpoint version is not compatible with current code
    #[error(
        "Checkpoint version {checkpoint_version} is not supported (current: {current_version})"
    )]
    VersionMismatch {
        checkpoint_version: u32,
        current_version: u32,
        checkpoint_path: PathBuf,
        checkpoint_created: Option<chrono::DateTime<chrono::Utc>>,
        suggestion: String,
    },

    /// Workflow has changed since checkpoint was created
    #[error("Workflow has changed since checkpoint was created")]
    WorkflowHashMismatch {
        expected_hash: String,
        actual_hash: String,
        checkpoint_steps: usize,
        current_steps: usize,
        session_id: String,
        workflow_path: PathBuf,
        checkpoint_created: Option<chrono::DateTime<chrono::Utc>>,
        suggestion: String,
    },

    /// Environment variable changed since checkpoint
    #[error("Environment has changed since checkpoint")]
    EnvironmentMismatch {
        session_id: String,
        changed_variables: Vec<(String, String, String)>, // (name, old_value, new_value)
        missing_variables: Vec<String>,
        suggestion: String,
    },

    /// Resume lock is held by another process
    #[error("Resume already in progress for session: {session_id}")]
    LockHeld {
        session_id: String,
        process_id: u32,
        hostname: String,
        lock_acquired: chrono::DateTime<chrono::Utc>,
        lock_path: PathBuf,
        suggestion: String,
    },

    /// Invalid checkpoint data
    #[error("Invalid checkpoint: {reason}")]
    InvalidCheckpoint { reason: String, session_id: String },

    /// I/O error during checkpoint operations
    #[error("Failed to {operation}: {source}")]
    IoError {
        operation: String,
        path: Option<PathBuf>,
        source: std::io::Error,
    },

    /// Serialization error
    #[error("Failed to serialize checkpoint: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl CheckpointError {
    /// Format the error with detailed context and suggestions
    pub fn format_detailed(&self) -> String {
        match self {
            CheckpointError::NotFound {
                session_id,
                checkpoint_dir,
                suggestion,
            } => {
                format!(
                    "Checkpoint not found for session: {}\n\
                    \n\
                    Possible causes:\n\
                      • Checkpoint storage was cleared\n\
                      • Session ID is incorrect\n\
                      • Workflow failed before checkpoint could be saved\n\
                    \n\
                    Checkpoint directory: {}\n\
                    \n\
                    Suggestions:\n\
                      • List available sessions: prodigy sessions list --status Failed\n\
                      • Check checkpoint directory: ls -la {}\n\
                      • {}",
                    session_id,
                    format_path(checkpoint_dir),
                    format_path(checkpoint_dir),
                    suggestion
                )
            }

            CheckpointError::WorkflowFileNotFound {
                workflow_path,
                session_id,
                checkpoint_created,
                suggestion,
            } => {
                let created_msg = checkpoint_created
                    .map(|t| {
                        format!(
                            "Checkpoint created: {}\n",
                            t.format("%Y-%m-%d %H:%M:%S UTC")
                        )
                    })
                    .unwrap_or_default();

                format!(
                    "Workflow file not found at: {}\n\
                    \n\
                    The workflow file may have been moved, renamed, or deleted since the checkpoint was created.\n\
                    \n\
                    Session ID: {}\n\
                    {}\
                    Suggestions:\n\
                      • Check if file exists: ls -la {}\n\
                      • Search for workflow: find . -name '*.yml' -o -name '*.yaml'\n\
                      • Specify different path: prodigy resume {} --workflow-path <path>\n\
                      • View checkpoint details: prodigy checkpoints show {}\n\
                      • {}",
                    format_path(workflow_path),
                    session_id,
                    created_msg,
                    format_path(workflow_path),
                    session_id,
                    session_id,
                    suggestion
                )
            }

            CheckpointError::VersionMismatch {
                checkpoint_version,
                current_version,
                checkpoint_path,
                checkpoint_created,
                suggestion,
            } => {
                let created_msg = checkpoint_created
                    .map(|t| format!("Created: {}\n", t.format("%Y-%m-%d %H:%M:%S UTC")))
                    .unwrap_or_default();

                let upgrade_msg = if checkpoint_version > current_version {
                    "This checkpoint was created by a newer version of Prodigy.\n\
                    You need to upgrade Prodigy to resume from this checkpoint.\n"
                } else {
                    "This checkpoint was created by an older version of Prodigy.\n\
                    The checkpoint format may not be compatible.\n"
                };

                format!(
                    "Checkpoint version {} is not supported (current version: {})\n\
                    \n\
                    {}\
                    Checkpoint: {}\n\
                    {}\
                    Suggestions:\n\
                      • Upgrade Prodigy: cargo install prodigy --force\n\
                      • Check current version: prodigy --version\n\
                      • View changelog: https://github.com/prodigy/releases\n\
                      • {}\n\
                    \n\
                    Note: Checkpoints created by newer versions cannot be loaded by older versions.",
                    checkpoint_version,
                    current_version,
                    upgrade_msg,
                    format_path(checkpoint_path),
                    created_msg,
                    suggestion
                )
            }

            CheckpointError::WorkflowHashMismatch {
                expected_hash,
                actual_hash,
                checkpoint_steps,
                current_steps,
                session_id,
                workflow_path,
                checkpoint_created,
                suggestion,
            } => {
                let created_msg = checkpoint_created
                    .map(|t| format!("Created: {}\n", t.format("%Y-%m-%d %H:%M:%S UTC")))
                    .unwrap_or_default();

                let step_change = if checkpoint_steps != current_steps {
                    format!(
                        "  • Step count changed: {} → {} ({} {})\n",
                        checkpoint_steps,
                        current_steps,
                        if current_steps > checkpoint_steps {
                            "added"
                        } else {
                            "removed"
                        },
                        current_steps.abs_diff(*checkpoint_steps)
                    )
                } else {
                    String::new()
                };

                format!(
                    "Workflow has changed since checkpoint was created\n\
                    \n\
                    The workflow file has been modified, which may cause resume to behave unexpectedly.\n\
                    \n\
                    Changes detected:\n\
                    {}  • Workflow hash: {} → {}\n\
                    \n\
                    Checkpoint: {}\n\
                    Workflow: {}\n\
                    {}\
                    Suggestions:\n\
                      • Review workflow changes: git diff HEAD workflow.yml\n\
                      • Restore original: git checkout HEAD~1 -- {}\n\
                      • Force resume (may skip/fail steps): prodigy resume {} --force\n\
                      • {}\n\
                    \n\
                    Warning: Force resume may produce unexpected results if workflow structure changed significantly.",
                    step_change,
                    &expected_hash[..7],
                    &actual_hash[..7],
                    session_id,
                    format_path(workflow_path),
                    created_msg,
                    format_path(workflow_path),
                    session_id,
                    suggestion
                )
            }

            CheckpointError::EnvironmentMismatch {
                session_id,
                changed_variables,
                missing_variables,
                suggestion,
            } => {
                let mut changes_text = String::new();

                if !changed_variables.is_empty() {
                    changes_text.push_str("Changed variables:\n");
                    for (name, old_val, new_val) in changed_variables {
                        changes_text.push_str(&format!(
                            "  • {}: {} → {}\n",
                            name,
                            mask_secret(old_val),
                            mask_secret(new_val)
                        ));
                    }
                }

                if !missing_variables.is_empty() {
                    if !changes_text.is_empty() {
                        changes_text.push('\n');
                    }
                    changes_text.push_str("Missing variables:\n");
                    for name in missing_variables {
                        changes_text.push_str(&format!("  • {}\n", name));
                    }
                }

                format!(
                    "Environment has changed since checkpoint was created\n\
                    \n\
                    The workflow's environment variables have changed, which may affect execution.\n\
                    \n\
                    Session ID: {}\n\
                    \n\
                    {}\
                    Suggestions:\n\
                      • Review environment changes\n\
                      • Reset environment variables to match checkpoint\n\
                      • Force resume if changes are acceptable: prodigy resume {} --force\n\
                      • {}\n\
                    \n\
                    Warning: Environment changes may cause workflow to behave differently.",
                    session_id,
                    changes_text,
                    session_id,
                    suggestion
                )
            }

            CheckpointError::LockHeld {
                session_id,
                process_id,
                hostname,
                lock_acquired,
                lock_path,
                suggestion,
            } => {
                let duration = chrono::Utc::now()
                    .signed_duration_since(*lock_acquired)
                    .num_minutes();

                format!(
                    "Resume already in progress for session: {}\n\
                    \n\
                    Another process is currently resuming this workflow:\n\
                      • Process ID: {}\n\
                      • Hostname: {}\n\
                      • Lock acquired: {} ({} minutes ago)\n\
                    \n\
                    Suggestions:\n\
                      • Wait for the other process to complete\n\
                      • Check if process is still running: ps aux | grep {}\n\
                      • Force resume (if other process crashed): prodigy resume {} --force\n\
                      • View lock details: cat {}\n\
                      • {}\n\
                    \n\
                    Warning: Using --force while another process is running may cause data corruption.",
                    session_id,
                    process_id,
                    hostname,
                    lock_acquired.format("%Y-%m-%d %H:%M:%S UTC"),
                    duration,
                    process_id,
                    session_id,
                    format_path(lock_path),
                    suggestion
                )
            }

            CheckpointError::InvalidCheckpoint { reason, session_id } => {
                format!(
                    "Invalid checkpoint for session {}: {}\n\
                    \n\
                    The checkpoint data is corrupted or invalid.\n\
                    \n\
                    Suggestions:\n\
                      • View checkpoint: prodigy checkpoints show {}\n\
                      • List all checkpoints: prodigy checkpoints list\n\
                      • Remove corrupt checkpoint: prodigy checkpoints delete {}",
                    session_id, reason, session_id, session_id
                )
            }

            CheckpointError::IoError {
                operation,
                path,
                source,
            } => {
                let path_msg = path
                    .as_ref()
                    .map(|p| format!(" at {}", format_path(p)))
                    .unwrap_or_default();

                format!(
                    "Failed to {}{}: {}\n\
                    \n\
                    This is likely a filesystem or permissions issue.\n\
                    \n\
                    Suggestions:\n\
                      • Check disk space: df -h\n\
                      • Check permissions{}\n\
                      • Ensure parent directory exists",
                    operation,
                    path_msg,
                    source,
                    path.as_ref()
                        .map(|p| format!(": ls -ld {}", format_path(p)))
                        .unwrap_or_default()
                )
            }

            CheckpointError::SerializationError(e) => {
                format!(
                    "Failed to serialize checkpoint: {}\n\
                    \n\
                    This may indicate corrupted data or a version incompatibility.\n\
                    \n\
                    Suggestions:\n\
                      • Check checkpoint data integrity\n\
                      • Report this issue if it persists",
                    e
                )
            }
        }
    }

    /// Create a NotFound error with default suggestion
    pub fn not_found(session_id: String, checkpoint_dir: PathBuf) -> Self {
        Self::NotFound {
            session_id: session_id.clone(),
            checkpoint_dir,
            suggestion: "Verify session ID format (should be: session-XXXXXXXXXX or similar)"
                .to_string(),
        }
    }

    /// Create a WorkflowFileNotFound error with default suggestion
    pub fn workflow_file_not_found(
        workflow_path: PathBuf,
        session_id: String,
        checkpoint_created: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self::WorkflowFileNotFound {
            workflow_path,
            session_id,
            checkpoint_created,
            suggestion: "Use --workflow-path flag to specify a different location".to_string(),
        }
    }

    /// Create a VersionMismatch error with default suggestion
    pub fn version_mismatch(
        checkpoint_version: u32,
        current_version: u32,
        checkpoint_path: PathBuf,
        checkpoint_created: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self::VersionMismatch {
            checkpoint_version,
            current_version,
            checkpoint_path,
            checkpoint_created,
            suggestion: if checkpoint_version > current_version {
                "Upgrade to the latest version of Prodigy".to_string()
            } else {
                "This checkpoint may not be compatible with the current version".to_string()
            },
        }
    }

    /// Create a WorkflowHashMismatch error with default suggestion
    pub fn workflow_hash_mismatch(
        expected_hash: String,
        actual_hash: String,
        checkpoint_steps: usize,
        current_steps: usize,
        session_id: String,
        workflow_path: PathBuf,
        checkpoint_created: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self::WorkflowHashMismatch {
            expected_hash,
            actual_hash,
            checkpoint_steps,
            current_steps,
            session_id,
            workflow_path,
            checkpoint_created,
            suggestion: "Review the changes before using --force".to_string(),
        }
    }
}

/// Mask secrets in error messages
///
/// Masks all but the first 4 characters of a value, making it safe
/// to display in error messages without exposing sensitive data.
pub fn mask_secret(value: &str) -> String {
    if value.len() <= 4 {
        "***".to_string()
    } else {
        format!("{}***", &value[..4])
    }
}

/// Format file path for display
///
/// Converts absolute paths to relative paths from home directory when possible
/// for more readable error messages.
pub fn format_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(relative) = path.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret() {
        assert_eq!(mask_secret("sk-abc123def456"), "sk-a***");
        assert_eq!(mask_secret("short"), "shor***"); // 5 chars, shows first 4
        assert_eq!(mask_secret(""), "***");
        assert_eq!(mask_secret("1234567890"), "1234***");
        assert_eq!(mask_secret("abc"), "***"); // <= 4 chars
        assert_eq!(mask_secret("abcd"), "***"); // exactly 4 chars
    }

    #[test]
    fn test_format_path_absolute() {
        let path = PathBuf::from("/tmp/test.txt");
        let formatted = format_path(&path);
        assert_eq!(formatted, "/tmp/test.txt");
    }

    #[test]
    fn test_checkpoint_not_found_error_message() {
        let error = CheckpointError::not_found(
            "session-abc123".to_string(),
            PathBuf::from("/home/user/.prodigy/checkpoints"),
        );

        let message = error.format_detailed();

        // Verify all required elements are present
        assert!(message.contains("session-abc123"));
        assert!(message.contains("Possible causes"));
        assert!(message.contains("Suggestions"));
        assert!(message.contains("prodigy sessions list"));
    }

    #[test]
    fn test_workflow_file_not_found_error_message() {
        let error = CheckpointError::workflow_file_not_found(
            PathBuf::from("/home/user/workflow.yml"),
            "session-abc123".to_string(),
            None,
        );

        let message = error.format_detailed();

        assert!(message.contains("Workflow file not found"));
        assert!(message.contains("session-abc123"));
        assert!(message.contains("--workflow-path"));
        assert!(message.contains("Suggestions"));
    }

    #[test]
    fn test_version_mismatch_error_message() {
        let error = CheckpointError::version_mismatch(
            5,
            3,
            PathBuf::from("/home/user/.prodigy/checkpoint.json"),
            None,
        );

        let message = error.format_detailed();

        assert!(message.contains("version 5"));
        assert!(message.contains("current version: 3"));
        assert!(message.contains("newer version"));
        assert!(message.contains("cargo install prodigy"));
    }

    #[test]
    fn test_lock_held_error_message() {
        let lock_acquired = chrono::Utc::now() - chrono::Duration::minutes(5);
        let error = CheckpointError::LockHeld {
            session_id: "session-abc123".to_string(),
            process_id: 12345,
            hostname: "macbook-pro.local".to_string(),
            lock_acquired,
            lock_path: PathBuf::from("/home/user/.prodigy/locks/session-abc123.lock"),
            suggestion: "Wait for the process to complete or check if it's still running"
                .to_string(),
        };

        let message = error.format_detailed();

        assert!(message.contains("session-abc123"));
        assert!(message.contains("12345"));
        assert!(message.contains("macbook-pro.local"));
        assert!(message.contains("5 minutes ago"));
        assert!(message.contains("ps aux"));
    }
}
