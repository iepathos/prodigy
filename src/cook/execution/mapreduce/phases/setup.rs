//! Setup phase executor for MapReduce workflows
//!
//! This module handles the execution of setup commands that prepare
//! the environment and generate work items for the map phase.

use super::{PhaseContext, PhaseError, PhaseExecutor, PhaseMetrics, PhaseResult, PhaseType};
use crate::cook::execution::SetupPhase;
use crate::cook::workflow::WorkflowStep;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};

/// Executor for the setup phase of MapReduce workflows
pub struct SetupPhaseExecutor {
    /// The setup phase configuration
    setup_phase: SetupPhase,
}

impl SetupPhaseExecutor {
    /// Create a new setup phase executor
    pub fn new(setup_phase: SetupPhase) -> Self {
        Self { setup_phase }
    }

    /// Execute setup commands and capture outputs
    async fn execute_setup_commands(
        &self,
        commands: &[WorkflowStep],
        context: &mut PhaseContext,
    ) -> Result<HashMap<String, String>, PhaseError> {
        let mut captured_outputs = HashMap::new();

        for (index, step) in commands.iter().enumerate() {
            debug!("Executing setup step {}/{}", index + 1, commands.len());

            // Execute the step using the subprocess manager
            let result = self.execute_step(step, context).await.map_err(|e| {
                PhaseError::ExecutionFailed {
                    message: format!("Setup step {} failed: {}", index + 1, e),
                }
            })?;

            // Check if this step's output should be captured
            if let Some(&capture_index) = self
                .setup_phase
                .capture_outputs
                .get(&format!("step_{}", index))
            {
                if capture_index == index {
                    captured_outputs.insert(format!("setup_output_{}", index), result.clone());
                }
            }

            // Make the output available for subsequent steps
            context.variables.insert("shell.output".to_string(), result);
        }

        Ok(captured_outputs)
    }

    /// Execute a single setup step
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        context: &mut PhaseContext,
    ) -> Result<String, PhaseError> {
        // For now, we'll use a simplified execution model
        // In the full implementation, this would delegate to the appropriate executor
        if let Some(cmd) = &step.shell {
            // Execute shell command using subprocess manager
            use crate::subprocess::ProcessCommandBuilder;
            let command = ProcessCommandBuilder::new("sh")
                .args(&["-c", cmd])
                .current_dir(&context.environment.working_dir)
                .build();

            let result = context
                .subprocess_manager
                .runner()
                .run(command)
                .await
                .map_err(|e| PhaseError::ExecutionFailed {
                    message: format!("Shell command failed: {}", e),
                })?;

            if !result.status.success() {
                return Err(PhaseError::ExecutionFailed {
                    message: format!(
                        "Command exited with code {:?}: {}",
                        result.status.code(), result.stderr
                        ),
                    });
                }

                Ok(result.stdout)
        } else {
            Err(PhaseError::ExecutionFailed {
                message: "Only shell commands are supported in setup phase".to_string(),
            })
        }
    }

    /// Check if a work items file was generated
    fn check_for_work_items_file(&self, context: &PhaseContext) -> Option<String> {
        let work_items_path = context.environment.working_dir.join("work-items.json");

        if work_items_path.exists() {
            info!("Found generated work-items.json file");
            Some(work_items_path.to_string_lossy().to_string())
        } else {
            None
        }
    }
}

#[async_trait]
impl PhaseExecutor for SetupPhaseExecutor {
    async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError> {
        info!("Starting setup phase execution");
        let start_time = Instant::now();

        // Execute setup commands
        let captured_outputs = self
            .execute_setup_commands(&self.setup_phase.commands, context)
            .await?;

        // Check if work items file was generated
        let work_items_file = self.check_for_work_items_file(context);

        // Update context with captured outputs
        for (key, value) in &captured_outputs {
            context.variables.insert(key.clone(), value.clone());
        }

        let duration = start_time.elapsed();
        let metrics = PhaseMetrics {
            duration_secs: duration.as_secs_f64(),
            items_processed: self.setup_phase.commands.len(),
            items_successful: self.setup_phase.commands.len(),
            items_failed: 0,
        };

        Ok(PhaseResult {
            phase_type: PhaseType::Setup,
            success: true,
            data: Some(json!({
                "captured_outputs": captured_outputs,
                "work_items_file": work_items_file,
                "variables": context.variables,
            })),
            error_message: None,
            metrics,
        })
    }

    fn phase_type(&self) -> PhaseType {
        PhaseType::Setup
    }

    fn can_skip(&self, _context: &PhaseContext) -> bool {
        // Setup phase can be skipped if there are no commands
        self.setup_phase.commands.is_empty()
    }

    fn validate_context(&self, _context: &PhaseContext) -> Result<(), PhaseError> {
        // Validate that we have commands to execute
        if self.setup_phase.commands.is_empty() {
            return Err(PhaseError::ValidationError {
                message: "No setup commands to execute".to_string(),
            });
        }

        // Note: We don't validate working directory existence here as it may not exist in test environments
        // The actual execution will handle missing directories appropriately

        Ok(())
    }
}
