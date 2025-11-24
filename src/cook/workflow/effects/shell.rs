//! Shell command execution effects
//!
//! This module provides Effect-based abstractions for executing shell commands
//! using pure functions from the command_builder and output_parser modules.
//!
//! # Architecture
//!
//! - Pure: Command building and output parsing (from `pure/` module)
//! - I/O: Shell command execution (wrapped in Effect)
//!
//! # Example
//!
//! ```ignore
//! use prodigy::cook::workflow::effects::shell::execute_shell_command_effect;
//! use std::collections::HashMap;
//!
//! let variables = HashMap::new();
//! let effect = execute_shell_command_effect("echo ${message}", &variables, None);
//! let result = effect.run(&env).await?;
//! ```

use super::environment::WorkflowEnv;
use super::{CommandError, CommandOutput};
use crate::cook::workflow::pure::{build_command, parse_output_variables};
use std::collections::HashMap;
use stillwater::Effect;

/// Effect: Execute shell command with variable expansion and output parsing
///
/// This effect composes pure functions (command building, output parsing) with
/// I/O operations (shell command execution) using the Effect pattern.
///
/// # Arguments
///
/// * `template` - Command template with variable placeholders (e.g., "echo ${msg}")
/// * `variables` - Map of variable names to values for expansion
/// * `timeout` - Optional timeout in seconds
///
/// # Returns
///
/// An Effect that, when run, will:
/// 1. Pure: Build the command string from the template
/// 2. I/O: Execute the shell command
/// 3. Pure: Parse variables from the output
///
/// # Example
///
/// ```ignore
/// let mut vars = HashMap::new();
/// vars.insert("path".to_string(), "/tmp/data".to_string());
///
/// let effect = execute_shell_command_effect("ls ${path}", &vars, Some(30));
/// let output = effect.run(&env).await?;
/// ```
pub fn execute_shell_command_effect(
    template: &str,
    variables: &HashMap<String, String>,
    timeout: Option<u64>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async(move |env: &WorkflowEnv| {
        let template = template.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let shell_runner = env.shell_runner.clone();
        let output_patterns = env.output_patterns.clone();
        let env_vars = env.env_vars.clone();

        async move {
            // Pure: Build command from template
            let command = build_command(&template, &variables);

            // I/O: Execute shell command
            let output = shell_runner
                .run(&command, &working_dir, env_vars, timeout)
                .await
                .map_err(|e| CommandError::ExecutionFailed {
                    message: e.to_string(),
                    exit_code: None,
                })?;

            if !output.success {
                return Err(CommandError::ExecutionFailed {
                    message: output.stderr.clone(),
                    exit_code: output.exit_code,
                });
            }

            // Pure: Parse variables from output
            let extracted_vars = parse_output_variables(&output.stdout, &output_patterns);

            Ok(CommandOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
                success: output.success,
                variables: extracted_vars,
                json_log_location: None,
            })
        }
    })
}

/// Effect: Execute shell command that may fail (returns result even on failure)
///
/// Similar to `execute_shell_command_effect` but returns the output even when
/// the command fails, allowing the caller to inspect the failure details.
///
/// # Arguments
///
/// * `template` - Command template with variable placeholders
/// * `variables` - Map of variable names to values for expansion
/// * `timeout` - Optional timeout in seconds
///
/// # Returns
///
/// An Effect that returns CommandOutput regardless of success/failure.
pub fn execute_shell_command_effect_fallible(
    template: &str,
    variables: &HashMap<String, String>,
    timeout: Option<u64>,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async(move |env: &WorkflowEnv| {
        let template = template.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let shell_runner = env.shell_runner.clone();
        let output_patterns = env.output_patterns.clone();
        let env_vars = env.env_vars.clone();

        async move {
            // Pure: Build command from template
            let command = build_command(&template, &variables);

            // I/O: Execute shell command
            let output = shell_runner
                .run(&command, &working_dir, env_vars, timeout)
                .await
                .map_err(|e| CommandError::ExecutionFailed {
                    message: e.to_string(),
                    exit_code: None,
                })?;

            // Pure: Parse variables from output (even on failure)
            let extracted_vars = parse_output_variables(&output.stdout, &output_patterns);

            Ok(CommandOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
                success: output.success,
                variables: extracted_vars,
                json_log_location: None,
            })
        }
    })
}

