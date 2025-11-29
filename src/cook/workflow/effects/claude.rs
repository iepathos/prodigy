//! Claude command execution effects
//!
//! This module provides Effect-based abstractions for executing Claude commands
//! using pure functions from the command_builder and output_parser modules.
//!
//! # Architecture
//!
//! - Pure: Command building and output parsing (from `pure/` module)
//! - I/O: Claude command execution (wrapped in Effect)
//! - Retry: Automatic retry for transient errors
//!
//! # Example
//!
//! ```ignore
//! use prodigy::cook::workflow::effects::claude::execute_claude_command_with_retry;
//! use prodigy::cook::workflow::effects::retry_helpers::default_claude_retry_policy;
//! use std::collections::HashMap;
//!
//! let variables = HashMap::new();
//! let retry_policy = default_claude_retry_policy();
//! let effect = execute_claude_command_with_retry("/task ${item}", &variables, retry_policy);
//! let result = effect.run(&env).await?;
//! ```

use super::claude_error::ClaudeError;
use super::environment::WorkflowEnv;
use super::{CommandError, CommandOutput};
use crate::cook::workflow::pure::{build_command, parse_output_variables};
use std::collections::HashMap;
use stillwater::effect::prelude::*;
use stillwater::retry::RetryPolicy;
use tracing::{info, warn};

/// Effect: Execute Claude command with variable expansion and output parsing
///
/// This effect composes pure functions (command building, output parsing) with
/// I/O operations (Claude command execution) using the Effect pattern.
///
/// # Arguments
///
/// * `template` - Command template with variable placeholders (e.g., "/task ${item}")
/// * `variables` - Map of variable names to values for expansion
///
/// # Returns
///
/// An Effect that, when run, will:
/// 1. Pure: Build the command string from the template
/// 2. I/O: Execute the Claude command
/// 3. Pure: Parse variables from the output
///
/// # Example
///
/// ```ignore
/// let mut vars = HashMap::new();
/// vars.insert("item".to_string(), "task-123".to_string());
///
/// let effect = execute_claude_command_effect("/process ${item}", &vars);
/// let output = effect.run(&env).await?;
/// ```
pub fn execute_claude_command_effect(
    template: &str,
    variables: &HashMap<String, String>,
) -> impl Effect<Output = CommandOutput, Error = CommandError, Env = WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    from_async(move |env: &WorkflowEnv| {
        let template = template.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let claude_runner = env.claude_runner.clone();
        let output_patterns = env.output_patterns.clone();
        let env_vars = env.env_vars.clone();

        async move {
            // Pure: Build command from template
            let command = build_command(&template, &variables);

            // I/O: Execute Claude command
            let output = claude_runner
                .run(&command, &working_dir, env_vars)
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
                json_log_location: output.json_log_location,
            })
        }
    })
}

/// Effect: Execute Claude command that may fail (returns result even on failure)
///
/// Similar to `execute_claude_command_effect` but returns the output even when
/// the command fails, allowing the caller to inspect the failure details.
///
/// # Arguments
///
/// * `template` - Command template with variable placeholders
/// * `variables` - Map of variable names to values for expansion
///
/// # Returns
///
/// An Effect that returns CommandOutput regardless of success/failure.
pub fn execute_claude_command_effect_fallible(
    template: &str,
    variables: &HashMap<String, String>,
) -> impl Effect<Output = CommandOutput, Error = CommandError, Env = WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    from_async(move |env: &WorkflowEnv| {
        let template = template.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let claude_runner = env.claude_runner.clone();
        let output_patterns = env.output_patterns.clone();
        let env_vars = env.env_vars.clone();

        async move {
            // Pure: Build command from template
            let command = build_command(&template, &variables);

            // I/O: Execute Claude command
            let output = claude_runner
                .run(&command, &working_dir, env_vars)
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
                json_log_location: output.json_log_location,
            })
        }
    })
}

