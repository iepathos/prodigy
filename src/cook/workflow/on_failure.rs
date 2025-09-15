//! On-failure handling for workflow steps
//!
//! Provides flexible error handling options for workflow commands.

use super::{CaptureOutput, WorkflowStep};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Handler execution strategy
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HandlerStrategy {
    /// Try to fix the problem
    #[default]
    Recovery,
    /// Use alternative approach
    Fallback,
    /// Clean up resources
    Cleanup,
    /// Custom handler logic
    Custom,
}

/// Detailed failure handler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureHandlerConfig {
    /// Commands to execute on failure
    pub commands: Vec<HandlerCommand>,

    /// Handler execution strategy
    #[serde(default)]
    pub strategy: HandlerStrategy,

    /// Maximum handler execution time in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// Variables to capture from handler
    #[serde(default)]
    pub capture: HashMap<String, String>,

    /// Whether to fail the workflow after handling
    #[serde(default = "default_fail")]
    pub fail_workflow: bool,

    /// Whether handler failure should be fatal
    #[serde(default)]
    pub handler_failure_fatal: bool,
}

/// A command to execute in the handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerCommand {
    /// Shell command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Claude command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<String>,

    /// Continue to next handler command even if this fails
    #[serde(default)]
    pub continue_on_error: bool,
}

/// Configuration for handling command failures
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OnFailureConfig {
    /// Simple ignore errors flag
    IgnoreErrors(bool),

    /// Single command string (shell or claude)
    SingleCommand(String),

    /// Multiple command strings
    MultipleCommands(Vec<String>),

    /// Detailed handler configuration
    Detailed(FailureHandlerConfig),

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
            OnFailureConfig::SingleCommand(_) => false,
            OnFailureConfig::MultipleCommands(_) => false,
            OnFailureConfig::Detailed(config) => config.fail_workflow,
            OnFailureConfig::Advanced { fail_workflow, .. } => *fail_workflow,
            OnFailureConfig::FailControl { fail_workflow } => *fail_workflow,
            OnFailureConfig::Handler(_) => false, // If there's a handler, don't fail by default
        }
    }

    /// Get handler commands as a vector
    pub fn handler_commands(&self) -> Vec<HandlerCommand> {
        match self {
            OnFailureConfig::SingleCommand(cmd) => {
                // Detect if it's a shell or claude command
                vec![if cmd.starts_with("/") {
                    HandlerCommand {
                        claude: Some(cmd.clone()),
                        shell: None,
                        continue_on_error: false,
                    }
                } else {
                    HandlerCommand {
                        shell: Some(cmd.clone()),
                        claude: None,
                        continue_on_error: false,
                    }
                }]
            }
            OnFailureConfig::MultipleCommands(cmds) => cmds
                .iter()
                .map(|cmd| {
                    if cmd.starts_with("/") {
                        HandlerCommand {
                            claude: Some(cmd.clone()),
                            shell: None,
                            continue_on_error: false,
                        }
                    } else {
                        HandlerCommand {
                            shell: Some(cmd.clone()),
                            claude: None,
                            continue_on_error: false,
                        }
                    }
                })
                .collect(),
            OnFailureConfig::Detailed(config) => config.commands.clone(),
            OnFailureConfig::Advanced { shell, claude, .. } => {
                let mut commands = Vec::new();
                if let Some(sh) = shell {
                    commands.push(HandlerCommand {
                        shell: Some(sh.clone()),
                        claude: None,
                        continue_on_error: false,
                    });
                }
                if let Some(cl) = claude {
                    commands.push(HandlerCommand {
                        claude: Some(cl.clone()),
                        shell: None,
                        continue_on_error: false,
                    });
                }
                commands
            }
            _ => Vec::new(),
        }
    }

    /// Get the handler command if any (for backward compatibility)
    pub fn handler(&self) -> Option<WorkflowStep> {
        match self {
            OnFailureConfig::Advanced { shell, claude, .. } => {
                if shell.is_some() || claude.is_some() {
                    Some(WorkflowStep {
                        name: None,
                        shell: shell.clone(),
                        claude: claude.clone(),
                        test: None,
                        goal_seek: None,
                        foreach: None,
                        command: None,
                        handler: None,
                        capture: None,
                        capture_format: None,
                        capture_streams: Default::default(),
                        output_file: None,
                        timeout: None,
                        capture_output: CaptureOutput::Disabled,
                        on_failure: None,
                        retry: None,
                        on_success: None,
                        on_exit_code: Default::default(),
                        commit_required: false,
                        auto_commit: false,
                        commit_config: None,
                        working_dir: None,
                        env: Default::default(),
                        validate: None,
                        step_validate: None,
                        skip_validation: false,
                        validation_timeout: None,
                        ignore_validation_failure: false,
                        when: None,
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

    /// Get the handler strategy
    pub fn strategy(&self) -> HandlerStrategy {
        match self {
            OnFailureConfig::Detailed(config) => config.strategy.clone(),
            _ => HandlerStrategy::Recovery,
        }
    }

    /// Check if handler failure should be fatal
    pub fn handler_failure_fatal(&self) -> bool {
        match self {
            OnFailureConfig::Detailed(config) => config.handler_failure_fatal,
            _ => false,
        }
    }

    /// Get handler timeout in seconds
    pub fn handler_timeout(&self) -> Option<u64> {
        match self {
            OnFailureConfig::Detailed(config) => config.timeout,
            _ => None,
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

    #[test]
    fn test_single_command() {
        let yaml = r#""echo 'Handling error'""#;
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        let commands = config.handler_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].shell, Some("echo 'Handling error'".to_string()));
        assert!(commands[0].claude.is_none());
        assert!(!config.should_fail_workflow());
    }

    #[test]
    fn test_multiple_commands() {
        let yaml = r#"
- "npm cache clean --force"
- "npm install"
- "/fix-errors"
"#;
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        let commands = config.handler_commands();
        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands[0].shell,
            Some("npm cache clean --force".to_string())
        );
        assert_eq!(commands[1].shell, Some("npm install".to_string()));
        assert_eq!(commands[2].claude, Some("/fix-errors".to_string()));
    }

    #[test]
    fn test_detailed_config() {
        let yaml = r#"
strategy: recovery
commands:
  - shell: "cleanup.sh"
    continue_on_error: true
  - claude: "/fix-issue"
timeout: 300
fail_workflow: false
handler_failure_fatal: true
"#;
        let config: OnFailureConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.strategy(), HandlerStrategy::Recovery);
        assert_eq!(config.handler_timeout(), Some(300));
        assert!(config.handler_failure_fatal());
        assert!(!config.should_fail_workflow());

        let commands = config.handler_commands();
        assert_eq!(commands.len(), 2);
        assert!(commands[0].continue_on_error);
    }
}
