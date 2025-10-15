//! Workflow execution logic for cook orchestrator
//!
//! Handles different workflow execution modes: standard, iterative, and structured.

use crate::config::WorkflowCommand;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::core::{CookConfig, ExecutionEnvironment};
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::cook::workflow::WorkflowStep;
use anyhow::{anyhow, Result};
use std::sync::Arc;

/// Workflow executor for managing workflow execution
pub struct WorkflowExecutor {
    session_manager: Arc<dyn SessionManager>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
    subprocess: crate::subprocess::SubprocessManager,
}

impl WorkflowExecutor {
    /// Create a new WorkflowExecutor instance
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        subprocess: crate::subprocess::SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            claude_executor,
            user_interaction,
            subprocess,
        }
    }

    /// Execute standard workflow from a specific point
    pub async fn execute_standard_workflow_from(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        _start_iteration: usize,
        start_step: usize,
    ) -> Result<()> {
        // Standard workflow only has one iteration
        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .map(Self::convert_command_to_step)
            .collect();

        // Execute steps starting from start_step
        for (index, step) in steps.iter().enumerate().skip(start_step) {
            self.user_interaction.display_info(&format!(
                "Executing step {}/{}",
                index + 1,
                steps.len()
            ));

            // Save checkpoint before executing
            let mut workflow_state = crate::cook::session::WorkflowState {
                current_iteration: 0,
                current_step: index,
                completed_steps: Vec::new(),
                workflow_path: config.command.playbook.clone(),
                input_args: config.command.args.clone(),
                map_patterns: config.command.map.clone(),
                using_worktree: true,
            };

            self.session_manager
                .update_session(SessionUpdate::UpdateWorkflowState(workflow_state.clone()))
                .await?;

            // Execute the step
            self.execute_step(env, step, config).await?;

            // Update completed steps
            workflow_state
                .completed_steps
                .push(crate::cook::session::StepResult {
                    step_index: index,
                    command: format!("{:?}", step),
                    success: true,
                    output: None,
                    duration: std::time::Duration::from_secs(0),
                    error: None,
                    started_at: chrono::Utc::now(),
                    completed_at: chrono::Utc::now(),
                    exit_code: Some(0),
                });

            // Save checkpoint after successful execution
            self.session_manager
                .update_session(SessionUpdate::UpdateWorkflowState(workflow_state))
                .await?;
        }

        Ok(())
    }

    /// Execute iterative workflow from a specific point
    pub async fn execute_iterative_workflow_from(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        start_iteration: usize,
        start_step: usize,
    ) -> Result<()> {
        // Similar to standard workflow but with iteration support
        let max_iterations = config.command.max_iterations as usize;

        for iteration in start_iteration..max_iterations {
            self.user_interaction.display_info(&format!(
                "Iteration {}/{}",
                iteration + 1,
                max_iterations
            ));

            self.session_manager
                .update_session(SessionUpdate::StartIteration((iteration + 1) as u32))
                .await?;

            let steps: Vec<WorkflowStep> = config
                .workflow
                .commands
                .iter()
                .map(Self::convert_command_to_step)
                .collect();

            let step_start = if iteration == start_iteration {
                start_step
            } else {
                0
            };

            for (index, step) in steps.iter().enumerate().skip(step_start) {
                // Save checkpoint and execute step
                let workflow_state = crate::cook::session::WorkflowState {
                    current_iteration: iteration,
                    current_step: index,
                    completed_steps: Vec::new(),
                    workflow_path: config.command.playbook.clone(),
                    input_args: config.command.args.clone(),
                    map_patterns: config.command.map.clone(),
                    using_worktree: true,
                };

                self.session_manager
                    .update_session(SessionUpdate::UpdateWorkflowState(workflow_state))
                    .await?;

                self.execute_step(env, step, config).await?;
            }

            self.session_manager
                .update_session(SessionUpdate::CompleteIteration)
                .await?;
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;
        }

        Ok(())
    }

    /// Execute structured workflow from a specific point
    pub async fn execute_structured_workflow_from(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        _start_iteration: usize,
        start_step: usize,
    ) -> Result<()> {
        // Similar to standard workflow but preserves output handling
        self.execute_standard_workflow_from(env, config, 0, start_step)
            .await
    }

    /// Execute a single workflow step
    pub async fn execute_step(
        &self,
        env: &ExecutionEnvironment,
        step: &WorkflowStep,
        _config: &CookConfig,
    ) -> Result<()> {
        // Execute based on step type
        if let Some(ref claude_cmd) = step.claude {
            // Execute Claude command using the correct method
            let env_vars = std::collections::HashMap::new();
            self.claude_executor
                .execute_claude_command(claude_cmd, &env.working_dir, env_vars)
                .await?;
        } else if let Some(ref shell_cmd) = step.shell {
            // Execute shell command using subprocess runner
            use crate::subprocess::{ProcessCommand, ProcessError};
            let command = ProcessCommand {
                program: "sh".to_string(),
                args: vec!["-c".to_string(), shell_cmd.clone()],
                working_dir: Some(env.working_dir.to_path_buf()),
                env: std::collections::HashMap::new(),
                timeout: None,
                stdin: None,
                suppress_stderr: false,
            };
            let output = self
                .subprocess
                .runner()
                .run(command)
                .await
                .map_err(|e: ProcessError| anyhow!("Shell command failed: {}", e))?;
            if !output.status.success() {
                return Err(anyhow!("Shell command failed: {}", shell_cmd));
            }
        }

        Ok(())
    }

    /// Convert a workflow command to a workflow step
    pub fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
        super::normalization::convert_command_to_step(cmd)
    }

    /// Helper to match glob-style patterns (delegates to workflow_classifier)
    pub fn matches_glob_pattern(&self, file: &str, pattern: &str) -> bool {
        super::workflow_classifier::matches_glob_pattern(file, pattern)
    }
}
