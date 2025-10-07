//! Workflow normalization and command conversion
//!
//! Pure functions for converting commands to workflow steps and normalizing configuration.

use crate::config::command::{TestCommand, WorkflowCommand, WorkflowStepCommand};
use crate::cook::workflow::{CaptureOutput, OnFailureConfig, WorkflowStep};

/// Convert a WorkflowCommand to a WorkflowStep
pub(super) fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
    match cmd {
        WorkflowCommand::WorkflowStep(step) => {
            // Handle new workflow step format directly
            // For shell commands with on_failure (retry logic), convert to test format
            let (shell, test, on_failure) = process_step_failure_config(step);

            WorkflowStep {
                name: None,
                command: None,
                claude: step.claude.clone(),
                shell,
                test, // Contains retry logic for shell commands
                goal_seek: step.goal_seek.clone(),
                foreach: step.foreach.clone(),
                handler: None,
                capture: None,
                auto_commit: false,
                commit_config: None,
                capture_format: step.capture_format.as_ref().and_then(|f| match f.as_str() {
                    "json" => Some(crate::cook::workflow::variables::CaptureFormat::Json),
                    "lines" => Some(crate::cook::workflow::variables::CaptureFormat::Lines),
                    "string" => Some(crate::cook::workflow::variables::CaptureFormat::String),
                    "number" => Some(crate::cook::workflow::variables::CaptureFormat::Number),
                    "boolean" => Some(crate::cook::workflow::variables::CaptureFormat::Boolean),
                    _ => None,
                }),
                capture_streams: match step.capture_streams.as_deref() {
                    Some("stdout") => crate::cook::workflow::variables::CaptureStreams {
                        stdout: true,
                        stderr: false,
                        exit_code: true,
                        success: true,
                        duration: true,
                    },
                    Some("stderr") => crate::cook::workflow::variables::CaptureStreams {
                        stdout: false,
                        stderr: true,
                        exit_code: true,
                        success: true,
                        duration: true,
                    },
                    Some("both") => crate::cook::workflow::variables::CaptureStreams {
                        stdout: true,
                        stderr: true,
                        exit_code: true,
                        success: true,
                        duration: true,
                    },
                    _ => crate::cook::workflow::variables::CaptureStreams::default(),
                },
                output_file: step.output_file.as_ref().map(std::path::PathBuf::from),
                capture_output: match &step.capture_output {
                    Some(crate::config::command::CaptureOutputConfig::Boolean(true)) => {
                        CaptureOutput::Default
                    }
                    Some(crate::config::command::CaptureOutputConfig::Boolean(false)) => {
                        CaptureOutput::Disabled
                    }
                    Some(crate::config::command::CaptureOutputConfig::Variable(var)) => {
                        CaptureOutput::Variable(var.clone())
                    }
                    None => CaptureOutput::Disabled,
                },
                timeout: None,
                working_dir: None,
                env: std::collections::HashMap::new(),
                on_failure,
                retry: None,
                on_success: None,
                on_exit_code: std::collections::HashMap::new(),
                // Commands don't require commits by default unless explicitly set
                commit_required: step.commit_required,
                validate: step.validate.clone(),
                step_validate: None,
                skip_validation: false,
                validation_timeout: None,
                ignore_validation_failure: false,
                when: None,
            }
        }
        _ => {
            // Convert to command and apply defaults to get proper commit_required
            let mut command = cmd.to_command();
            crate::config::apply_command_defaults(&mut command);

            let command_str = command.name.clone();
            let commit_required = determine_commit_required(cmd, &command);

            WorkflowStep {
                name: None,
                command: Some(if command_str.starts_with('/') {
                    command_str
                } else {
                    format!("/{command_str}")
                }),
                claude: None,
                shell: None,
                test: None,
                goal_seek: None,
                foreach: None,
                handler: None,
                capture: None,
                auto_commit: false,
                commit_config: None,
                capture_format: None,
                capture_streams: Default::default(),
                output_file: None,
                capture_output: CaptureOutput::Disabled,
                timeout: None,
                working_dir: None,
                env: std::collections::HashMap::new(),
                on_failure: None,
                retry: None,
                on_success: None,
                on_exit_code: std::collections::HashMap::new(),
                commit_required,
                validate: None,
                step_validate: None,
                skip_validation: false,
                validation_timeout: None,
                ignore_validation_failure: false,
                when: None,
            }
        }
    }
}

