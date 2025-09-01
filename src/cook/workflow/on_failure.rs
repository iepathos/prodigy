//! On-failure handling for workflow steps
//!
//! Provides flexible error handling options for workflow commands.

use super::{CaptureOutput, WorkflowStep};
use serde::{Deserialize, Serialize};

/// Configuration for handling command failures
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OnFailureConfig {
    /// Simple ignore errors flag
    IgnoreErrors(bool),

    /// Advanced configuration with handler and control flags
    /// This must come before FailControl because it has more specific fields
    Advanced {
        /// Shell command to execute on failure
        #[serde(skip_serializing_if = "Option::is_none")]
        shell: Option<String>,

        /// Claude command to execute on failure
        #[serde(skip_serializing_if = "Option::is_none")]
        claude: Option<String>,

        /// Whether to fail the workflow after handling
        #[serde(default = "default_fail")]
        fail_workflow: bool,

        /// Whether to retry the original command after handling
        #[serde(default = "default_retry_original")]
        retry_original: bool,

        /// Maximum retry attempts (supports both max_retries and max_attempts)
        #[serde(default = "default_retries", alias = "max_attempts")]
        max_retries: u32,
    },

    /// Just control whether to fail the workflow
    FailControl {
        #[serde(default)]
        fail_workflow: bool,
    },

    /// Execute a handler command
    Handler(Box<WorkflowStep>),
}

fn default_fail() -> bool {
    false // By default, don't fail if there's a handler
}

fn default_retries() -> u32 {
    1
}

fn default_retry_original() -> bool {
    false // This field is now deprecated - we use max_retries > 0 to determine retry behavior
}

impl OnFailureConfig {
    /// Check if the workflow should fail after handling this error
    pub fn should_fail_workflow(&self) -> bool {
        match self {
            OnFailureConfig::IgnoreErrors(false) => true,
            OnFailureConfig::IgnoreErrors(true) => false,
            OnFailureConfig::Advanced { fail_workflow, .. } => *fail_workflow,
            OnFailureConfig::FailControl { fail_workflow } => *fail_workflow,
            OnFailureConfig::Handler(_) => false, // If there's a handler, don't fail by default
        }
    }

    /// Get the handler command if any
    pub fn handler(&self) -> Option<WorkflowStep> {
        match self {
            OnFailureConfig::Advanced { shell, claude, .. } => {
                if shell.is_some() || claude.is_some() {
                    Some(WorkflowStep {
                        name: None,
                        shell: shell.clone(),
                        claude: claude.clone(),
                        test: None,
                        command: None,
                        handler: None,
                        timeout: None,
                        capture_output: CaptureOutput::Disabled,
                        on_failure: None,
                        on_success: None,
                        on_exit_code: Default::default(),
                        commit_required: false,
                        working_dir: None,
                        env: Default::default(),
                        validate: None,
                    })
                } else {
                    None
                }
            }
            OnFailureConfig::Handler(step) => Some((**step).clone()),
            _ => None,
        }
    }

    /// Check if the original command should be retried
    /// If max_retries > 0, we should retry (consistent with regular workflow behavior)
    pub fn should_retry(&self) -> bool {
        // If max_retries > 0, we should retry regardless of retry_original
        // This matches the behavior of regular workflows where max_attempts implies retry
        self.max_retries() > 0
    }

    /// Get maximum retry attempts
    pub fn max_retries(&self) -> u32 {
        match self {
            OnFailureConfig::Advanced { max_retries, .. } => *max_retries,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ignore_errors() {
        let yaml = "true";
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.should_fail_workflow());

        let yaml = "false";
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.should_fail_workflow());
    }

    #[test]
    fn test_parse_fail_control() {
        let yaml = "fail_workflow: true";
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.should_fail_workflow());

        let yaml = "fail_workflow: false";
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.should_fail_workflow());
    }

    #[test]
    fn test_parse_handler() {
        // The Handler variant expects a full WorkflowStep, not just shell command
        // For simple shell commands, they get parsed as Advanced variant
        let yaml = r#"
shell: "echo 'Handling error'"
"#;
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.handler().is_some());
        assert!(!config.should_fail_workflow()); // Default: don't fail with handler
    }

    #[test]
    fn test_parse_advanced() {
        let yaml = r#"
shell: "fix-error"
fail_workflow: true
retry_original: true
max_retries: 3
"#;
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.handler().is_some());
        assert!(config.should_fail_workflow());
        assert!(config.should_retry());
        assert_eq!(config.max_retries(), 3);
    }

    #[test]
    fn test_max_attempts_implies_retry() {
        // Test that max_attempts > 0 implies retry without retry_original
        let yaml = r#"
claude: "/fix-error"
max_attempts: 3
fail_workflow: false
"#;
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.handler().is_some());
        assert!(!config.should_fail_workflow());
        // should_retry() should be true because max_retries (from max_attempts) is 3
        assert!(config.should_retry());
        assert_eq!(config.max_retries(), 3);
    }
}
