//! Construction and builder logic for WorktreeManager
//!
//! This module contains all construction-related functionality including:
//! - WorktreeManager initialization and configuration
//! - Worktree session creation
//! - Command builders for git operations
//! - Factory methods for executors and checkpoint managers

use crate::config::mapreduce::MergeWorkflow;
use crate::cook::execution::ClaudeExecutorImpl;
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

use super::state::{IterationInfo, WorktreeStats};
use super::{WorktreeManager, WorktreeSession, WorktreeState, WorktreeStatus};

/// Builder for constructing WorktreeManager instances
///
/// This builder provides a fluent interface for configuring and creating
/// WorktreeManager instances with various options.
pub struct WorktreeBuilder {
    repo_path: PathBuf,
    subprocess: SubprocessManager,
    verbosity: u8,
    custom_merge_workflow: Option<MergeWorkflow>,
    workflow_env: HashMap<String, String>,
}

impl WorktreeBuilder {
    /// Create a new WorktreeBuilder with default settings
    pub fn new(repo_path: PathBuf, subprocess: SubprocessManager) -> Self {
        Self {
            repo_path,
            subprocess,
            verbosity: 0,
            custom_merge_workflow: None,
            workflow_env: HashMap::new(),
        }
    }

    /// Set the verbosity level
    pub fn verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Set a custom merge workflow
    pub fn custom_merge_workflow(mut self, workflow: Option<MergeWorkflow>) -> Self {
        self.custom_merge_workflow = workflow;
        self
    }

    /// Set workflow environment variables
    pub fn workflow_env(mut self, env: HashMap<String, String>) -> Self {
        self.workflow_env = env;
        self
    }

    /// Build the WorktreeManager instance
    pub fn build(self) -> Result<WorktreeManager> {
        // Get the repository name from the path
        let repo_name = self
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                anyhow!(
                    "Could not determine repository name from path: {}",
                    self.repo_path.display()
                )
            })?;

        // Use home directory for worktrees (or temp dir during tests)
        let base_dir = {
            #[cfg(test)]
            {
                use std::sync::OnceLock;
                static TEST_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();
                let test_dir = TEST_DIR.get_or_init(|| {
                    let temp =
                        std::env::temp_dir().join(format!("prodigy-test-{}", std::process::id()));
                    std::fs::create_dir_all(&temp).unwrap();
                    temp
                });
                test_dir.join("worktrees").join(repo_name)
            }
            #[cfg(not(test))]
            {
                let home_dir = directories::BaseDirs::new()
                    .ok_or_else(|| anyhow!("Could not determine base directories"))?
                    .home_dir()
                    .to_path_buf();
                home_dir.join(".prodigy").join("worktrees").join(repo_name)
            }
        };

        std::fs::create_dir_all(&base_dir).context("Failed to create worktree base directory")?;

        // Create .gitignore if it doesn't exist
        let gitignore_path = base_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, ".metadata/\n")?;
        }

        // Try to canonicalize paths to handle symlinks (e.g., /private/var vs /var on macOS)
        // If canonicalization fails (e.g., on certain filesystems), use the original paths
        let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
        let repo_path = self.repo_path.canonicalize().unwrap_or(self.repo_path);

        Ok(WorktreeManager {
            base_dir,
            repo_path,
            subprocess: self.subprocess,
            verbosity: self.verbosity,
            custom_merge_workflow: self.custom_merge_workflow,
            workflow_env: self.workflow_env,
        })
    }
}

/// Session creation functions
impl WorktreeManager {
    /// Create a new worktree session
    ///
    /// # Returns
    /// * `Result<WorktreeSession>` - The created worktree session
    ///
    /// # Errors
    /// Returns error if worktree creation fails
    pub async fn create_session(&self) -> Result<WorktreeSession> {
        let session_id = Uuid::new_v4();
        // Simple name using UUID
        let name = format!("session-{session_id}");

        self.create_session_with_id(&name).await
    }