/// Effect: Execute Claude command with retry for transient errors
///
/// This effect wraps Claude command execution with automatic retry logic for
/// transient errors (500, overload, timeouts) using Stillwater's Effect::retry_if.
///
/// # Arguments
///
/// * `template` - Command template with variable placeholders
/// * `variables` - Map of variable names to values for expansion
/// * `retry_policy` - Retry policy configuration (delays, max attempts, jitter)
///
/// # Returns
///
/// An Effect that retries transient errors and fails fast for permanent errors.
///
/// # Example
///
/// ```ignore
/// use prodigy::cook::workflow::effects::retry_helpers::default_claude_retry_policy;
///
/// let policy = default_claude_retry_policy();
/// let effect = execute_claude_command_with_retry("/task", &vars, policy);
/// let result = effect.run(&env).await?;
/// ```
pub fn execute_claude_command_with_retry(
    template: &str,
    variables: &HashMap<String, String>,
    retry_policy: RetryPolicy,
) -> impl Effect<Output = CommandOutput, Error = CommandError, Env = WorkflowEnv> {
    let template = template.to_string();
    let variables = variables.clone();

    retry_if(
        move || {
            let template = template.clone();
            let variables = variables.clone();
            execute_raw_claude(template, variables)
        },
        retry_policy.clone(),
        |error: &ClaudeError| {
            let is_transient = error.is_transient();
            if is_transient {
                warn!("Claude command failed with transient error, will retry: {}", error);
            } else {
                info!("Claude command failed with permanent error, not retrying: {}", error);
            }
            is_transient
        },
    )
    .map_err(|claude_error| {
        // Convert ClaudeError to CommandError
        warn!("Claude command failed: {}", claude_error);
        CommandError::ExecutionFailed {
            message: claude_error.to_string(),
            exit_code: None,
        }
    })
}

