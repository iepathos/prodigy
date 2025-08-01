//! Workflow executor implementation

use crate::cook::analysis::AnalysisCoordinator;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::metrics::MetricsCoordinator;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::{SessionManager, SessionUpdate};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// A simple workflow step for the new orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Step name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Extended workflow configuration for the new orchestrator
#[derive(Debug, Clone)]
pub struct ExtendedWorkflowConfig {
    /// Workflow name
    pub name: String,
    /// Steps to execute
    pub steps: Vec<WorkflowStep>,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Whether to iterate
    pub iterate: bool,
    /// Analyze before workflow
    pub analyze_before: bool,
    /// Analyze between iterations
    pub analyze_between: bool,
    /// Collect metrics
    pub collect_metrics: bool,
}

/// Executes workflow steps
pub struct WorkflowExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    analysis_coordinator: Arc<dyn AnalysisCoordinator>,
    metrics_coordinator: Arc<dyn MetricsCoordinator>,
    user_interaction: Arc<dyn UserInteraction>,
}

impl WorkflowExecutor {
    /// Create a new workflow executor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        analysis_coordinator: Arc<dyn AnalysisCoordinator>,
        metrics_coordinator: Arc<dyn MetricsCoordinator>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            analysis_coordinator,
            metrics_coordinator,
            user_interaction,
        }
    }

    /// Execute a workflow
    pub async fn execute(
        &self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        self.user_interaction.display_info(&format!(
            "Executing workflow: {} (max {} iterations)",
            workflow.name, workflow.max_iterations
        ));

        let mut iteration = 0;
        let mut should_continue = true;

        while should_continue && iteration < workflow.max_iterations {
            iteration += 1;
            self.user_interaction.display_progress(&format!(
                "Starting iteration {}/{}",
                iteration, workflow.max_iterations
            ));

            // Update session
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;

            // Execute workflow steps
            for (step_index, step) in workflow.steps.iter().enumerate() {
                self.user_interaction.display_progress(&format!(
                    "Executing step {}/{}: {}",
                    step_index + 1,
                    workflow.steps.len(),
                    step.name
                ));

                self.execute_step(step, env).await
                    .context(format!("Failed to execute step: {}", step.name))?;
            }

            // Check if we should continue
            if workflow.iterate {
                // In automated mode, check based on metrics or other criteria
                if let Ok(metrics) = self.metrics_coordinator
                    .collect_all(&env.working_dir)
                    .await
                {
                    // Simple heuristic: stop if no lint warnings
                    if metrics.lint_warnings == 0 {
                        self.user_interaction.display_success(
                            "No lint warnings remaining, stopping iterations"
                        );
                        should_continue = false;
                    }
                } else {
                    // If metrics collection fails, ask user
                    should_continue = self.user_interaction
                        .prompt_yes_no("Continue with another iteration?")
                        .await?;
                }
            } else {
                // Single iteration workflow
                should_continue = false;
            }

            // Run analysis between iterations if configured
            if should_continue && workflow.analyze_between {
                self.user_interaction.display_progress("Running analysis between iterations...");
                let analysis = self.analysis_coordinator
                    .analyze_project(&env.working_dir)
                    .await?;
                self.analysis_coordinator
                    .save_analysis(&env.working_dir, &analysis)
                    .await?;
            }
        }

        // Collect final metrics if enabled
        if workflow.collect_metrics {
            self.user_interaction.display_progress("Collecting final metrics...");
            let metrics = self.metrics_coordinator
                .collect_all(&env.working_dir)
                .await?;
            self.metrics_coordinator
                .store_metrics(&env.working_dir, &metrics)
                .await?;
            
            // Generate report
            let history = self.metrics_coordinator
                .load_history(&env.working_dir)
                .await?;
            let report = self.metrics_coordinator
                .generate_report(&metrics, &history)
                .await?;
            self.user_interaction.display_info(&report);
        }

        Ok(())
    }

    /// Execute a single workflow step
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Prepare environment variables
        let mut env_vars = HashMap::new();
        
        // Add MMM context variables
        env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "MMM_CONTEXT_DIR".to_string(),
            env.working_dir.join(".mmm/context").to_string_lossy().to_string(),
        );
        
        if let Some(ref focus) = env.focus {
            env_vars.insert("MMM_FOCUS".to_string(), focus.clone());
        }
        
        env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());

        // Add step-specific environment variables
        for (key, value) in &step.env {
            env_vars.insert(key.clone(), value.clone());
        }

        // Execute the command
        let result = self.claude_executor
            .execute_claude_command(&step.command, &env.working_dir, env_vars)
            .await?;

        if !result.success {
            anyhow::bail!(
                "Step '{}' failed with exit code {:?}. Error: {}",
                step.name,
                result.exit_code,
                result.stderr
            );
        }

        // Count files changed (simplified - in real implementation would use git)
        self.session_manager
            .update_session(SessionUpdate::AddFilesChanged(1))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::cook::execution::claude::ClaudeExecutorImpl;
    use crate::cook::execution::ExecutionResult;
    use crate::cook::analysis::runner::AnalysisRunnerImpl;
    use crate::cook::metrics::collector::MetricsCollectorImpl;
    use crate::cook::session::tracker::SessionTrackerImpl;
    use crate::cook::interaction::mocks::MockUserInteraction;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workflow_executor_single_step() {
        let temp_dir = TempDir::new().unwrap();
        let mock_runner = Arc::new(MockCommandRunner::new());
        
        // Setup successful command response
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "Command executed".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let claude_executor = Arc::new(ClaudeExecutorImpl::new(mock_runner.clone()));
        let session_manager = Arc::new(SessionTrackerImpl::new(
            "test".to_string(),
            temp_dir.path().to_path_buf(),
        ));
        let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(mock_runner.clone()));
        let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(mock_runner));
        let user_interaction = Arc::new(MockUserInteraction::new());

        let executor = WorkflowExecutor::new(
            claude_executor,
            session_manager.clone(),
            analysis_coordinator,
            metrics_coordinator,
            user_interaction.clone(),
        );

        let workflow = ExtendedWorkflowConfig {
            name: "test".to_string(),
            steps: vec![WorkflowStep {
                name: "Test Step".to_string(),
                command: "/test-command".to_string(),
                env: HashMap::new(),
            }],
            max_iterations: 1,
            iterate: false,
            analyze_before: false,
            analyze_between: false,
            collect_metrics: false,
        };

        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test".to_string(),
            focus: None,
        };

        executor.execute(&workflow, &env).await.unwrap();

        // Verify session was updated
        assert_eq!(session_manager.get_state().iterations_completed, 1);
        
        // Verify user was informed
        let messages = user_interaction.get_messages();
        assert!(messages.iter().any(|m| m.contains("Executing workflow: test")));
    }
}