    /// Create a new worktree session with a specific session ID
    ///
    /// # Arguments
    /// * `session_id` - The session ID to use (should be in "session-{uuid}" format)
    ///
    /// # Returns
    /// * `Result<WorktreeSession>` - The created worktree session
    ///
    /// # Errors
    /// Returns error if worktree creation fails
    pub async fn create_session_with_id(&self, session_id: &str) -> Result<WorktreeSession> {
        // Capture current branch BEFORE creating worktree
        let mut original_branch = self.get_current_branch().await.unwrap_or_else(|e| {
            warn!(
                "Failed to detect current branch: {}, will use default for merge",
                e
            );
            String::from("HEAD")
        });

        // If in detached HEAD state, use default branch instead
        if original_branch == "HEAD" {
            original_branch = self.determine_default_branch().await.unwrap_or_else(|e| {
                warn!("Failed to determine default branch: {}, using master", e);
                String::from("master")
            });
            info!(
                "Detached HEAD detected, using default branch: {}",
                original_branch
            );
        }

        info!("Creating worktree from branch: {}", original_branch);

        // Use the provided session ID as the name
        let name = session_id.to_string();
        let branch = format!("prodigy-{name}");
        let worktree_path = self.base_dir.join(&name);

        // Create worktree
        let command = ProcessCommandBuilder::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "add", "-b", &branch])
            .arg(worktree_path.to_string_lossy().as_ref())
            .build();

        let output = self
            .subprocess
            .runner()
            .run(command)
            .await
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            anyhow::bail!("Failed to create worktree: {}", output.stderr);
        }

        // Create session
        let session = WorktreeSession::new(name.clone(), branch, worktree_path);

        // Save session state with original branch
        self.save_session_state_with_original_branch(&session, &original_branch)?;

        Ok(session)
    }

    /// Save session state with original branch tracking
    pub(crate) fn save_session_state_with_original_branch(
        &self,
        session: &WorktreeSession,
        original_branch: &str,
    ) -> Result<()> {
        let state_dir = self.base_dir.join(".metadata");
        fs::create_dir_all(&state_dir)?;

        let state_file = state_dir.join(format!("{}.json", session.name));
        let state = WorktreeState {
            session_id: session.name.clone(),
            worktree_name: session.name.clone(),
            branch: session.branch.clone(),
            original_branch: original_branch.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 0,
                max: 10,
            },
            stats: WorktreeStats::default(),
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: None,
            resumable: false,
        };

        let json = serde_json::to_string_pretty(&state)?;

        // Write to temp file first, then rename atomically
        let temp_file = state_dir.join(format!("{}.json.tmp", session.name));
        fs::write(&temp_file, &json)?;
        fs::rename(&temp_file, &state_file)?;

        Ok(())
    }

    /// Create a worktree session from an existing worktree path and branch
    pub(crate) fn create_worktree_session(
        &self,
        path: PathBuf,
        branch: String,
    ) -> Option<WorktreeSession> {
        let canonical_path = path.canonicalize().unwrap_or(path.clone());

        // Include all worktrees in our base directory, regardless of branch name
        // This includes MapReduce branches like "merge-prodigy-*" and "prodigy-agent-*"
        if !canonical_path.starts_with(&self.base_dir) {
            return None;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&branch)
            .to_string();

        Some(WorktreeSession::new(name, branch, canonical_path))
    }
}

/// Pure command builder functions
///
/// These are static/pure functions that build git commands without side effects
impl WorktreeManager {
    /// Build a git command to check if a branch exists
    pub(crate) fn build_branch_check_command(
        repo_path: &Path,
        branch: &str,
    ) -> crate::subprocess::ProcessCommand {
        ProcessCommandBuilder::new("git")
            .current_dir(repo_path)
            .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
            .build()
    }

    /// Build a git command to get commit count between branches
    pub(crate) fn build_commit_diff_command(
        repo_path: &Path,
        target_branch: &str,
        worktree_branch: &str,
    ) -> crate::subprocess::ProcessCommand {
        ProcessCommandBuilder::new("git")
            .current_dir(repo_path)
            .args([
                "rev-list",
                "--count",
                &format!("{}..{}", target_branch, worktree_branch),
            ])
            .build()
    }

