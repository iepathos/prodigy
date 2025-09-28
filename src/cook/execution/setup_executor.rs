//! Setup phase executor for MapReduce workflows
//!
//! Handles execution of setup commands with output capture and timeout management.

use crate::cook::execution::variable_capture::{CommandResult, VariableCaptureEngine};
use crate::cook::execution::SetupPhase;
use crate::cook::workflow::{WorkflowContext, WorkflowStep};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout as tokio_timeout;
use tracing::{debug, info, warn};

/// Executor for the setup phase of MapReduce workflows
pub struct SetupPhaseExecutor {
    /// Timeout for the entire setup phase
    timeout: Duration,
    /// Variable capture engine
    capture_engine: Option<VariableCaptureEngine>,
}

impl SetupPhaseExecutor {
    /// Create a new setup phase executor
    pub fn new(setup_phase: &SetupPhase) -> Self {
        // Use the capture_outputs directly as they are now CaptureConfig
        let capture_engine = if !setup_phase.capture_outputs.is_empty() {
            Some(VariableCaptureEngine::new(
                setup_phase.capture_outputs.clone(),
            ))
        } else {
            None
        };

        Self {
            timeout: Duration::from_secs(setup_phase.timeout),
            capture_engine,
        }
    }

    /// Execute the setup phase
    pub async fn execute<E>(
        &mut self,
        commands: &[WorkflowStep],
        executor: &mut E,
        env: &crate::cook::orchestrator::ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<HashMap<String, String>>
    where
        E: crate::cook::workflow::StepExecutor,
    {
        let start_time = Instant::now();
        let mut captured_outputs = HashMap::new();

        // Execute setup commands with timeout
        let result = tokio_timeout(self.timeout, async {
            for (index, step) in commands.iter().enumerate() {
                debug!("Executing setup step {}/{}", index + 1, commands.len());

                // Execute the step
                let step_result = executor.execute_step(step, env, context).await?;

                // Capture variables if configured
                if let Some(ref mut engine) = self.capture_engine {
                    let cmd_result = CommandResult {
                        stdout: step_result.stdout.clone(),
                        stderr: step_result.stderr.clone(),
                        success: step_result.success,
                        exit_code: step_result.exit_code,
                    };

                    if let Err(e) = engine.capture_from_command(index, &cmd_result).await {
                        warn!("Failed to capture variables from command {}: {}", index, e);
                        // Continue execution even if capture fails
                    }

                    // Export captured variables to context
                    for (var_name, var_value) in engine.export_variables() {
                        captured_outputs.insert(var_name.clone(), var_value.clone());
                        context.variables.insert(var_name, var_value);
                    }
                }

                // Check if command failed
                if !step_result.success {
                    return Err(anyhow!(
                        "Setup command {} failed with exit code {:?}",
                        index + 1,
                        step_result.exit_code
                    ));
                }
            }

            Ok::<(), anyhow::Error>(())
        })
        .await;

        // Handle timeout
        match result {
            Ok(Ok(())) => {
                let elapsed = start_time.elapsed();
                info!("Setup phase completed in {:?}", elapsed);
                Ok(captured_outputs)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                warn!("Setup phase timed out after {:?}", self.timeout);
                Err(anyhow!(
                    "Setup phase timed out after {} seconds",
                    self.timeout.as_secs()
                ))
            }
        }
    }

    /// Execute setup phase with file detection
    /// Returns captured outputs and optionally a generated input file
    pub async fn execute_with_file_detection<E>(
        &mut self,
        commands: &[WorkflowStep],
        executor: &mut E,
        env: &crate::cook::orchestrator::ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<(HashMap<String, String>, Option<String>)>
    where
        E: crate::cook::workflow::StepExecutor,
    {
        // Use the working directory from the environment
        let working_dir = &env.working_dir;

        info!(
            "Setup phase executing in directory: {}",
            working_dir.display()
        );

        // Track files before setup to detect created files
        let files_before_setup = std::fs::read_dir(&**working_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();

        // Execute setup phase
        let captured_outputs = self.execute(commands, executor, env, context).await?;

        // Detect created files
        let files_after_setup = std::fs::read_dir(&**working_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();

        // Check if work-items.json was created
        let mut generated_input_file = None;
        for file in files_after_setup.difference(&files_before_setup) {
            if file.ends_with("work-items.json") || file == "work-items.json" {
                // Use full path for the generated file
                let full_path = working_dir.join(file);
                generated_input_file = Some(full_path.to_string_lossy().to_string());
                info!("Setup phase generated input file: {}", full_path.display());
                break;
            }
        }

        Ok((captured_outputs, generated_input_file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::SetupPhase;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::cook::workflow::{StepResult, WorkflowStep};
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct MockExecutor {
        results: Vec<StepResult>,
        call_count: usize,
    }

    #[async_trait]
    impl crate::cook::workflow::StepExecutor for MockExecutor {
        async fn execute_step(
            &mut self,
            _step: &WorkflowStep,
            _env: &ExecutionEnvironment,
            _context: &mut WorkflowContext,
        ) -> Result<StepResult> {
            if self.call_count >= self.results.len() {
                return Err(anyhow!("No more results"));
            }
            let result = self.results[self.call_count].clone();
            self.call_count += 1;
            Ok(result)
        }
    }

    #[tokio::test]
    async fn test_setup_executor_captures_output() {
        use crate::cook::execution::variable_capture::CaptureConfig;
        let mut capture_outputs = HashMap::new();
        capture_outputs.insert("INPUT_FILE".to_string(), CaptureConfig::Simple(0));
        capture_outputs.insert("ITEM_COUNT".to_string(), CaptureConfig::Simple(1));

        let setup_phase = SetupPhase {
            commands: vec![WorkflowStep::default(), WorkflowStep::default()],
            timeout: 60,
            capture_outputs,
        };

        let mut executor_impl = SetupPhaseExecutor::new(&setup_phase);

        let mut mock_executor = MockExecutor {
            results: vec![
                StepResult {
                    success: true,
                    exit_code: Some(0),
                    stdout: "items.json".to_string(),
                    stderr: String::new(),
                },
                StepResult {
                    success: true,
                    exit_code: Some(0),
                    stdout: "42".to_string(),
                    stderr: String::new(),
                },
            ],
            call_count: 0,
        };

        let env = ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from(".")),
            project_dir: Arc::new(PathBuf::from(".")),
            session_id: Arc::from("test-session"),
            worktree_name: Some(Arc::from("test-worktree")),
        };

        let mut context = WorkflowContext::default();

        let captured = executor_impl
            .execute(
                &setup_phase.commands,
                &mut mock_executor,
                &env,
                &mut context,
            )
            .await
            .unwrap();

        assert_eq!(captured.get("INPUT_FILE").unwrap(), "items.json");
        assert_eq!(captured.get("ITEM_COUNT").unwrap(), "42");
        assert_eq!(context.variables.get("INPUT_FILE").unwrap(), "items.json");
        assert_eq!(context.variables.get("ITEM_COUNT").unwrap(), "42");
    }

    #[tokio::test]
    async fn test_setup_executor_timeout() {
        let setup_phase = SetupPhase {
            commands: vec![WorkflowStep::default()],
            timeout: 0, // Immediate timeout
            capture_outputs: HashMap::new(),
        };

        let mut executor_impl = SetupPhaseExecutor::new(&setup_phase);

        // Create a mock executor that takes too long
        struct SlowExecutor;

        #[async_trait]
        impl crate::cook::workflow::StepExecutor for SlowExecutor {
            async fn execute_step(
                &mut self,
                _step: &WorkflowStep,
                _env: &ExecutionEnvironment,
                _context: &mut WorkflowContext,
            ) -> Result<StepResult> {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(StepResult::default())
            }
        }

        let mut slow_executor = SlowExecutor;
        let env = ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from(".")),
            project_dir: Arc::new(PathBuf::from(".")),
            session_id: Arc::from("test-session"),
            worktree_name: Some(Arc::from("test-worktree")),
        };
        let mut context = WorkflowContext::default();

        let result = executor_impl
            .execute(
                &setup_phase.commands,
                &mut slow_executor,
                &env,
                &mut context,
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
