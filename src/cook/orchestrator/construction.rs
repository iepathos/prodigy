//! Orchestrator construction and configuration
//!
//! This module contains factory methods and configuration builders for creating
//! DefaultCookOrchestrator instances.

use crate::abstractions::git::GitOperations;
use crate::config::mapreduce::MergeWorkflow;
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

/// Pure function: Determine if unified session should be created
pub fn should_create_unified_session(has_mapreduce_config: bool, dry_run: bool) -> bool {
    !has_mapreduce_config && !dry_run
}

/// Pure function: Generate workflow ID
pub fn generate_workflow_id() -> String {
    format!("workflow-{}", chrono::Utc::now().timestamp_millis())
}

/// Pure function: Create session metadata
pub fn create_session_metadata(
    total_steps: usize,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "execution_start_time".to_string(),
        serde_json::json!(chrono::Utc::now().to_rfc3339()),
    );
    metadata.insert("workflow_type".to_string(), serde_json::json!("standard"));
    metadata.insert("total_steps".to_string(), serde_json::json!(total_steps));
    metadata.insert("current_step".to_string(), serde_json::json!(0));
    metadata
}

/// Pure function: Build session configuration
pub fn build_session_config(
    workflow_id: String,
    workflow_name: Option<String>,
    metadata: std::collections::HashMap<String, serde_json::Value>,
) -> crate::unified_session::SessionConfig {
    crate::unified_session::SessionConfig {
        session_type: crate::unified_session::SessionType::Workflow,
        workflow_id: Some(workflow_id),
        workflow_name,
        job_id: None,
        metadata,
    }
}

/// Pure function: Extract merge configuration
pub fn extract_merge_config(
    workflow: &WorkflowConfig,
    mapreduce_config: &Option<Arc<crate::config::MapReduceWorkflowConfig>>,
) -> Option<MergeWorkflow> {
    workflow
        .merge
        .clone()
        .or_else(|| mapreduce_config.as_ref().and_then(|m| m.merge.clone()))
}

/// Pure function: Extract workflow environment variables
pub fn extract_workflow_env(
    workflow: &WorkflowConfig,
) -> std::collections::HashMap<String, String> {
    workflow.env.clone().unwrap_or_default()
}

/// Pure function: Determine if merge should occur (cleanup decision logic)
pub fn should_merge_worktree(test_mode: bool, auto_accept: bool) -> Option<bool> {
    if test_mode {
        Some(false) // Default to not merging in test mode
    } else if auto_accept {
        Some(true) // Auto-accept when -y flag is provided
    } else {
        None // Need to prompt user
    }
}

/// Pure function: Determine if commit validation should be skipped
pub fn should_skip_commit_validation(
    test_mode: bool,
    skip_validation: bool,
    commit_required: bool,
    dry_run: bool,
) -> bool {
    (!commit_required) || dry_run || (test_mode && skip_validation)
}

/// Pure function: Determine if we should capture HEAD before command execution
/// We capture HEAD when:
/// - commit_required is true AND
/// - NOT in test mode (test mode uses different validation logic)
pub fn should_capture_head_before_execution(
    test_mode: bool,
    _skip_validation: bool,
    commit_required: bool,
) -> bool {
    commit_required && !test_mode
}

/// Pure function: Check if commits were created (pure comparison)
pub fn commits_were_created(head_before: &str, head_after: &str) -> bool {
    head_before != head_after
}

/// Pure function: Extract command name from command string
pub fn extract_command_name(command: &str) -> &str {
    command
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or(command)
}

/// Pure function: Check if command is in no-changes list
pub fn is_no_changes_command(command_name: &str, no_changes_commands: &[String]) -> bool {
    no_changes_commands
        .iter()
        .any(|cmd| cmd.trim() == command_name)
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
            params: std::collections::HashMap::new(),
        };

        let (playbook, args, map) = create_workflow_state_base(&command);

        assert_eq!(playbook, PathBuf::from("/path/to/workflow.yml"));
        assert_eq!(args, vec!["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(map, vec!["map1".to_string()]);
    }

    #[test]
    fn test_should_create_unified_session() {
        // Should create when not mapreduce and not dry-run
        assert!(should_create_unified_session(false, false));

        // Should NOT create for mapreduce
        assert!(!should_create_unified_session(true, false));

        // Should NOT create for dry-run
        assert!(!should_create_unified_session(false, true));

        // Should NOT create for mapreduce + dry-run
        assert!(!should_create_unified_session(true, true));
    }

    #[test]
    fn test_should_merge_worktree() {
        // Test mode should return Some(false)
        assert_eq!(should_merge_worktree(true, false), Some(false));
        assert_eq!(should_merge_worktree(true, true), Some(false));

        // Auto-accept should return Some(true)
        assert_eq!(should_merge_worktree(false, true), Some(true));

        // Neither test nor auto-accept should return None (prompt user)
        assert_eq!(should_merge_worktree(false, false), None);
    }

    #[test]
    fn test_should_skip_commit_validation() {
        // Skip if not required
        assert!(should_skip_commit_validation(false, false, false, false));

        // Skip if dry-run
        assert!(should_skip_commit_validation(false, false, true, true));

        // Skip if test mode with skip_validation
        assert!(should_skip_commit_validation(true, true, true, false));

        // Don't skip if required and not dry-run or test skip
        assert!(!should_skip_commit_validation(false, false, true, false));
    }

    #[test]
    fn test_commits_were_created() {
        assert!(commits_were_created("abc123", "def456"));
        assert!(!commits_were_created("abc123", "abc123"));
    }

    #[test]
    fn test_extract_command_name() {
        assert_eq!(
            extract_command_name("/prodigy-test arg1 arg2"),
            "prodigy-test"
        );
        assert_eq!(extract_command_name("prodigy-test arg1"), "prodigy-test");
        assert_eq!(extract_command_name("/simple"), "simple");
        assert_eq!(extract_command_name("command"), "command");
    }

    #[test]
    fn test_is_no_changes_command() {
        let no_changes = vec!["prodigy-test".to_string(), "prodigy-lint".to_string()];

        assert!(is_no_changes_command("prodigy-test", &no_changes));
        assert!(is_no_changes_command("prodigy-lint", &no_changes));
        assert!(!is_no_changes_command("prodigy-other", &no_changes));
    }

    #[test]
    fn test_extract_merge_config() {
        let workflow = WorkflowConfig {
            name: None,
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: Some(MergeWorkflow {
                commands: vec![],
                timeout: Some(600),
            }),
        };

        let result = extract_merge_config(&workflow, &None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().timeout, Some(600));
    }

    #[test]
    fn test_extract_workflow_env() {
        let mut env = HashMap::new();
        env.insert("VAR1".to_string(), "value1".to_string());

        let workflow = WorkflowConfig {
            name: None,
            commands: vec![],
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let result = extract_workflow_env(&workflow);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("VAR1"), Some(&"value1".to_string()));
    }
}
