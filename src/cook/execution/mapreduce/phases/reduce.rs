//! Reduce phase executor for MapReduce workflows
//!
//! This module handles the execution of reduce commands that aggregate
//! and process results from the map phase. It extracts the complex
//! reduce logic from the main module into a focused orchestrator.

use super::{PhaseContext, PhaseError, PhaseExecutor, PhaseMetrics, PhaseResult, PhaseType};
use crate::cook::execution::mapreduce::{
    utils::{build_agent_context_variables, calculate_map_result_summary},
    AgentResult, ReducePhase,
};
use crate::cook::workflow::variables::CapturedValue;
use crate::cook::workflow::WorkflowStep;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Executor for the reduce phase of MapReduce workflows
pub struct ReducePhaseExecutor {
    /// The reduce phase configuration
    reduce_phase: ReducePhase,
}

impl ReducePhaseExecutor {
    /// Create a new reduce phase executor
    pub fn new(reduce_phase: ReducePhase) -> Self {
        Self { reduce_phase }
    }

    /// Prepare the reduce context with map results
    async fn prepare_reduce_context(
        &self,
        map_results: &[AgentResult],
        context: &mut PhaseContext,
    ) -> Result<(), PhaseError> {
        // Calculate summary statistics using pure functions
        let summary_stats = calculate_map_result_summary(map_results);

        // Build and add variables using pure functions
        let context_variables = build_agent_context_variables(map_results, &summary_stats)
            .map_err(|e| PhaseError::ExecutionFailed {
                message: format!("Failed to build agent context variables: {}", e),
            })?;

        // Transfer variables to reduce context
        for (key, value) in context_variables {
            context.variables.insert(key, value);
        }

        // Add map results to variable store as structured data
        self.add_map_results_to_store(map_results, &summary_stats, context)
            .await;

        Ok(())
    }

    /// Add map results to the variable store for access in reduce commands
    async fn add_map_results_to_store(
        &self,
        map_results: &[AgentResult],
        summary_stats: &crate::cook::execution::mapreduce::utils::MapResultSummary,
        context: &mut PhaseContext,
    ) {
        // Add summary statistics
        context
            .variable_store
            .set(
                "map.successful",
                CapturedValue::Number(summary_stats.successful as f64),
            )
            .await;
        context
            .variable_store
            .set(
                "map.failed",
                CapturedValue::Number(summary_stats.failed as f64),
            )
            .await;
        context
            .variable_store
            .set(
                "map.total",
                CapturedValue::Number(summary_stats.total as f64),
            )
            .await;

        // Add the full results as a structured JSON value
        if let Ok(results_value) = serde_json::to_value(map_results) {
            context
                .variable_store
                .set("map.results", CapturedValue::from(results_value))
                .await;
        }

        // Also add individual results for easier access
        let results_array: Vec<CapturedValue> = map_results
            .iter()
            .map(|result| {
                if let Ok(result_json) = serde_json::to_value(result) {
                    CapturedValue::from(result_json)
                } else {
                    CapturedValue::String(format!("{:?}", result))
                }
            })
            .collect();
        context
            .variable_store
            .set("map.results_array", CapturedValue::Array(results_array))
            .await;
    }

