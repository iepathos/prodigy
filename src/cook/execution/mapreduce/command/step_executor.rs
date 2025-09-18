//! Step execution orchestration
//!
//! This module handles the complete execution of workflow steps including
//! interpolation, routing to appropriate executors, and output capture.

use super::executor::{CommandRouter, ExecutionContext};
use super::interpolation::StepInterpolator;
use super::types::determine_command_type;
use crate::cook::execution::errors::MapReduceResult;
use crate::cook::execution::mapreduce::AgentContext;
use crate::cook::workflow::variables::CommandResult as VarCommandResult;
use crate::cook::workflow::StepResult;
use crate::cook::workflow::WorkflowStep;
use std::sync::Arc;

/// Executes a workflow step with full interpolation and capture handling
pub struct StepExecutor {
    command_router: Arc<CommandRouter>,
    interpolator: Arc<StepInterpolator>,
}

impl StepExecutor {
    /// Create a new step executor
    pub fn new(command_router: Arc<CommandRouter>, interpolator: Arc<StepInterpolator>) -> Self {
        Self {
            command_router,
            interpolator,
        }
    }

    /// Execute a single workflow step with interpolation and capture
    pub async fn execute(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
    ) -> MapReduceResult<StepResult> {
        // Interpolate the step
        let interpolated_step = self.interpolator.interpolate(step, context).await?;

        // Create execution context
        let exec_context = build_execution_context(context);

        // Execute via command router
        let command_result = self
            .command_router
            .execute(&interpolated_step, &exec_context)
            .await?;

        // Convert to step result
        let result: StepResult = command_result.into();

        // Handle output capture
        capture_output(step, &result, context).await?;

        Ok(result)
    }
}

/// Build execution context from agent context
fn build_execution_context(context: &AgentContext) -> ExecutionContext {
    ExecutionContext {
        worktree_path: context.worktree_path.clone(),
        worktree_name: context.worktree_name.clone(),
        item_id: context.item_id.clone(),
        variables: context.variables.clone(),
        captured_outputs: context.captured_outputs.clone(),
        environment: std::collections::HashMap::new(),
    }
}

/// Handle output capture for both new and legacy formats
async fn capture_output(
    step: &WorkflowStep,
    result: &StepResult,
    context: &mut AgentContext,
) -> MapReduceResult<()> {
    // Handle new capture field
    if let Some(capture_name) = &step.capture {
        capture_with_new_format(step, result, context, capture_name).await?;
    }

    // Handle legacy capture_output field
    if step.capture_output.is_enabled() && !result.stdout.is_empty() {
        capture_with_legacy_format(step, result, context)?;
    }

    Ok(())
}

/// Capture output using new format with variable store
async fn capture_with_new_format(
    step: &WorkflowStep,
    result: &StepResult,
    context: &mut AgentContext,
    capture_name: &str,
) -> MapReduceResult<()> {
    let command_result = VarCommandResult {
        stdout: Some(result.stdout.clone()),
        stderr: Some(result.stderr.clone()),
        exit_code: result.exit_code.unwrap_or(-1),
        success: result.success,
        duration: std::time::Duration::from_secs(0), // TODO: Track actual duration
    };

    let capture_format = step.capture_format.unwrap_or_default();
    let capture_streams = &step.capture_streams;

    context
        .variable_store
        .capture_command_result(
            capture_name,
            command_result,
            capture_format,
            capture_streams,
        )
        .await
        .map_err(
            |e| crate::cook::execution::errors::MapReduceError::General {
                message: format!("Failed to capture command result: {}", e),
                source: None,
            },
        )?;

    // Also update captured_outputs for backward compatibility
    context
        .captured_outputs
        .insert(capture_name.to_string(), result.stdout.clone());

    Ok(())
}

/// Capture output using legacy format
fn capture_with_legacy_format(
    step: &WorkflowStep,
    result: &StepResult,
    context: &mut AgentContext,
) -> MapReduceResult<()> {
    // Determine command type for variable naming
    let command_type = determine_command_type(step)?;

    // Get the variable name for this output
    if let Some(var_name) = step.capture_output.get_variable_name(&command_type) {
        context
            .captured_outputs
            .insert(var_name, result.stdout.clone());
    }

    // Store as generic CAPTURED_OUTPUT for backward compatibility
    context
        .captured_outputs
        .insert("CAPTURED_OUTPUT".to_string(), result.stdout.clone());

    Ok(())
}
