//! Cook module - orchestrates improvement sessions
//!
//! This module has been refactored to use a component-based architecture
//! with dependency injection for improved testability and maintainability.

pub mod analysis;
pub mod command;
pub mod coordinators;
pub mod execution;
pub mod git_ops;
pub mod interaction;
pub mod metrics;
pub mod orchestrator;
pub mod retry;
pub mod session;
pub mod signal_handler;
pub mod workflow;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mod_tests;

use crate::abstractions::git::RealGitOperations;
use crate::config::{workflow::WorkflowConfig, ConfigLoader};
use crate::simple_state::StateManager;
use anyhow::{anyhow, Context as _, Result};
use std::path::Path;
use std::sync::Arc;

// Re-export key types
pub use command::CookCommand;
pub use orchestrator::{CookConfig, CookOrchestrator, DefaultCookOrchestrator};

/// Main entry point for cook operations
pub async fn cook(mut cmd: CookCommand) -> Result<()> {
    // Save the original directory before any path changes
    let original_dir = std::env::current_dir()?;

    // Determine project path
    let project_path = if let Some(ref path) = cmd.path {
        // Expand tilde notation if present
        let expanded_path = if path.to_string_lossy().starts_with("~/") {
            let home =
                dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
            home.join(
                path.strip_prefix("~/")
                    .context("Failed to strip ~/ prefix")?,
            )
        } else {
            path.clone()
        };

        // Resolve to absolute path
        let absolute_path = if expanded_path.is_absolute() {
            expanded_path
        } else {
            original_dir.join(&expanded_path)
        };

        // Validate path exists and is a directory
        if !absolute_path.exists() {
            return Err(anyhow!("Directory not found: {}", absolute_path.display()));
        }
        if !absolute_path.is_dir() {
            return Err(anyhow!(
                "Path is not a directory: {}",
                absolute_path.display()
            ));
        }

        // Check if it's a git repository
        if !absolute_path.join(".git").exists() {
            return Err(anyhow!("Not a git repository: {}", absolute_path.display()));
        }

        // Change to the specified directory
        std::env::set_current_dir(&absolute_path).with_context(|| {
            format!("Failed to change to directory: {}", absolute_path.display())
        })?;

        absolute_path
    } else {
        original_dir.clone()
    };

    // Make playbook path absolute if it's relative (based on original directory)
    if !cmd.playbook.is_absolute() {
        cmd.playbook = original_dir.join(&cmd.playbook);
    }

    // Load configuration
    let config_loader = ConfigLoader::new().await?;
    config_loader
        .load_with_explicit_path(&project_path, None)
        .await?;
    let config = config_loader.get_config();

    // Load workflow
    let workflow = load_workflow(&cmd, &config).await?;

    // Create orchestrator with all dependencies
    let orchestrator = create_orchestrator(&project_path).await?;

    // Create cook configuration
    let cook_config = CookConfig {
        command: cmd,
        project_path,
        workflow,
    };

    // Run the orchestrator
    orchestrator.run(cook_config).await
}

