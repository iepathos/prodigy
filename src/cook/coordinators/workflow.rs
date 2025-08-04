//! Workflow coordinator for high-level workflow orchestration

use crate::config::WorkflowCommand;
use crate::cook::interaction::UserInteraction;
use crate::cook::workflow::WorkflowStep;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Workflow execution context
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Current iteration
    pub iteration: usize,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Variables for command substitution
    pub variables: HashMap<String, String>,
}

/// Trait for workflow coordination
#[async_trait]
pub trait WorkflowCoordinator: Send + Sync {
    /// Execute a workflow step
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> Result<HashMap<String, String>>;

    /// Execute complete workflow
    async fn execute_workflow(
        &self,
        commands: &[WorkflowCommand],
        context: &mut WorkflowContext,
    ) -> Result<()>;

    /// Check if workflow should continue
    async fn should_continue(&self, context: &WorkflowContext) -> Result<bool>;

    /// Handle user interaction
    async fn prompt_user(&self, message: &str, default: bool) -> Result<bool>;

    /// Display progress
    fn display_progress(&self, message: &str);
}

/// Default implementation of workflow coordinator
pub struct DefaultWorkflowCoordinator {
    #[allow(dead_code)]
    workflow_executor: Arc<crate::cook::workflow::WorkflowExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
}

impl DefaultWorkflowCoordinator {
    /// Create new workflow coordinator
    pub fn new(
        workflow_executor: Arc<crate::cook::workflow::WorkflowExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            workflow_executor,
            user_interaction,
        }
    }
}

#[async_trait]
impl WorkflowCoordinator for DefaultWorkflowCoordinator {
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> Result<HashMap<String, String>> {
        // Display progress
        let step_display = step.name.as_deref().unwrap_or("unnamed step");
        self.display_progress(&format!(
            "Executing step: {} (iteration {}/{})",
            step_display, context.iteration, context.max_iterations
        ));

        // For now, return empty outputs as we delegate to workflow executor
        Ok(HashMap::new())
    }

    async fn execute_workflow(
        &self,
        commands: &[WorkflowCommand],
        context: &mut WorkflowContext,
    ) -> Result<()> {
        // Execute workflow
        loop {
            context.iteration += 1;

            // Check if we should continue
            if !self.should_continue(context).await? {
                break;
            }

            // Execute all commands in the workflow
            for command in commands.iter() {
                // Convert to workflow step
                let command_str = match command {
                    crate::config::WorkflowCommand::Simple(s) => s.clone(),
                    crate::config::WorkflowCommand::Structured(c) => c.name.clone(),
                    crate::config::WorkflowCommand::WorkflowStep(step) => {
                        if let Some(claude_cmd) = &step.claude {
                            claude_cmd.clone()
                        } else if let Some(shell_cmd) = &step.shell {
                            format!("shell {shell_cmd}")
                        } else {
                            String::new()
                        }
                    }
                    crate::config::WorkflowCommand::SimpleObject(obj) => obj.name.clone(),
                };

                let step = WorkflowStep {
                    name: None,
                    command: Some(if command_str.starts_with('/') {
                        command_str
                    } else {
                        format!("/{command_str}")
                    }),
                    claude: None,
                    shell: None,
                    capture_output: false,
                    timeout: None,
                    working_dir: None,
                    env: HashMap::new(),
                    on_failure: None,
                    on_success: None,
                    on_exit_code: HashMap::new(),
                    commit_required: true,
                    analysis: None,
                };

                // Execute step
                let _outputs = self.execute_step(&step, context).await?;
            }
        }

        Ok(())
    }

    async fn should_continue(&self, context: &WorkflowContext) -> Result<bool> {
        // Check iteration limit
        if context.iteration > context.max_iterations {
            self.display_progress(&format!(
                "Reached maximum iterations ({})",
                context.max_iterations
            ));
            return Ok(false);
        }

        // Could add more conditions here (e.g., check if improvements were made)
        Ok(true)
    }

    async fn prompt_user(&self, message: &str, _default: bool) -> Result<bool> {
        self.user_interaction.prompt_yes_no(message).await
    }

    fn display_progress(&self, message: &str) {
        self.user_interaction.display_progress(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_should_continue() {
        let context = WorkflowContext {
            iteration: 3,
            max_iterations: 5,
            variables: HashMap::new(),
        };

        // Test with a simple mock coordinator
        struct TestCoordinator;

        #[async_trait]
        impl WorkflowCoordinator for TestCoordinator {
            async fn execute_step(
                &self,
                _step: &WorkflowStep,
                _context: &WorkflowContext,
            ) -> Result<HashMap<String, String>> {
                Ok(HashMap::new())
            }

            async fn execute_workflow(
                &self,
                _commands: &[WorkflowCommand],
                _context: &mut WorkflowContext,
            ) -> Result<()> {
                Ok(())
            }

            async fn should_continue(&self, context: &WorkflowContext) -> Result<bool> {
                Ok(context.iteration <= context.max_iterations)
            }

            async fn prompt_user(&self, _message: &str, _default: bool) -> Result<bool> {
                Ok(true)
            }

            fn display_progress(&self, _message: &str) {}
        }

        let coordinator = TestCoordinator;
        let should_continue = coordinator.should_continue(&context).await.unwrap();
        assert!(should_continue);
    }

    #[tokio::test]
    async fn test_workflow_max_iterations_reached() {
        let context = WorkflowContext {
            iteration: 6,
            max_iterations: 5,
            variables: HashMap::new(),
        };

        struct TestCoordinator;

        #[async_trait]
        impl WorkflowCoordinator for TestCoordinator {
            async fn execute_step(
                &self,
                _step: &WorkflowStep,
                _context: &WorkflowContext,
            ) -> Result<HashMap<String, String>> {
                Ok(HashMap::new())
            }

            async fn execute_workflow(
                &self,
                _commands: &[WorkflowCommand],
                _context: &mut WorkflowContext,
            ) -> Result<()> {
                Ok(())
            }

            async fn should_continue(&self, context: &WorkflowContext) -> Result<bool> {
                Ok(context.iteration <= context.max_iterations)
            }

            async fn prompt_user(&self, _message: &str, _default: bool) -> Result<bool> {
                Ok(true)
            }

            fn display_progress(&self, _message: &str) {}
        }

        let coordinator = TestCoordinator;
        let should_continue = coordinator.should_continue(&context).await.unwrap();
        assert!(!should_continue);
    }
}
