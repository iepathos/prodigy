//! Environment coordinator for managing execution environment

use crate::abstractions::git::GitOperations;
use crate::config::{ConfigLoader, WorkflowConfig};
use crate::cook::CookCommand;
use crate::worktree::WorktreeManager;
use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Environment setup result
#[derive(Debug)]
pub struct EnvironmentSetup {
    /// Working directory (may be worktree)
    pub working_dir: PathBuf,
    /// Original project directory
    pub project_dir: PathBuf,
    /// Worktree name if using worktree
    pub worktree_name: Option<String>,
    /// Loaded configuration
    pub config: crate::config::Config,
    /// Loaded workflow
    pub workflow: WorkflowConfig,
}

/// Trait for environment coordination
#[async_trait]
pub trait EnvironmentCoordinator: Send + Sync {
    /// Verify git repository state
    async fn verify_git_repository(&self, path: &Path) -> Result<()>;

    /// Load configuration and workflow
    async fn load_configuration(
        &self,
        project_path: &Path,
        command: &CookCommand,
    ) -> Result<(crate::config::Config, WorkflowConfig)>;

    /// Setup worktree if needed
    async fn setup_worktree(
        &self,
        command: &CookCommand,
        project_path: &Path,
    ) -> Result<Option<(PathBuf, String)>>;

    /// Prepare complete execution environment
    async fn prepare_environment(
        &self,
        command: &CookCommand,
        project_path: &Path,
    ) -> Result<EnvironmentSetup>;
}

/// Default implementation of environment coordinator
pub struct DefaultEnvironmentCoordinator {
    config_loader: Arc<ConfigLoader>,
    worktree_manager: Arc<WorktreeManager>,
    git_operations: Arc<dyn GitOperations>,
}

impl DefaultEnvironmentCoordinator {
    /// Create new environment coordinator
    pub fn new(
        config_loader: Arc<ConfigLoader>,
        worktree_manager: Arc<WorktreeManager>,
        git_operations: Arc<dyn GitOperations>,
    ) -> Self {
        Self {
            config_loader,
            worktree_manager,
            git_operations,
        }
    }
}

#[async_trait]
impl EnvironmentCoordinator for DefaultEnvironmentCoordinator {
    async fn verify_git_repository(&self, _path: &Path) -> Result<()> {
        if !self.git_operations.is_git_repo().await {
            return Err(anyhow::Error::msg("Not a git repository"));
        }
        Ok(())
    }

    async fn load_configuration(
        &self,
        project_path: &Path,
        command: &CookCommand,
    ) -> Result<(crate::config::Config, WorkflowConfig)> {
        // Load config
        self.config_loader
            .load_with_explicit_path(project_path, None)
            .await?;
        let config = self.config_loader.get_config();

        // Load workflow from playbook
        let workflow = crate::cook::load_playbook(&command.playbook).await?;

        Ok((config.clone(), workflow))
    }

    async fn setup_worktree(
        &self,
        command: &CookCommand,
        _project_path: &Path,
    ) -> Result<Option<(PathBuf, String)>> {
        if !command.worktree {
            return Ok(None);
        }

        // Create worktree session
        let session = self.worktree_manager.create_session().await?;

        let working_dir = session.path.clone();
        let worktree_name = session.name.clone();

        Ok(Some((working_dir, worktree_name)))
    }

    async fn prepare_environment(
        &self,
        command: &CookCommand,
        project_path: &Path,
    ) -> Result<EnvironmentSetup> {
        // Verify git repository
        self.verify_git_repository(project_path).await?;

        // Load configuration and workflow
        let (config, workflow) = self.load_configuration(project_path, command).await?;

        // Setup worktree if needed
        let (working_dir, worktree_name) =
            if let Some((dir, name)) = self.setup_worktree(command, project_path).await? {
                (dir, Some(name))
            } else {
                (project_path.to_path_buf(), None)
            };

        Ok(EnvironmentSetup {
            working_dir,
            project_dir: project_path.to_path_buf(),
            worktree_name,
            config,
            workflow,
        })
    }
}