/// Create the orchestrator with all dependencies
async fn create_orchestrator(project_path: &Path) -> Result<Arc<dyn CookOrchestrator>> {
    // Create shared dependencies
    let git_operations = Arc::new(RealGitOperations::new());
    let subprocess = Arc::new(crate::subprocess::SubprocessManager::production());

    // Create runners - use multiple instances since RealCommandRunner is not Clone
    let command_runner1 = execution::runner::RealCommandRunner::new();
    let command_runner2 = execution::runner::RealCommandRunner::new();
    let command_runner3 = execution::runner::RealCommandRunner::new();

    // Create base components
    let config_loader = Arc::new(ConfigLoader::new().await?);
    let worktree_manager = Arc::new(crate::worktree::WorktreeManager::new(
        project_path.to_path_buf(),
        subprocess.as_ref().clone(),
    )?);
    let session_manager = Arc::new(session::tracker::SessionTrackerImpl::new(
        format!("cook-{}", chrono::Utc::now().timestamp()),
        project_path.to_path_buf(),
    ));
    let state_manager = Arc::new(StateManager::new()?);
    let user_interaction = Arc::new(interaction::DefaultUserInteraction::new());

    // Create executors
    let command_executor = Arc::new(command_runner1);
    let claude_executor = Arc::new(execution::claude::ClaudeExecutorImpl::new(command_runner2));

    // Create analysis coordinator
    let analysis_coordinator = Arc::new(analysis::runner::AnalysisRunnerImpl::new(command_runner3));

    // Create environment coordinator
    let _environment_coordinator = Arc::new(coordinators::DefaultEnvironmentCoordinator::new(
        config_loader,
        worktree_manager,
        git_operations.clone(),
    ));

    // Create session coordinator
    let _session_coordinator = Arc::new(coordinators::DefaultSessionCoordinator::new(
        session_manager.clone(),
        state_manager.clone(),
    ));

    // Create execution coordinator
    let _execution_coordinator = Arc::new(coordinators::DefaultExecutionCoordinator::new(
        command_executor.clone(),
        claude_executor.clone(),
        subprocess.clone(),
    ));

    // Create workflow executor
    let workflow_executor: Arc<dyn workflow::WorkflowExecutor> =
        Arc::new(workflow::WorkflowExecutorImpl::new(
            claude_executor.clone(),
            session_manager.clone(),
            analysis_coordinator.clone(),
            Arc::new(metrics::collector::MetricsCollectorImpl::new(
                execution::runner::RealCommandRunner::new(),
            )),
            user_interaction.clone(),
        ));

    // Create workflow coordinator
    let _workflow_coordinator = Arc::new(coordinators::DefaultWorkflowCoordinator::new(
        workflow_executor.clone(),
        user_interaction.clone(),
    ));

    // Create metrics coordinator
    let metrics_coordinator = Arc::new(metrics::collector::MetricsCollectorImpl::new(
        execution::runner::RealCommandRunner::new(),
    ));

    // Create orchestrator with correct trait implementations
    Ok(Arc::new(DefaultCookOrchestrator::new(
        session_manager.clone(),
        command_executor.clone(),
        claude_executor.clone(),
        analysis_coordinator,
        metrics_coordinator,
        user_interaction.clone(),
        git_operations,
        StateManager::new()?,
        (*subprocess).clone(),
    )))
}

/// Load workflow configuration
async fn load_workflow(
    cmd: &CookCommand,
    _config: &crate::config::Config,
) -> Result<WorkflowConfig> {
    // Always load from playbook since it's required
    load_playbook(&cmd.playbook).await
}

/// Load workflow configuration from a playbook file
async fn load_playbook(path: &Path) -> Result<WorkflowConfig> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read playbook file: {}", path.display()))?;

    // Try to parse as YAML first, then fall back to JSON
    if path.extension().and_then(|s| s.to_str()) == Some("yml")
        || path.extension().and_then(|s| s.to_str()) == Some("yaml")
    {
        match serde_yaml::from_str::<WorkflowConfig>(&content) {
            Ok(config) => Ok(config),
            Err(e) => {
                // Try to provide more helpful error messages
                let mut error_msg = format!("Failed to parse YAML playbook: {}\n", path.display());

                // Extract line and column info if available
                if let Some(location) = e.location() {
                    error_msg.push_str(&format!(
                        "Error at line {}, column {}\n",
                        location.line(),
                        location.column()
                    ));

                    // Try to show the problematic line
                    if let Some(line) = content.lines().nth(location.line().saturating_sub(1)) {
                        error_msg.push_str(&format!("Problematic line: {line}\n"));
                        if location.column() > 0 {
                            error_msg.push_str(&format!(
                                "{}^\n",
                                " ".repeat(location.column().saturating_sub(1))
                            ));
                        }
                    }
                }

                error_msg.push_str(&format!("\nOriginal error: {e}"));

                // Add hints for common issues
                if content.contains("claude:") || content.contains("shell:") {
                    error_msg.push_str("\n\nHint: This appears to use the new workflow syntax with 'claude:' or 'shell:' commands.");
                    error_msg.push_str("\nThe workflow configuration expects 'commands:' as a list of command objects.");
                    error_msg.push_str("\nEnsure your YAML structure matches the expected format.");
                }

                Err(anyhow!(error_msg))
            }
        }
    } else {
        // Default to JSON parsing
        match serde_json::from_str::<WorkflowConfig>(&content) {
            Ok(config) => Ok(config),
            Err(e) => {
                let mut error_msg = format!("Failed to parse JSON playbook: {}\n", path.display());

                // JSON errors usually include line/column info
                error_msg.push_str(&format!("Error: {e}"));

                Err(anyhow!(error_msg))
            }
        }
    }
}

