//! Session configuration types

use crate::config::workflow::WorkflowConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub project_path: PathBuf,
    pub workflow: WorkflowConfig,
    pub execution_mode: ExecutionMode,
    pub max_iterations: u32,
    pub focus: Option<String>,
    pub options: SessionOptions,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            project_path: PathBuf::from("."),
            workflow: WorkflowConfig {
                commands: vec![],
            },
            execution_mode: ExecutionMode::Direct,
            max_iterations: 10,
            focus: None,
            options: SessionOptions::default(),
        }
    }
}

/// Execution mode for sessions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionMode {
    /// Direct execution in current directory
    Direct,
    /// Execution in a git worktree
    Worktree { name: String },
}

impl ExecutionMode {
    /// Check if this is worktree mode
    pub fn is_worktree(&self) -> bool {
        matches!(self, ExecutionMode::Worktree { .. })
    }

    /// Get worktree name if applicable
    pub fn worktree_name(&self) -> Option<&str> {
        match self {
            ExecutionMode::Worktree { name } => Some(name),
            ExecutionMode::Direct => None,
        }
    }
}

/// Session runtime options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOptions {
    pub fail_fast: bool,
    pub auto_merge: bool,
    pub collect_metrics: bool,
    pub verbose: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            fail_fast: false,
            auto_merge: false,
            collect_metrics: false,
            verbose: false,
        }
    }
}

impl SessionOptions {
    /// Create options from command flags
    pub fn from_flags(fail_fast: bool, auto_accept: bool, metrics: bool, verbose: bool) -> Self {
        Self {
            fail_fast,
            auto_merge: auto_accept,
            collect_metrics: metrics,
            verbose,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode() {
        let mode = ExecutionMode::Direct;
        assert!(!mode.is_worktree());
        assert_eq!(mode.worktree_name(), None);

        let mode = ExecutionMode::Worktree {
            name: "test-wt".to_string(),
        };
        assert!(mode.is_worktree());
        assert_eq!(mode.worktree_name(), Some("test-wt"));
    }

    #[test]
    fn test_session_options() {
        let options = SessionOptions::from_flags(true, false, true, false);
        assert!(options.fail_fast);
        assert!(!options.auto_merge);
        assert!(options.collect_metrics);
        assert!(!options.verbose);
    }
}