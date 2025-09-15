//! Workflow coordinator for high-level workflow orchestration

use crate::config::command::WorkflowStepCommand;
use crate::config::WorkflowCommand;
use crate::cook::interaction::UserInteraction;
use crate::cook::workflow::{CaptureOutput, WorkflowStep};
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
    workflow_executor: Arc<dyn crate::cook::workflow::WorkflowExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
}

impl DefaultWorkflowCoordinator {
    /// Create new workflow coordinator
    pub fn new(
        workflow_executor: Arc<dyn crate::cook::workflow::WorkflowExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            workflow_executor,
            user_interaction,
        }
    }

    fn extract_command_string(command: &WorkflowCommand) -> String {
        match command {
            crate::config::WorkflowCommand::Simple(s) => s.clone(),
            crate::config::WorkflowCommand::Structured(c) => c.name.clone(),
            crate::config::WorkflowCommand::WorkflowStep(step) => {
                Self::extract_workflow_step_command(step)
            }
            crate::config::WorkflowCommand::SimpleObject(obj) => obj.name.clone(),
        }
    }

    fn extract_workflow_step_command(step: &WorkflowStepCommand) -> String {
        if let Some(claude_cmd) = &step.claude {
            claude_cmd.clone()
        } else if let Some(shell_cmd) = &step.shell {
            format!("shell {shell_cmd}")
        } else if let Some(test_cmd) = &step.test {
            format!("test {}", test_cmd.command)
        } else {
            String::new()
        }
    }

    fn normalize_command_string(command_str: String) -> String {
        if command_str.starts_with('/') {
            command_str
        } else {
            format!("/{command_str}")
        }
    }

    fn create_default_workflow_step(command: Option<String>) -> WorkflowStep {
        WorkflowStep {
            name: None,
            command,
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            handler: None,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
            validate: None,
            when: None,
        }
    }

    fn convert_to_workflow_step(command: &WorkflowCommand) -> WorkflowStep {
        let command_str = Self::extract_command_string(command);
        let normalized_command = Self::normalize_command_string(command_str);
        Self::create_default_workflow_step(Some(normalized_command))
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

        // For now, return empty outputs as actual execution would be done elsewhere
        // The coordinator just coordinates, doesn't execute
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
                let step = Self::convert_to_workflow_step(command);
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

    #[test]
    fn test_extract_command_string_simple() {
        let command = WorkflowCommand::Simple("test-command".to_string());
        let result = DefaultWorkflowCoordinator::extract_command_string(&command);
        assert_eq!(result, "test-command");
    }

    #[test]
    fn test_extract_command_string_structured() {
        use crate::config::command::Command;
        let command = WorkflowCommand::Structured(Box::new(Command {
            name: "structured-command".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: Default::default(),
            id: None,
            outputs: None,
            analysis: None,
        }));
        let result = DefaultWorkflowCoordinator::extract_command_string(&command);
        assert_eq!(result, "structured-command");
    }

    #[test]
    fn test_extract_command_string_simple_object() {
        use crate::config::command::SimpleCommand;
        let command = WorkflowCommand::SimpleObject(SimpleCommand {
            name: "simple-object".to_string(),
            commit_required: None,
            args: None,
            analysis: None,
        });
        let result = DefaultWorkflowCoordinator::extract_command_string(&command);
        assert_eq!(result, "simple-object");
    }

    #[test]
    fn test_extract_workflow_step_command_claude() {
        let step = WorkflowStepCommand {
            claude: Some("claude-command".to_string()),
            shell: None,
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            commit_required: true,
            analysis: None,
            outputs: None,
            validate: None,
            timeout: None,
            when: None,
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };
        let result = DefaultWorkflowCoordinator::extract_workflow_step_command(&step);
        assert_eq!(result, "claude-command");
    }

    #[test]
    fn test_extract_workflow_step_command_shell() {
        let step = WorkflowStepCommand {
            claude: None,
            shell: Some("ls -la".to_string()),
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            commit_required: true,
            analysis: None,
            outputs: None,
            validate: None,
            timeout: None,
            when: None,
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };
        let result = DefaultWorkflowCoordinator::extract_workflow_step_command(&step);
        assert_eq!(result, "shell ls -la");
    }

    #[test]
    fn test_extract_workflow_step_command_test() {
        use crate::config::command::TestCommand;
        let step = WorkflowStepCommand {
            claude: None,
            shell: None,
            analyze: None,
            test: Some(TestCommand {
                command: "cargo test".to_string(),
                on_failure: None,
            }),
            goal_seek: None,
            foreach: None,
            id: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            commit_required: true,
            analysis: None,
            outputs: None,
            validate: None,
            timeout: None,
            when: None,
        };
        let result = DefaultWorkflowCoordinator::extract_workflow_step_command(&step);
        assert_eq!(result, "test cargo test");
    }

    #[test]
    fn test_extract_workflow_step_command_empty() {
        let step = WorkflowStepCommand {
            claude: None,
            shell: None,
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            commit_required: true,
            analysis: None,
            outputs: None,
            validate: None,
            timeout: None,
            when: None,
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };
        let result = DefaultWorkflowCoordinator::extract_workflow_step_command(&step);
        assert_eq!(result, "");
    }

    #[test]
    fn test_normalize_command_string_with_slash() {
        let result = DefaultWorkflowCoordinator::normalize_command_string("/command".to_string());
        assert_eq!(result, "/command");
    }

    #[test]
    fn test_normalize_command_string_without_slash() {
        let result = DefaultWorkflowCoordinator::normalize_command_string("command".to_string());
        assert_eq!(result, "/command");
    }

    #[test]
    fn test_create_default_workflow_step() {
        let step =
            DefaultWorkflowCoordinator::create_default_workflow_step(Some("/test".to_string()));
        assert_eq!(step.command, Some("/test".to_string()));
        assert!(step.commit_required);
        assert!(step.env.is_empty());
        assert!(step.on_exit_code.is_empty());
        assert!(step.name.is_none());
        assert!(step.claude.is_none());
        assert!(step.shell.is_none());
        assert!(step.test.is_none());
        assert!(step.handler.is_none());
        assert!(!step.capture_output.is_enabled());
        assert!(step.timeout.is_none());
        assert!(step.working_dir.is_none());
        assert!(step.on_failure.is_none());
        assert!(step.on_success.is_none());
    }

    #[test]
    fn test_convert_to_workflow_step_integration() {
        // Test with a simple command
        let command = WorkflowCommand::Simple("test".to_string());
        let step = DefaultWorkflowCoordinator::convert_to_workflow_step(&command);
        assert_eq!(step.command, Some("/test".to_string()));

        // Test with a command already having slash
        let command = WorkflowCommand::Simple("/test".to_string());
        let step = DefaultWorkflowCoordinator::convert_to_workflow_step(&command);
        assert_eq!(step.command, Some("/test".to_string()));
    }

    #[test]
    fn test_extract_workflow_step_command_priority() {
        // Test priority: claude > shell > test
        let step = WorkflowStepCommand {
            claude: Some("claude".to_string()),
            shell: Some("shell".to_string()),
            analyze: None,
            test: Some(crate::config::command::TestCommand {
                command: "test".to_string(),
                on_failure: None,
            }),
            goal_seek: None,
            foreach: None,
            id: None,
            capture_output: None,
            on_failure: None,
            on_success: None,
            commit_required: true,
            analysis: None,
            outputs: None,
            validate: None,
            timeout: None,
            when: None,
        };
        let result = DefaultWorkflowCoordinator::extract_workflow_step_command(&step);
        assert_eq!(
            result, "claude",
            "claude should take priority over shell and test"
        );
    }
}
