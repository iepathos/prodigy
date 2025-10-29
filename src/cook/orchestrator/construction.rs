//! Orchestrator construction and configuration
//!
//! This module contains factory methods and configuration builders for creating
//! DefaultCookOrchestrator instances.

use crate::abstractions::git::GitOperations;
use crate::config::WorkflowConfig;
use crate::cook::command::CookCommand;
use crate::cook::execution::{ClaudeExecutor, CommandExecutor};
use crate::cook::interaction::UserInteraction;
use crate::cook::session::SessionManager;
use crate::testing::config::TestConfiguration;
use std::path::PathBuf;
use std::sync::Arc;

use super::core::DefaultCookOrchestrator;

/// Create a new orchestrator with dependencies
#[allow(clippy::too_many_arguments)]
pub fn new_orchestrator(
    session_manager: Arc<dyn SessionManager>,
    command_executor: Arc<dyn CommandExecutor>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
    git_operations: Arc<dyn GitOperations>,
    subprocess: crate::subprocess::SubprocessManager,
) -> DefaultCookOrchestrator {
    DefaultCookOrchestrator::new(
        session_manager,
        command_executor,
        claude_executor,
        user_interaction,
        git_operations,
        subprocess,
    )
}

/// Create a new orchestrator with test configuration
#[allow(clippy::too_many_arguments)]
pub fn new_orchestrator_with_test_config(
    session_manager: Arc<dyn SessionManager>,
    command_executor: Arc<dyn CommandExecutor>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
    git_operations: Arc<dyn GitOperations>,
    subprocess: crate::subprocess::SubprocessManager,
    test_config: Arc<TestConfiguration>,
) -> DefaultCookOrchestrator {
    DefaultCookOrchestrator::with_test_config(
        session_manager,
        command_executor,
        claude_executor,
        user_interaction,
        git_operations,
        subprocess,
        test_config,
    )
}

/// Create environment configuration from workflow config
pub fn create_env_config(workflow: &WorkflowConfig) -> crate::cook::environment::EnvironmentConfig {
    crate::cook::environment::EnvironmentConfig {
        global_env: workflow
            .env
            .as_ref()
            .map(|env| {
                env.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            crate::cook::environment::EnvValue::Static(v.clone()),
                        )
                    })
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

/// Create workflow executor instance
pub fn create_workflow_executor(
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    playbook_path: PathBuf,
) -> crate::cook::workflow::WorkflowExecutorImpl {
    crate::cook::workflow::WorkflowExecutorImpl::new(
        claude_executor,
        session_manager,
        user_interaction,
    )
    .with_workflow_path(playbook_path)
}

/// Create base workflow state components for session management
pub fn create_workflow_state_base(command: &CookCommand) -> (PathBuf, Vec<String>, Vec<String>) {
    (
        command.playbook.clone(),
        command.args.clone(),
        command.map.clone(),
    )
}

/// Generate a new session ID
pub fn generate_session_id() -> String {
    crate::unified_session::SessionId::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();

        // Session IDs should be non-empty
        assert!(!id1.is_empty());
        assert!(!id2.is_empty());

        // Session IDs should be unique (with very high probability)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_create_env_config_empty() {
        let workflow = WorkflowConfig {
            name: None,
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let env_config = create_env_config(&workflow);

        assert!(env_config.global_env.is_empty());
        assert!(env_config.secrets.is_empty());
        assert!(env_config.env_files.is_empty());
        assert!(env_config.profiles.is_empty());
        assert!(env_config.inherit);
    }

    #[test]
    fn test_create_env_config_with_env() {
        let mut env = HashMap::new();
        env.insert("KEY1".to_string(), "value1".to_string());
        env.insert("KEY2".to_string(), "value2".to_string());

        let workflow = WorkflowConfig {
            name: None,
            commands: vec![],
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let env_config = create_env_config(&workflow);

        assert_eq!(env_config.global_env.len(), 2);
        assert!(env_config.global_env.contains_key("KEY1"));
        assert!(env_config.global_env.contains_key("KEY2"));
    }

    #[test]
    fn test_create_workflow_state_base() {
        let command = CookCommand {
            playbook: PathBuf::from("/path/to/workflow.yml"),
            path: None,
            max_iterations: 1,
            args: vec!["arg1".to_string(), "arg2".to_string()],
            map: vec!["map1".to_string()],
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run: false,
        };

        let (playbook, args, map) = create_workflow_state_base(&command);

        assert_eq!(playbook, PathBuf::from("/path/to/workflow.yml"));
        assert_eq!(args, vec!["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(map, vec!["map1".to_string()]);
    }
}
