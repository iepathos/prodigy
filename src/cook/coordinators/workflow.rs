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
        self.display_progress(&format!(
            "Executing step: {} (iteration {}/{})",
            step.name, context.iteration, context.max_iterations
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
            for (i, command) in commands.iter().enumerate() {
                // Convert to workflow step
                let command_str = match command {
                    crate::config::WorkflowCommand::Simple(s) => s.clone(),
                    crate::config::WorkflowCommand::Structured(c) => c.name.clone(),
                    crate::config::WorkflowCommand::SimpleObject(obj) => obj.name.clone(),
                };

                let step = WorkflowStep {
                    name: format!("Step {}", i + 1),
                    command: if command_str.starts_with('/') {
                        command_str
                    } else {
                        format!("/{}", command_str)
                    },
                    env: HashMap::new(),
                    commit_required: true,
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