    /// Build a git command to check merged branches
    pub(crate) fn build_merge_check_command(
        repo_path: &Path,
        target_branch: &str,
    ) -> crate::subprocess::ProcessCommand {
        ProcessCommandBuilder::new("git")
            .current_dir(repo_path)
            .args(["branch", "--merged", target_branch])
            .build()
    }

    /// Build Claude environment variables for automation
    pub(crate) fn build_claude_environment_variables(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        if self.verbosity >= 1 {
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
        }

        if std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").unwrap_or_default() == "true" {
            env_vars.insert(
                "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
                "true".to_string(),
            );
        }

        env_vars
    }
}

/// Factory methods for creating executors and managers
impl WorktreeManager {
    /// Create a Claude executor instance
    pub(crate) fn create_claude_executor(
        &self,
    ) -> ClaudeExecutorImpl<crate::cook::execution::runner::RealCommandRunner> {
        use crate::cook::execution::runner::RealCommandRunner;
        let command_runner = RealCommandRunner::new();
        ClaudeExecutorImpl::new(command_runner).with_verbosity(self.verbosity)
    }

    /// Create a checkpoint manager for merge operations
    pub(crate) fn create_merge_checkpoint_manager(
        &self,
    ) -> Result<crate::cook::workflow::checkpoint::CheckpointManager> {
        use crate::storage::{extract_repo_name, GlobalStorage};
        use std::fs;

        let storage = GlobalStorage::new()
            .map_err(|e| anyhow::anyhow!("Failed to create global storage: {}", e))?;

        let repo_name = extract_repo_name(&self.repo_path)
            .map_err(|e| anyhow::anyhow!("Failed to extract repository name: {}", e))?;

        // Create checkpoint directory synchronously
        let checkpoint_dir = storage
            .base_dir()
            .join("state")
            .join(&repo_name)
            .join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).context("Failed to create checkpoint directory")?;

        use crate::cook::workflow::checkpoint_path::CheckpointStorage;

        #[allow(deprecated)]
        let manager = crate::cook::workflow::checkpoint::CheckpointManager::with_storage(
            CheckpointStorage::Local(checkpoint_dir),
        );
        Ok(manager)
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    //! Test helper functions for worktree testing
    //!
    //! These helpers are used throughout the worktree test suite to set up
    //! test environments, create test data, and verify test conditions.

    use super::*;
    use crate::subprocess::SubprocessManager;
    use tempfile::TempDir;

    /// Set up a test git repository with initial commit
    #[allow(dead_code)]
    pub async fn setup_test_git_repo(
        temp_dir: &TempDir,
        subprocess: &SubprocessManager,
    ) -> anyhow::Result<()> {
        // Initialize a git repository
        let init_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .build();
        subprocess.runner().run(init_command).await?;

        // Configure user for git (needed for commits)
        let config_name = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.name", "Test User"])
            .build();
        subprocess.runner().run(config_name).await?;

        let config_email = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.email", "test@example.com"])
            .build();
        subprocess.runner().run(config_email).await?;

        // Create initial commit (required for worktrees)
        let initial_file = temp_dir.path().join("README.md");
        std::fs::write(&initial_file, "# Test Repository")?;

        let add_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .build();
        subprocess.runner().run(add_command).await?;

        let commit_command = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial commit"])
            .build();
        subprocess.runner().run(commit_command).await?;

        Ok(())
    }