/// Effect: Execute shell command with timeout that returns timeout error
///
/// This variant explicitly handles timeout as an error type rather than
/// returning a failed CommandOutput.
pub fn execute_shell_command_effect_with_timeout_error(
    template: &str,
    variables: &HashMap<String, String>,
    timeout_secs: u64,
) -> Effect<CommandOutput, CommandError, WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    Effect::from_async(move |env: &WorkflowEnv| {
        let template = template.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let shell_runner = env.shell_runner.clone();
        let output_patterns = env.output_patterns.clone();
        let env_vars = env.env_vars.clone();

        async move {
            // Pure: Build command from template
            let command = build_command(&template, &variables);

            // I/O: Execute shell command
            let output = shell_runner
                .run(&command, &working_dir, env_vars, Some(timeout_secs))
                .await
                .map_err(|e| CommandError::ExecutionFailed {
                    message: e.to_string(),
                    exit_code: None,
                })?;

            // Check for timeout (indicated by exit code -1 and specific stderr)
            if output.exit_code == Some(-1) && output.stderr.contains("timed out") {
                return Err(CommandError::Timeout {
                    seconds: timeout_secs,
                });
            }

            if !output.success {
                return Err(CommandError::ExecutionFailed {
                    message: output.stderr.clone(),
                    exit_code: output.exit_code,
                });
            }

            // Pure: Parse variables from output
            let extracted_vars = parse_output_variables(&output.stdout, &output_patterns);

            Ok(CommandOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
                success: output.success,
                variables: extracted_vars,
                json_log_location: None,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::effects::environment::{ClaudeRunner, RunnerOutput, ShellRunner};
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    type ShellCallRecord = (String, Option<u64>);

    struct MockShellRunner {
        responses: Arc<std::sync::Mutex<Vec<RunnerOutput>>>,
        calls: Arc<std::sync::Mutex<Vec<ShellCallRecord>>>,
    }

    impl MockShellRunner {
        fn new() -> Self {
            Self {
                responses: Arc::new(std::sync::Mutex::new(Vec::new())),
                calls: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn add_response(&self, response: RunnerOutput) {
            self.responses.lock().unwrap().push(response);
        }

        fn get_calls(&self) -> Vec<(String, Option<u64>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ShellRunner for MockShellRunner {
        async fn run(
            &self,
            command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
            timeout: Option<u64>,
        ) -> anyhow::Result<RunnerOutput> {
            self.calls
                .lock()
                .unwrap()
                .push((command.to_string(), timeout));

            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
        }
    }

    struct MockClaudeRunner;

    #[async_trait]
    impl ClaudeRunner for MockClaudeRunner {
        async fn run(
            &self,
            _command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
        ) -> anyhow::Result<RunnerOutput> {
            Ok(RunnerOutput::success("claude output".to_string()))
        }
    }

    fn create_test_env(shell_runner: Arc<dyn ShellRunner>) -> WorkflowEnv {
        WorkflowEnv {
            claude_runner: Arc::new(MockClaudeRunner),
            shell_runner,
            output_patterns: Vec::new(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_success() {
        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::success("file1\nfile2\n".to_string()));

        let env = create_test_env(mock_runner.clone());

        let mut vars = HashMap::new();
        vars.insert("dir".to_string(), "/home/user".to_string());

        let effect = execute_shell_command_effect("ls ${dir}", &vars, None);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("file1"));

        // Verify command was built correctly
        let calls = mock_runner.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "ls /home/user");
        assert_eq!(calls[0].1, None);
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_with_timeout() {
        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::success("done".to_string()));

        let env = create_test_env(mock_runner.clone());

        let effect = execute_shell_command_effect("long-running-cmd", &HashMap::new(), Some(30));
        let result = effect.run(&env).await;

        assert!(result.is_ok());

        let calls = mock_runner.get_calls();
        assert_eq!(calls[0].1, Some(30));
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_failure() {
        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::failure("command not found".to_string(), 127));

        let env = create_test_env(mock_runner);

        let effect = execute_shell_command_effect("nonexistent-cmd", &HashMap::new(), None);
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            CommandError::ExecutionFailed {
                message, exit_code, ..
            } => {
                assert!(message.contains("command not found"));
                assert_eq!(exit_code, Some(127));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_fallible_returns_on_failure() {
        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::failure("error output".to_string(), 1));

        let env = create_test_env(mock_runner);

        let effect = execute_shell_command_effect_fallible("failing-cmd", &HashMap::new(), None);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
        assert_eq!(output.exit_code, Some(1));
        assert_eq!(output.stderr, "error output");
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_with_output_patterns() {
        use crate::cook::workflow::pure::OutputPattern;
        use regex::Regex;

        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::success("Version: 1.2.3".to_string()));

        let env = WorkflowEnv {
            claude_runner: Arc::new(MockClaudeRunner),
            shell_runner: mock_runner,
            output_patterns: vec![OutputPattern::Regex {
                name: "version".to_string(),
                regex: Regex::new(r"Version: (\S+)").unwrap(),
            }],
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
        };

        let effect = execute_shell_command_effect("get-version", &HashMap::new(), None);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.variables.get("version"), Some(&"1.2.3".to_string()));
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_with_timeout_error() {
        let mock_runner = Arc::new(MockShellRunner::new());
        let timeout_response = RunnerOutput {
            stdout: String::new(),
            stderr: "Command timed out after 5 seconds".to_string(),
            exit_code: Some(-1),
            success: false,
            json_log_location: None,
        };
        mock_runner.add_response(timeout_response);

        let env = create_test_env(mock_runner);

        let effect =
            execute_shell_command_effect_with_timeout_error("sleep 100", &HashMap::new(), 5);
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            CommandError::Timeout { seconds } => {
                assert_eq!(seconds, 5);
            }
            _ => panic!("Expected Timeout error, got {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_multiple_variables() {
        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::success("copied".to_string()));

        let env = create_test_env(mock_runner.clone());

        let mut vars = HashMap::new();
        vars.insert("src".to_string(), "/source/file".to_string());
        vars.insert("dest".to_string(), "/destination/".to_string());

        let effect = execute_shell_command_effect("cp ${src} ${dest}", &vars, None);
        let result = effect.run(&env).await;

        assert!(result.is_ok());

        let calls = mock_runner.get_calls();
        assert_eq!(calls[0].0, "cp /source/file /destination/");
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_runner_error() {
        struct FailingRunner;

        #[async_trait]
        impl ShellRunner for FailingRunner {
            async fn run(
                &self,
                _command: &str,
                _working_dir: &Path,
                _env_vars: HashMap<String, String>,
                _timeout: Option<u64>,
            ) -> anyhow::Result<RunnerOutput> {
                Err(anyhow::anyhow!("Failed to spawn process"))
            }
        }

        let env = WorkflowEnv {
            claude_runner: Arc::new(MockClaudeRunner),
            shell_runner: Arc::new(FailingRunner),
            output_patterns: Vec::new(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
        };

        let effect = execute_shell_command_effect("cmd", &HashMap::new(), None);
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            CommandError::ExecutionFailed { message, .. } => {
                assert!(message.contains("Failed to spawn process"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }

    #[tokio::test]
    async fn test_execute_shell_command_effect_empty_output() {
        let mock_runner = Arc::new(MockShellRunner::new());
        mock_runner.add_response(RunnerOutput::success(String::new()));

        let env = create_test_env(mock_runner);

        let effect = execute_shell_command_effect("true", &HashMap::new(), None);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.is_empty());
    }
}