/// Legacy function for backward compatibility
/// Delegates to the new orchestrator
pub async fn run_improvement_loop(
    cmd: CookCommand,
    _session: &crate::worktree::WorktreeSession,
    _worktree_manager: &crate::worktree::WorktreeManager,
    _verbose: bool,
) -> Result<()> {
    // Simply delegate to the new cook function
    cook(cmd).await
}

#[cfg(test)]
mod cook_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_orchestrator() {
        let temp_dir = TempDir::new().unwrap();
        let orchestrator = create_orchestrator(temp_dir.path()).await.unwrap();

        // Should create orchestrator successfully - just check it exists by trying to drop it
        drop(orchestrator);
    }

    #[tokio::test]
    async fn test_load_workflow_default() {
        let temp_dir = TempDir::new().unwrap();
        let playbook_path = temp_dir.path().join("test.yml");

        // Create a simple test workflow
        let workflow_content = r#"commands:
  - "mmm-code-review"
  - name: "mmm-lint"
    focus: "performance"
"#;
        tokio::fs::write(&playbook_path, workflow_content)
            .await
            .unwrap();

        let cmd = CookCommand {
            playbook: playbook_path,
            path: None,
            max_iterations: 5,
            worktree: false,
            map: vec![],
            args: vec![],
            fail_fast: false,
            metrics: false,
            auto_accept: false,
            resume: None,
            skip_analysis: false,
        };

        let config = crate::config::Config::default();
        let workflow = load_workflow(&cmd, &config).await.unwrap();

        // Should load default workflow
        assert!(!workflow.commands.is_empty());
        assert_eq!(workflow.commands.len(), 2);
    }

    #[tokio::test]
    async fn test_yaml_error_messages() {
        let temp_dir = TempDir::new().unwrap();

        // Test case 1: Invalid YAML syntax
        let playbook_path = temp_dir.path().join("invalid.yml");
        let invalid_content = r#"commands:
  - claude: "/mmm-coverage"
    id: coverage
      commit_required: false  # Wrong indentation
"#;
        tokio::fs::write(&playbook_path, invalid_content)
            .await
            .unwrap();

        let err = load_playbook(&playbook_path).await.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("Error at line"));
        assert!(err_msg.contains("column"));
        assert!(err_msg.contains("commit_required: false"));

        // Test case 2: Wrong structure that triggers new syntax hint
        let playbook_path2 = temp_dir.path().join("new_syntax.yml");
        let new_syntax_content = r#"commands:
  - claude: "/mmm-coverage"
    outputs:
      spec:
        file_pattern: "*.md"
      invalid_field:  # Wrong field at wrong level
        something: true
"#;
        tokio::fs::write(&playbook_path2, new_syntax_content)
            .await
            .unwrap();

        let err2 = load_playbook(&playbook_path2).await.unwrap_err();
        let err_msg2 = err2.to_string();
        assert!(err_msg2.contains("claude:") || err_msg2.contains("shell:"));
    }

    #[tokio::test]
    async fn test_run_improvement_loop() {
        // Create a test playbook
        let temp_dir = TempDir::new().unwrap();
        let playbook_path = temp_dir.path().join("test.yml");

        // Create a minimal workflow
        let workflow_content = r#"commands:
  - "mmm-lint"
"#;
        tokio::fs::write(&playbook_path, workflow_content)
            .await
            .unwrap();

        // Create test command
        let cmd = CookCommand {
            playbook: playbook_path,
            path: Some(temp_dir.path().to_path_buf()),
            max_iterations: 1,
            worktree: false,
            map: vec![],
            args: vec![],
            fail_fast: false,
            metrics: false,
            auto_accept: false,
            resume: None,
            skip_analysis: false,
        };

        // Create dummy session and worktree manager (not used in the function)
        let session = crate::worktree::WorktreeSession::new(
            "test-session".to_string(),
            "test-branch".to_string(),
            temp_dir.path().to_path_buf(),
        );
        let subprocess = crate::subprocess::SubprocessManager::production();
        let worktree_manager =
            crate::worktree::WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)
                .unwrap();

        // Note: This will fail in tests because no Claude API is available
        // but we're just testing that the function delegates correctly
        let result = run_improvement_loop(cmd, &session, &worktree_manager, false).await;

        // Should fail due to missing Claude API, but that's expected
        assert!(result.is_err());
    }
}
