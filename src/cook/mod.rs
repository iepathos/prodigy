//! Cook module - orchestrates improvement sessions
//!
//! This module has been refactored to use a component-based architecture
//! with dependency injection for improved testability and maintainability.

pub mod analysis;
pub mod command;
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

    // Create runners - use multiple instances since RealCommandRunner is not Clone
    let command_runner1 = execution::runner::RealCommandRunner::new();
    let command_runner2 = execution::runner::RealCommandRunner::new();
    let command_runner3 = execution::runner::RealCommandRunner::new();
    let command_runner4 = execution::runner::RealCommandRunner::new();

    // Create session manager
    let session_manager = Arc::new(session::tracker::SessionTrackerImpl::new(
        format!("cook-{}", chrono::Utc::now().timestamp()),
        project_path.to_path_buf(),
    ));

    // Create executors
    let command_executor = Arc::new(command_runner1);
    let claude_executor = Arc::new(execution::claude::ClaudeExecutorImpl::new(command_runner2));

    // Create coordinators
    let analysis_coordinator = Arc::new(analysis::runner::AnalysisRunnerImpl::new(command_runner3));
    let metrics_coordinator = Arc::new(metrics::collector::MetricsCollectorImpl::new(
        command_runner4,
    ));

    // Create user interaction
    let user_interaction = Arc::new(interaction::DefaultUserInteraction::new());

    // Create state manager
    let state_manager = StateManager::new()?;

    // Create subprocess manager
    let subprocess = crate::subprocess::SubprocessManager::production();

    // Create orchestrator
    Ok(Arc::new(DefaultCookOrchestrator::new(
        session_manager,
        command_executor,
        claude_executor,
        analysis_coordinator,
        metrics_coordinator,
        user_interaction,
        git_operations,
        state_manager,
        subprocess,
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
        serde_yaml::from_str(&content)
            .context(format!("Failed to parse YAML playbook: {}", path.display()))
    } else {
        // Default to JSON parsing
        serde_json::from_str(&content)
            .context(format!("Failed to parse JSON playbook: {}", path.display()))
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
            focus: None,
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
}
