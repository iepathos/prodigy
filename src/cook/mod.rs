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
pub mod workflow_new;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mod_tests;

use crate::abstractions::git::RealGitOperations;
use crate::config::{workflow::WorkflowConfig, ConfigLoader};
use crate::simple_state::StateManager;
use anyhow::{Context as _, Result};
use std::path::Path;
use std::sync::Arc;

// Re-export key types
pub use command::CookCommand;
pub use orchestrator::{CookConfig, CookOrchestrator, DefaultCookOrchestrator};

/// Main entry point for cook operations
pub async fn cook(cmd: CookCommand) -> Result<()> {
    // Load configuration
    let config_loader = ConfigLoader::new().await?;
    let config = config_loader.load().await?;

    // Determine project path
    let project_path = std::env::current_dir()?;
    
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
    let claude_executor = Arc::new(execution::claude::ClaudeExecutorImpl::new(
        command_runner2
    ));
    
    // Create coordinators
    let analysis_coordinator = Arc::new(analysis::runner::AnalysisRunnerImpl::new(
        command_runner3
    ));
    let metrics_coordinator = Arc::new(metrics::collector::MetricsCollectorImpl::new(
        command_runner4
    ));
    
    // Create user interaction
    let user_interaction = Arc::new(interaction::DefaultUserInteraction::new());
    
    // Create state manager
    let state_manager = StateManager::new()?;
    
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
    )))
}

/// Load workflow configuration
async fn load_workflow(cmd: &CookCommand, _config: &crate::config::Config) -> Result<WorkflowConfig> {
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
    use std::path::PathBuf;
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
        let cmd = CookCommand {
            playbook: PathBuf::from("test.yml"),
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
    }
}