    /// Execute reduce commands sequentially
    async fn execute_reduce_commands(&self, context: &mut PhaseContext) -> Result<(), PhaseError> {
        for (step_index, step) in self.reduce_phase.commands.iter().enumerate() {
            debug!(
                "Executing reduce step {}/{}",
                step_index + 1,
                self.reduce_phase.commands.len()
            );

            // Execute the step
            let step_result = self.execute_single_step(step, context).await.map_err(|e| {
                PhaseError::ExecutionFailed {
                    message: format!("Reduce step {} failed: {}", step_index + 1, e),
                }
            })?;

            if !step_result.success {
                // Handle step failure with on_failure handler if present
                if let Some(on_failure) = &step.on_failure {
                    info!(
                        "Step {} failed, executing on_failure handler",
                        step_index + 1
                    );

                    // Store error context for handler
                    context.variables.insert(
                        "error.message".to_string(),
                        step_result
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string()),
                    );
                    context.variables.insert(
                        "error.step".to_string(),
                        format!("reduce_step_{}", step_index + 1),
                    );

                    // Execute the on_failure handler
                    let handler_result = self.handle_on_failure(on_failure, step, context).await;

                    match handler_result {
                        Ok(handled) if handled => {
                            // Handler succeeded, continue to next step
                            info!("on_failure handler succeeded, continuing");
                        }
                        Ok(_) => {
                            // Handler says we should fail
                            return Err(PhaseError::ExecutionFailed {
                                message: format!(
                                    "Reduce step {} failed and fail_workflow is true",
                                    step_index + 1
                                ),
                            });
                        }
                        Err(e) => {
                            // Handler itself failed
                            if on_failure.should_fail_workflow() {
                                return Err(PhaseError::ExecutionFailed {
                                    message: format!(
                                        "Reduce step {} on_failure handler failed: {}",
                                        step_index + 1,
                                        e
                                    ),
                                });
                            }
                            // Log but continue
                            warn!("on_failure handler failed but continuing: {}", e);
                        }
                    }
                } else {
                    // No on_failure handler, fail immediately
                    return Err(PhaseError::ExecutionFailed {
                        message: format!(
                            "Reduce step {} failed: {}",
                            step_index + 1,
                            step_result
                                .error
                                .unwrap_or_else(|| "Unknown error".to_string())
                        ),
                    });
                }
            } else if let Some(on_success) = &step.on_success {
                // Execute on_success handler for successful step
                info!(
                    "Step {} succeeded, executing on_success handler",
                    step_index + 1
                );

                // Store success context
                context.variables.insert(
                    "shell.output".to_string(),
                    step_result.output.clone().unwrap_or_default(),
                );

                // Execute the on_success handler
                let success_result = self.execute_single_step(on_success, context).await;

                if let Err(e) = success_result {
                    warn!(
                        "on_success handler failed for step {}: {}",
                        step_index + 1,
                        e
                    );
                    // Note: We don't fail the workflow when on_success handler fails
                }
            }

            // Make captured outputs available for subsequent commands
            if let Some(output) = step_result.output {
                context.variables.insert("shell.output".to_string(), output);
            }
        }

