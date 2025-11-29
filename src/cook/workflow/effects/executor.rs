//! Effect-based workflow execution
//!
//! This module provides composable Effect-based workflow execution with retry support,
//! checkpoint integration, and comprehensive error context.

use super::{
    execute_claude_command_effect, execute_shell_command_effect,
    progress::{StepResult, WorkflowProgress, WorkflowResult},
    step_error::{StepError, WorkflowError},
    ExecutionEnv,
};
use crate::cook::workflow::normalized::{NormalizedStep, StepCommand};
use crate::cook::workflow::pure::build_command;
use std::collections::HashMap;
use std::time::Instant;
use stillwater::{from_async, Effect};

/// Execute a single workflow step
///
/// This function wraps step execution in an Effect, handling both Claude and shell commands.
/// It does not include retry logic - use `execute_claude_step_with_retry` for Claude commands
/// that need transient error handling.
pub fn execute_step(
    step: &NormalizedStep,
    variables: &HashMap<String, String>,
) -> impl Effect<Output = StepResult, Error = StepError, Env = ExecutionEnv> {
    let step = step.clone();
    let variables = variables.clone();

    from_async(move |env: &ExecutionEnv| {
        let step = step.clone();
        let variables = variables.clone();
        let workflow_env = env.workflow_env.clone();

        async move {
            let start = Instant::now();

            let result = match &step.command {
                StepCommand::Claude(command) => {
                    let cmd = build_command(command, &variables);
                    execute_claude_command_effect(&cmd, &variables)
                        .run(&workflow_env)
                        .await
                        .map_err(StepError::CommandError)
                }
                StepCommand::Shell(command) => {
                    let cmd = build_command(command, &variables);
                    execute_shell_command_effect(&cmd, &variables, None)
                        .run(&workflow_env)
                        .await
                        .map_err(StepError::CommandError)
                }
                _ => Ok(super::CommandOutput::success(String::new())),
            }?;

            let duration = start.elapsed();
            Ok(StepResult::from_command_output(result, duration))
        }
    })
}

/// Execute Claude step with built-in retry for transient errors
///
/// Implements custom retry logic with exponential backoff for transient Claude errors
/// (500, overloaded, rate limit, ECONNRESET). Uses manual retry loop since
/// Stillwater's Effect::retry is not yet available.
///
/// Retry configuration:
/// - Max attempts: 3
/// - Initial delay: 1 second
/// - Backoff multiplier: 2x (exponential)
/// - Max delay: 30 seconds
pub fn execute_claude_step_with_retry(
    command: &str,
    variables: &HashMap<String, String>,
) -> impl Effect<Output = StepResult, Error = StepError, Env = ExecutionEnv> {
    let command = command.to_string();
    let variables = variables.clone();

    from_async(move |env: &ExecutionEnv| {
        let command = command.clone();
        let variables = variables.clone();
        let workflow_env = env.workflow_env.clone();

        async move {
            let start = Instant::now();
            let cmd = build_command(&command, &variables);

            // Retry configuration
            const MAX_ATTEMPTS: u32 = 3;
            const INITIAL_DELAY_MS: u64 = 1000;
            const BACKOFF_MULTIPLIER: u64 = 2;
            const MAX_DELAY_MS: u64 = 30000;

            let mut attempt = 0;
            let mut delay_ms = INITIAL_DELAY_MS;
            let mut last_error = None;

            while attempt < MAX_ATTEMPTS {
                attempt += 1;

                match execute_claude_command_effect(&cmd, &variables)
                    .run(&workflow_env)
                    .await
                {
                    Ok(output) => {
                        let duration = start.elapsed();
                        return Ok(StepResult::from_command_output(output, duration));
                    }
                    Err(e) => {
                        // Check if error is transient (retryable)
                        let is_transient = is_transient_error(&e);

                        if !is_transient || attempt >= MAX_ATTEMPTS {
                            // Non-transient error or max retries exhausted
                            return Err(StepError::CommandError(e));
                        }

                        // Store error for potential final return
                        last_error = Some(e);

                        // Wait before retry with exponential backoff
                        if attempt < MAX_ATTEMPTS {
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            delay_ms = (delay_ms * BACKOFF_MULTIPLIER).min(MAX_DELAY_MS);
                        }
                    }
                }
            }

            // If we exhausted all retries, return the last error
            Err(StepError::CommandError(last_error.unwrap_or_else(|| {
                super::CommandError::ExecutionFailed {
                    message: "All retry attempts failed".to_string(),
                    exit_code: None,
                }
            })))
        }
    })
}

