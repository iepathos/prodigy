//! Orchestrator construction and configuration
//!
//! This module handles the creation and setup of DefaultCookOrchestrator instances.
//! It provides a builder pattern for configuring the orchestrator with its dependencies.

use crate::abstractions::git::GitOperations;
use crate::cook::environment::{EnvironmentConfig, EnvValue};
use crate::cook::execution::{ClaudeExecutor, CommandExecutor};
use crate::cook::interaction::UserInteraction;
use crate::cook::session::SessionManager;
use crate::cook::workflow::WorkflowExecutorImpl;
use crate::config::WorkflowConfig;
use crate::testing::config::TestConfiguration;
use std::path::PathBuf;
use std::sync::Arc;

use super::core::{CookConfig, DefaultCookOrchestrator};

/// Builder for creating DefaultCookOrchestrator instances
pub struct OrchestratorBuilder {
    session_manager: Arc<dyn SessionManager>,
    command_executor: Arc<dyn CommandExecutor>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
    git_operations: Arc<dyn GitOperations>,
    subprocess: crate::subprocess::SubprocessManager,
    test_config: Option<Arc<TestConfiguration>>,
}

impl OrchestratorBuilder {
    /// Create a new builder with required dependencies
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: crate::subprocess::SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
            test_config: None,
        }
    }

    /// Add test configuration (for testing)
    pub fn with_test_config(mut self, test_config: Arc<TestConfiguration>) -> Self {
        self.test_config = Some(test_config);
        self
    }

    /// Build the orchestrator
    pub fn build(self) -> DefaultCookOrchestrator {
        DefaultCookOrchestrator::from_builder(
            self.session_manager,
            self.command_executor,
            self.claude_executor,
            self.user_interaction,
            self.git_operations,
            self.subprocess,
            self.test_config,
        )
    }
}

/// Pure helper functions for creating workflow components
impl OrchestratorBuilder {
    /// Create environment configuration from workflow config
    pub fn create_env_config(workflow: &WorkflowConfig) -> EnvironmentConfig {
        EnvironmentConfig {
            global_env: workflow
                .env
                .as_ref()
                .map(|env| {
                    env.iter()
                        .map(|(k, v)| (k.clone(), EnvValue::Static(v.clone())))
                        .collect()
                })
                .unwrap_or_default(),
            secrets: workflow.secrets.clone().unwrap_or_default(),
            env_files: workflow.env_files.clone().unwrap_or_default(),
            inherit: true,
            profiles: workflow.profiles.clone().unwrap_or_default(),
            active_profile: None,
        }
    }
}

/// Configuration helper methods for the orchestrator
pub trait OrchestratorConfigHelpers {
    /// Create workflow executor - avoids repeated Arc cloning
    fn create_workflow_executor(&self, config: &CookConfig) -> WorkflowExecutorImpl;

    /// Create base workflow state for session management - avoids field cloning
    fn create_workflow_state_base(
        &self,
        config: &CookConfig,
    ) -> (PathBuf, Vec<String>, Vec<String>);
}

impl OrchestratorConfigHelpers for DefaultCookOrchestrator {
    fn create_workflow_executor(&self, config: &CookConfig) -> WorkflowExecutorImpl {
        self.create_workflow_executor_internal(config)
    }

    fn create_workflow_state_base(
        &self,
        config: &CookConfig,
    ) -> (PathBuf, Vec<String>, Vec<String>) {
        self.create_workflow_state_base_internal(config)
    }
}
