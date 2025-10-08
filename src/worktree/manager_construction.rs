//! Construction and initialization functions for WorktreeManager
//!
//! This module contains constructor functions, builders, and test helpers
//! for creating and initializing WorktreeManager instances.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::builder::WorktreeBuilder;
use super::{Checkpoint, WorktreeManager};
use crate::config::mapreduce::MergeWorkflow;
use crate::subprocess::SubprocessManager;

// ============================================================================
// Constructors and Builders
// ============================================================================

impl WorktreeManager {
    /// Create a new WorktreeManager for the given repository
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `subprocess` - Subprocess manager for git operations
    ///
    /// # Returns
    /// * `Result<Self>` - WorktreeManager instance or error
    ///
    /// # Errors
    /// Returns error if:
    /// - Repository path is invalid
    /// - Git repository is not found
    pub fn new(repo_path: PathBuf, subprocess: SubprocessManager) -> Result<Self> {
        Self::with_config(repo_path, subprocess, 0, None, HashMap::new())
    }

    /// Create a new WorktreeManager with configuration
    pub fn with_config(
        repo_path: PathBuf,
        subprocess: SubprocessManager,
        verbosity: u8,
        custom_merge_workflow: Option<MergeWorkflow>,
        workflow_env: HashMap<String, String>,
    ) -> Result<Self> {
        WorktreeBuilder::new(repo_path, subprocess)
            .verbosity(verbosity)
            .custom_merge_workflow(custom_merge_workflow)
            .workflow_env(workflow_env)
            .build()
    }

    /// Create a checkpoint for the current state
    pub fn create_checkpoint(&self, session_name: &str, checkpoint: Checkpoint) -> Result<()> {
        self.update_session_state(session_name, |state| {
            state.last_checkpoint = Some(checkpoint);
            state.resumable = true;
        })
    }
}