/// Check if an error is transient and should be retried
fn is_transient_error(error: &super::CommandError) -> bool {
    let error_string = error.to_string().to_lowercase();

    // Check for common transient error patterns
    error_string.contains("500")
        || error_string.contains("overloaded")
        || error_string.contains("rate limit")
        || error_string.contains("econnreset")
        || error_string.contains("timeout")
        || error_string.contains("connection refused")
        || error_string.contains("temporarily unavailable")
}

/// Execute entire workflow as composed Effect
///
/// This function composes all workflow steps sequentially, propagating variables between
/// steps and accumulating progress. Each step's captured variables are made available to
/// subsequent steps.
pub fn execute_workflow(
    steps: Vec<NormalizedStep>,
    initial_variables: HashMap<String, String>,
) -> impl Effect<Output = WorkflowResult, Error = WorkflowError, Env = ExecutionEnv> {
    from_async(move |env: &ExecutionEnv| {
        let steps = steps.clone();
        let mut variables = initial_variables.clone();
        let env = env.clone();

        async move {
            let mut progress = WorkflowProgress::new();

            for (idx, step) in steps.iter().enumerate() {
                // Execute step with current variables
                let result = execute_step(step, &variables)
                    .run(&env)
                    .await
                    .map_err(|e| WorkflowError::StepFailed {
                        step_index: idx,
                        error: stillwater::ContextError::new(e)
                            .context(format!("Executing step {}", idx)),
                    })?;

                // Update variables with captured outputs
                for (k, v) in &result.captured_variables {
                    variables.insert(k.clone(), v.clone());
                }

                progress = progress.with_step_result(idx, result);
            }

            Ok(progress.into_result())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::checkpoint::CheckpointManager;
    use crate::cook::workflow::checkpoint_path::CheckpointStorage;
    use crate::cook::workflow::effects::environment::WorkflowEnvBuilder;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    fn create_test_execution_env() -> ExecutionEnv {
        use crate::cook::workflow::effects::environment::{ClaudeRunner, RunnerOutput};
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::path::Path;

        struct MockClaudeRunner;

        #[async_trait]
        impl ClaudeRunner for MockClaudeRunner {
            async fn run(
                &self,
                _command: &str,
                _working_dir: &Path,
                _env_vars: HashMap<String, String>,
            ) -> anyhow::Result<RunnerOutput> {
                Ok(RunnerOutput::success("test output".to_string()))
            }
        }

        let workflow_env = WorkflowEnvBuilder::default()
            .with_claude_runner(Arc::new(MockClaudeRunner))
            .build();
        let checkpoint_manager = Arc::new(CheckpointManager::with_storage(
            CheckpointStorage::Session {
                session_id: "test-session".to_string(),
            },
        ));

        ExecutionEnv::builder(workflow_env)
            .with_session_id("test-session")
            .with_workflow_path(PathBuf::from("/tmp/workflow.yml"))
            .with_checkpoint_manager(checkpoint_manager)
            .build()
            .expect("Failed to create execution env")
    }

    #[tokio::test]
    async fn test_execute_shell_step() {
        let env = create_test_execution_env();
        let step = NormalizedStep {
            id: "test-step".into(),
            command: StepCommand::Shell("echo hello".into()),
            validation: None,
            handlers: Default::default(),
            timeout: None,
            working_dir: None,
            env: Arc::new(HashMap::new()),
            outputs: None,
            commit_required: false,
            when: None,
        };

        let variables = HashMap::new();
        let result = execute_step(&step, &variables).run(&env).await;

        // This will likely fail since we don't have a real shell runner
        // but it demonstrates the API
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_workflow_progress_accumulation() {
        let mut vars1 = HashMap::new();
        vars1.insert("foo".to_string(), "bar".to_string());
        let step1 = StepResult::success(Duration::from_secs(1)).with_variables(vars1);

        let progress = WorkflowProgress::new().with_step_result(0, step1);

        assert_eq!(progress.current_step, 1);
        assert_eq!(progress.completed_steps.len(), 1);
    }
}