    /// Set up a test WorktreeManager with initialized git repo
    #[allow(dead_code)]
    pub async fn setup_test_worktree_manager(
        temp_dir: &TempDir,
    ) -> anyhow::Result<WorktreeManager> {
        let subprocess = SubprocessManager::production();
        setup_test_git_repo(temp_dir, &subprocess).await?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

        // Create metadata directory
        let metadata_dir = manager.base_dir.join(".metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        Ok(manager)
    }

    /// Create a test worktree state with checkpoint
    #[allow(dead_code)]
    pub fn create_test_worktree_state_with_checkpoint(
        session_id: &str,
        iteration: u32,
        command: &str,
    ) -> WorktreeState {
        use crate::worktree::Checkpoint;

        WorktreeState {
            session_id: session_id.to_string(),
            worktree_name: session_id.to_string(),
            branch: "test-branch".to_string(),
            original_branch: String::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: WorktreeStatus::InProgress,
            iterations: IterationInfo {
                completed: 0,
                max: 5,
            },
            stats: WorktreeStats {
                files_changed: 0,
                commits: 0,
                last_commit_sha: None,
            },
            merged: false,
            merged_at: None,
            error: None,
            merge_prompt_shown: false,
            merge_prompt_response: None,
            interrupted_at: None,
            interruption_type: None,
            last_checkpoint: Some(Checkpoint {
                iteration,
                timestamp: chrono::Utc::now(),
                last_command: command.to_string(),
                last_command_type: crate::worktree::CommandType::CodeReview,
                last_spec_id: Some("spec-123".to_string()),
                files_modified: vec!["src/main.rs".to_string()],
                command_output: None,
            }),
            resumable: true,
        }
    }

    /// Create a test session state JSON
    #[allow(dead_code)]
    pub fn create_test_session_state(
        session_id: &str,
        status: &str,
        hours_ago: i64,
        minutes_ago: i64,
        files_changed: u32,
        commits: u32,
        error_msg: Option<&str>,
    ) -> serde_json::Value {
        serde_json::json!({
            "session_id": session_id,
            "status": status,
            "branch": format!("feature-{}", session_id.split('-').next_back().unwrap_or("1")),
            "created_at": (chrono::Utc::now() - chrono::Duration::hours(hours_ago)).to_rfc3339(),
            "updated_at": (chrono::Utc::now() - chrono::Duration::minutes(minutes_ago)).to_rfc3339(),
            "error": error_msg,
            "stats": {
                "files_changed": files_changed,
                "commits": commits,
                "last_commit_sha": null
            },
            "worktree_name": session_id,
            "iterations": { "completed": 0, "max": 5 },
            "merged": false,
            "merged_at": null,
            "merge_prompt_shown": false,
            "merge_prompt_response": null,
            "interrupted_at": null,
            "interruption_type": null,
            "last_checkpoint": null,
            "resumable": false,
            "original_branch": ""
        })
    }

    /// Create mock worktree directories for testing
    #[allow(dead_code)]
    pub fn create_mock_worktree_dirs(
        manager: &WorktreeManager,
        session_ids: &[&str],
    ) -> anyhow::Result<()> {
        for session_id in session_ids {
            let wt_dir = manager.base_dir.join(session_id);
            std::fs::create_dir_all(&wt_dir)?;
            // Create minimal .git file to make it appear as valid worktree
            std::fs::write(wt_dir.join(".git"), "gitdir: /fake/path")?;
        }
        Ok(())
    }

    /// Create a test worktree with session state
    #[allow(dead_code)]
    pub async fn create_test_worktree_with_session_state(
        manager: &WorktreeManager,
        temp_dir: &TempDir,
        session_id: &str,
        branch: &str,
        session_state: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let wt_dir = manager.base_dir.join(session_id);
        let subprocess = SubprocessManager::production();

        let add_worktree = ProcessCommandBuilder::new("git")
            .current_dir(temp_dir.path())
            .args([
                "worktree",
                "add",
                "-b",
                branch,
                wt_dir.to_string_lossy().as_ref(),
            ])
            .build();
        subprocess.runner().run(add_worktree).await?;

        let prodigy_dir = wt_dir.join(".prodigy");
        std::fs::create_dir_all(&prodigy_dir)?;

        let session_state_file = prodigy_dir.join("session_state.json");
        std::fs::write(&session_state_file, serde_json::to_string(session_state)?)?;

        Ok(())
    }
}