/// Process step failure configuration to extract shell, test, and on_failure fields
pub(super) fn process_step_failure_config(
    step: &WorkflowStepCommand,
) -> (Option<String>, Option<TestCommand>, Option<OnFailureConfig>) {
    if step.shell.is_some() && step.on_failure.is_some() {
        // Convert shell command with on_failure to test command for retry logic
        // Safe to use unwrap here as we just checked is_some() above
        let test_cmd = step.shell.as_ref().map(|shell_cmd| TestCommand {
            command: shell_cmd.clone(),
            on_failure: step.on_failure.clone(),
        });
        // Clear shell field when converting to test
        (None, test_cmd, None)
    } else if step.on_failure.is_some() {
        // For non-shell commands, convert TestDebugConfig to OnFailureConfig
        let on_failure = step.on_failure.as_ref().map(|debug_config| {
            // Use Advanced config with claude command
            OnFailureConfig::Advanced {
                shell: None,
                claude: Some(debug_config.claude.clone()),
                fail_workflow: debug_config.fail_workflow,
                retry_original: false,
                max_retries: debug_config.max_attempts - 1, // max_attempts includes first try
            }
        });
        (step.shell.clone(), step.test.clone(), on_failure)
    } else {
        (step.shell.clone(), step.test.clone(), None)
    }
}

/// Determine if a command requires a commit (delegates to workflow_classifier)
fn determine_commit_required(
    cmd: &WorkflowCommand,
    command: &crate::config::command::Command,
) -> bool {
    super::workflow_classifier::determine_commit_required(cmd, command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::{CaptureOutputConfig, TestDebugConfig};

    fn empty_workflow_step() -> WorkflowStepCommand {
        WorkflowStepCommand {
            claude: None,
            shell: None,
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            commit_required: false,
            analysis: None,
            outputs: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            validate: None,
            timeout: None,
            when: None,
            capture_format: None,
            capture_streams: None,
            output_file: None,
        }
    }

    #[test]
    fn test_process_step_failure_config_shell_with_on_failure() {
        let step = WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            on_failure: Some(TestDebugConfig {
                claude: "/debug".to_string(),
                max_attempts: 3,
                fail_workflow: false,
                commit_required: true,
            }),
            ..empty_workflow_step()
        };

        let (shell, test, on_failure) = process_step_failure_config(&step);

        // Shell should be None (converted to test)
        assert!(shell.is_none());
        // Test should contain the shell command
        assert!(test.is_some());
        assert_eq!(test.unwrap().command, "echo test");
        // on_failure should be None (moved to test)
        assert!(on_failure.is_none());
    }

    #[test]
    fn test_process_step_failure_config_non_shell_with_on_failure() {
        let step = WorkflowStepCommand {
            claude: Some("/test".to_string()),
            on_failure: Some(TestDebugConfig {
                claude: "/debug".to_string(),
                max_attempts: 3,
                fail_workflow: true,
                commit_required: true,
            }),
            ..empty_workflow_step()
        };

        let (shell, test, on_failure) = process_step_failure_config(&step);

        assert!(shell.is_none());
        assert!(test.is_none());
        assert!(on_failure.is_some());

        if let Some(OnFailureConfig::Advanced {
            claude,
            fail_workflow,
            max_retries,
            ..
        }) = on_failure
        {
            assert_eq!(claude, Some("/debug".to_string()));
            assert!(fail_workflow);
            assert_eq!(max_retries, 2); // max_attempts - 1
        } else {
            panic!("Expected Advanced on_failure config");
        }
    }

    #[test]
    fn test_process_step_failure_config_no_on_failure() {
        let step = WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            on_failure: None,
            ..empty_workflow_step()
        };

        let (shell, test, on_failure) = process_step_failure_config(&step);

        assert_eq!(shell, Some("echo test".to_string()));
        assert!(test.is_none());
        assert!(on_failure.is_none());
    }

    #[test]
    fn test_convert_command_to_step_simple_command() {
        let cmd = WorkflowCommand::Simple("test".to_string());

        let step = convert_command_to_step(&cmd);

        assert_eq!(step.command, Some("/test".to_string()));
        assert!(step.shell.is_none());
        assert!(step.claude.is_none());
    }

    #[test]
    fn test_convert_command_to_step_workflow_step() {
        let cmd = WorkflowCommand::WorkflowStep(Box::new(WorkflowStepCommand {
            claude: Some("/lint".to_string()),
            commit_required: true,
            capture_output: Some(CaptureOutputConfig::Variable("result".to_string())),
            ..empty_workflow_step()
        }));

        let step = convert_command_to_step(&cmd);

        assert_eq!(step.claude, Some("/lint".to_string()));
        assert!(step.commit_required);
        assert!(matches!(step.capture_output, CaptureOutput::Variable(v) if v == "result"));
    }

    #[test]
    fn test_convert_command_to_step_capture_streams() {
        let cmd = WorkflowCommand::WorkflowStep(Box::new(WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            capture_streams: Some("both".to_string()),
            ..empty_workflow_step()
        }));

        let step = convert_command_to_step(&cmd);

        assert!(step.capture_streams.stdout);
        assert!(step.capture_streams.stderr);
        assert!(step.capture_streams.exit_code);
    }
}