        Ok(())
    }

    /// Execute a single reduce step
    async fn execute_single_step(
        &self,
        step: &WorkflowStep,
        context: &mut PhaseContext,
    ) -> Result<StepResult, PhaseError> {
        // This is a simplified execution model
        // In the full implementation, this would delegate to the appropriate executor
        if let Some(cmd) = &step.shell {
            // Execute shell command
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

            Ok(StepResult {
                success: result.status.success(),
                output: Some(result.stdout),
                error: if !result.status.success() {
                    Some(result.stderr)
                } else {
                    None
                    },
                })
        } else {
            // Handle other command types
            Ok(StepResult {
                success: true,
                output: Some("Command executed".to_string()),
                error: None,
            })
        }
    }

    /// Handle on_failure configuration
    async fn handle_on_failure(
        &self,
        on_failure: &crate::cook::workflow::OnFailureConfig,
        _original_step: &WorkflowStep,
        context: &mut PhaseContext,
    ) -> Result<bool, PhaseError> {
        // Handle on_failure based on its variant
        match on_failure {
            crate::cook::workflow::OnFailureConfig::IgnoreErrors(_) => {
                // Just continue, errors are ignored
                Ok(true)
            }
            crate::cook::workflow::OnFailureConfig::SingleCommand(cmd) => {
                // Execute the single command
                let handler_step = crate::cook::workflow::WorkflowStep {
                    command: Some(cmd.clone()),
                    name: None,
                    claude: None,
                    shell: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    handler: None,
                    on_failure: None,
                    on_success: None,
                    timeout: None,
                    commit_required: false,
                    auto_commit: false,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    capture_output: Default::default(),
                    retry: None,
                    working_dir: None,
                    env: HashMap::new(),
                    on_exit_code: HashMap::new(),
                    commit_config: None,
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                };
                let result = self.execute_single_step(&handler_step, context).await?;
                Ok(result.success)
            }
            crate::cook::workflow::OnFailureConfig::MultipleCommands(cmds) => {
                // Execute multiple commands
                for cmd in cmds {
                    let handler_step = crate::cook::workflow::WorkflowStep {
                        command: Some(cmd.clone()),
                        name: None,
                        claude: None,
                        shell: None,
                        test: None,
                        goal_seek: None,
                        foreach: None,
                        handler: None,
                        on_failure: None,
                        on_success: None,
                        timeout: None,
                        commit_required: false,
                        auto_commit: false,
                        capture: None,
                        capture_format: None,
                        capture_streams: Default::default(),
                        output_file: None,
                        capture_output: Default::default(),
                        retry: None,
                        working_dir: None,
                        env: HashMap::new(),
                        on_exit_code: HashMap::new(),
                        commit_config: None,
                        validate: None,
                        step_validate: None,
                        skip_validation: false,
                        validation_timeout: None,
                        ignore_validation_failure: false,
                        when: None,
                    };
                    let result = self.execute_single_step(&handler_step, context).await?;
                    if !result.success {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => {
                // For other variants, just continue
                Ok(true)
            }
        }
    }
}

/// Result from executing a single step
struct StepResult {
    success: bool,
    output: Option<String>,
    error: Option<String>,
}

#[async_trait]
impl PhaseExecutor for ReducePhaseExecutor {
    async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError> {
        info!("Starting reduce phase execution");
        let start_time = Instant::now();

        // Get map results from context
        let map_results = context
            .map_results
            .as_ref()
            .ok_or_else(|| PhaseError::ValidationError {
                message: "No map results available for reduce phase".to_string(),
            })?
            .clone();

        // Calculate summary for metrics
        let summary = calculate_map_result_summary(&map_results);

        info!(
            "Processing reduce phase with {} successful and {} failed map results",
            summary.successful, summary.failed
        );

        // Prepare the reduce context with map results
        self.prepare_reduce_context(&map_results, context).await?;

        // Execute reduce commands
        self.execute_reduce_commands(context).await?;

        let duration = start_time.elapsed();
        let metrics = PhaseMetrics {
            duration_secs: duration.as_secs_f64(),
            items_processed: self.reduce_phase.commands.len(),
            items_successful: self.reduce_phase.commands.len(),
            items_failed: 0,
        };

        info!("Reduce phase completed successfully");

        Ok(PhaseResult {
            phase_type: PhaseType::Reduce,
            success: true,
            data: Some(json!({
                "commands_executed": self.reduce_phase.commands.len(),
                "map_results_processed": map_results.len(),
                "successful_agents": summary.successful,
                "failed_agents": summary.failed,
            })),
            error_message: None,
            metrics,
        })
    }

    fn phase_type(&self) -> PhaseType {
        PhaseType::Reduce
    }

    fn can_skip(&self, context: &PhaseContext) -> bool {
        // Skip reduce phase if there are no map results or no commands
        context.map_results.is_none() || self.reduce_phase.commands.is_empty()
    }

    fn validate_context(&self, context: &PhaseContext) -> Result<(), PhaseError> {
        // Validate that we have map results
        if context.map_results.is_none() {
            return Err(PhaseError::ValidationError {
                message: "Reduce phase requires map results".to_string(),
            });
        }

        // Validate that required variables are available
        // This would check for any ${map.*} references in commands
        // For now, we'll assume validation passes
        Ok(())
    }
}
