//! Setup phase executor for MapReduce workflows
//!
//! Handles execution of setup commands with output capture and timeout management.

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
    /// Variables to capture from setup commands
    capture_outputs: HashMap<String, usize>,
}

impl SetupPhaseExecutor {
    /// Create a new setup phase executor
    pub fn new(setup_phase: &SetupPhase) -> Self {
        Self {
            timeout: Duration::from_secs(setup_phase.timeout),
            capture_outputs: setup_phase.capture_outputs.clone(),
        }
    }

    /// Execute the setup phase
    pub async fn execute<E>(
        &self,
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

                // Check if we need to capture output from this command
                for (var_name, cmd_index) in &self.capture_outputs {
                    if *cmd_index == index {
                        info!(
                            "Capturing output from command {} as variable {}",
                            index, var_name
                        );

                        // Capture the stdout as the variable value
                        let output = step_result.stdout.trim().to_string();
                        captured_outputs.insert(var_name.clone(), output.clone());

                        // Also add to workflow context for immediate use
                        context.variables.insert(var_name.clone(), output);
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
        &self,
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

        info!("Setup phase executing in directory: {}", working_dir.display());

        // Track files before setup to detect created files
        let files_before_setup = std::fs::read_dir(working_dir)
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
        let files_after_setup = std::fs::read_dir(working_dir)
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
        let mut capture_outputs = HashMap::new();
        capture_outputs.insert("INPUT_FILE".to_string(), 0);
        capture_outputs.insert("ITEM_COUNT".to_string(), 1);

        let setup_phase = SetupPhase {
            commands: vec![WorkflowStep::default(), WorkflowStep::default()],
            timeout: 60,
            capture_outputs,
        };

        let executor_impl = SetupPhaseExecutor::new(&setup_phase);

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
            working_dir: PathBuf::from("."),
            project_dir: PathBuf::from("."),
            session_id: "test-session".to_string(),
            worktree_name: Some("test-worktree".to_string()),
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

        let executor_impl = SetupPhaseExecutor::new(&setup_phase);

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
            working_dir: PathBuf::from("."),
            project_dir: PathBuf::from("."),
            session_id: "test-session".to_string(),
            worktree_name: Some("test-worktree".to_string()),
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