/// Execute raw Claude command (single attempt, no retry)
///
/// This is the inner function used by retry logic. It classifies errors into
/// ClaudeError types for transient error detection.
fn execute_raw_claude(
    template: String,
    variables: HashMap<String, String>,
) -> impl Effect<Output = CommandOutput, Error = ClaudeError, Env = WorkflowEnv> {
    from_async(move |env: &WorkflowEnv| {
        let template = template.clone();
        let variables = variables.clone();
        let working_dir = env.working_dir.clone();
        let claude_runner = env.claude_runner.clone();
        let output_patterns = env.output_patterns.clone();
        let env_vars = env.env_vars.clone();

        async move {
            // Pure: Build command from template
            let command = build_command(&template, &variables);

            // I/O: Execute Claude command
            let output = claude_runner
                .run(&command, &working_dir, env_vars)
                .await
                .map_err(|e| ClaudeError::ProcessError {
                    message: e.to_string(),
                })?;

            if !output.success {
                // Classify error based on stderr
                return Err(ClaudeError::from_stderr(&output.stderr));
            }

            // Pure: Parse variables from output
            let extracted_vars = parse_output_variables(&output.stdout, &output_patterns);

            Ok(CommandOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
                success: output.success,
                variables: extracted_vars,
                json_log_location: output.json_log_location,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::effects::environment::{ClaudeRunner, RunnerOutput};
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct MockClaudeRunner {
        responses: Arc<std::sync::Mutex<Vec<RunnerOutput>>>,
        calls: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl MockClaudeRunner {
        fn new() -> Self {
            Self {
                responses: Arc::new(std::sync::Mutex::new(Vec::new())),
                calls: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn add_response(&self, response: RunnerOutput) {
            self.responses.lock().unwrap().push(response);
        }

        fn get_calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ClaudeRunner for MockClaudeRunner {
        async fn run(
            &self,
            command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
        ) -> anyhow::Result<RunnerOutput> {
            self.calls.lock().unwrap().push(command.to_string());

            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
        }
    }

    struct MockShellRunner;

    #[async_trait]
    impl crate::cook::workflow::effects::environment::ShellRunner for MockShellRunner {
        async fn run(
            &self,
            _command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
            _timeout: Option<u64>,
        ) -> anyhow::Result<RunnerOutput> {
            Ok(RunnerOutput::success("shell output".to_string()))
        }
    }

    fn create_test_env(claude_runner: Arc<dyn ClaudeRunner>) -> WorkflowEnv {
        WorkflowEnv {
            claude_runner,
            shell_runner: Arc::new(MockShellRunner),
            output_patterns: Vec::new(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_success() {
        let mock_runner = Arc::new(MockClaudeRunner::new());
        mock_runner.add_response(RunnerOutput::success("task completed".to_string()));

        let env = create_test_env(mock_runner.clone());

        let mut vars = HashMap::new();
        vars.insert("item".to_string(), "test-item".to_string());

        let effect = execute_claude_command_effect("/process ${item}", &vars);
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert_eq!(output.stdout, "task completed");

        // Verify command was built correctly
        let calls = mock_runner.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "/process test-item");
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_failure() {
        let mock_runner = Arc::new(MockClaudeRunner::new());
        mock_runner.add_response(RunnerOutput::failure("command failed".to_string(), 1));

        let env = create_test_env(mock_runner);

        let vars = HashMap::new();
        let effect = execute_claude_command_effect("/failing-task", &vars);
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CommandError::ExecutionFailed { .. }));
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_with_output_patterns() {
        use crate::cook::workflow::pure::OutputPattern;
        use regex::Regex;

        let mock_runner = Arc::new(MockClaudeRunner::new());
        mock_runner.add_response(RunnerOutput::success("Result: success-123".to_string()));

        let env = WorkflowEnv {
            claude_runner: mock_runner,
            shell_runner: Arc::new(MockShellRunner),
            output_patterns: vec![OutputPattern::Regex {
                name: "result".to_string(),
                regex: Regex::new(r"Result: (\S+)").unwrap(),
            }],
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
        };

        let effect = execute_claude_command_effect("/task", &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(
            output.variables.get("result"),
            Some(&"success-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_fallible_returns_on_failure() {
        let mock_runner = Arc::new(MockClaudeRunner::new());
        mock_runner.add_response(RunnerOutput::failure("error message".to_string(), 1));

        let env = create_test_env(mock_runner);

        let effect = execute_claude_command_effect_fallible("/task", &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
        assert_eq!(output.stderr, "error message");
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_preserves_json_log_location() {
        let mock_runner = Arc::new(MockClaudeRunner::new());
        let mut response = RunnerOutput::success("output".to_string());
        response.json_log_location = Some("/tmp/log.json".to_string());
        mock_runner.add_response(response);

        let env = create_test_env(mock_runner);

        let effect = execute_claude_command_effect("/task", &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.json_log_location, Some("/tmp/log.json".to_string()));
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_multiple_variables() {
        let mock_runner = Arc::new(MockClaudeRunner::new());
        mock_runner.add_response(RunnerOutput::success("done".to_string()));

        let env = create_test_env(mock_runner.clone());

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("action".to_string(), "process".to_string());

        let effect = execute_claude_command_effect("/${action} --user ${name}", &vars);
        let result = effect.run(&env).await;

        assert!(result.is_ok());

        let calls = mock_runner.get_calls();
        assert_eq!(calls[0], "/process --user Alice");
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_empty_template() {
        let mock_runner = Arc::new(MockClaudeRunner::new());
        mock_runner.add_response(RunnerOutput::success("".to_string()));

        let env = create_test_env(mock_runner.clone());

        let effect = execute_claude_command_effect("", &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_ok());

        let calls = mock_runner.get_calls();
        assert_eq!(calls[0], "");
    }

    #[tokio::test]
    async fn test_execute_claude_command_effect_runner_error() {
        // Create a runner that always fails
        struct FailingRunner;

        #[async_trait]
        impl ClaudeRunner for FailingRunner {
            async fn run(
                &self,
                _command: &str,
                _working_dir: &Path,
                _env_vars: HashMap<String, String>,
            ) -> anyhow::Result<RunnerOutput> {
                Err(anyhow::anyhow!("Connection failed"))
            }
        }

        let env = WorkflowEnv {
            claude_runner: Arc::new(FailingRunner),
            shell_runner: Arc::new(MockShellRunner),
            output_patterns: Vec::new(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: HashMap::new(),
        };

        let effect = execute_claude_command_effect("/task", &HashMap::new());
        let result = effect.run(&env).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            CommandError::ExecutionFailed { message, .. } => {
                assert!(message.contains("Connection failed"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }
}
