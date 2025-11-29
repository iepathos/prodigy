//! Pure functional checkpoint path resolution system
//!
//! This module provides type-safe, deterministic path resolution for workflow checkpoints.
//! All path resolution functions are pure (no I/O, no side effects) and return consistent
//! results for the same inputs.
//!
//! # Storage Strategies
//!
//! - **Local**: Project-local storage in `.prodigy/checkpoints/`
//!   - Use for: Testing, backwards compatibility
//!   - Example: Local checkpoints that stay within project directory
//!
//! - **Global**: Repository-scoped storage in `~/.prodigy/state/{repo}/checkpoints/`
//!   - Use for: Repository-level metadata, shared across sessions
//!   - Example: Shared checkpoint data for all sessions of a project
//!
//! - **Session**: Session-scoped storage in `~/.prodigy/state/{session_id}/checkpoints/`
//!   - Use for: Normal workflow checkpoints (recommended default)
//!   - Example: Isolated checkpoints for each workflow execution
//!
//! # Functional Design Principles
//!
//! 1. **Pure Functions**: All path resolution is deterministic with no side effects
//! 2. **Explicit Configuration**: Storage strategy is always explicit, never inferred
//! 3. **Immutability**: CheckpointStorage enum is immutable once constructed
//! 4. **Error as Values**: Returns `Result<T>` instead of panicking
//! 5. **Composition**: Small pure functions compose to build complex paths
//!
//! # Example Usage
//!
//! ```
//! use prodigy::cook::workflow::checkpoint_path::CheckpointStorage;
//!
//! // Session-scoped storage (recommended for workflows)
//! let storage = CheckpointStorage::Session {
//!     session_id: "session-abc123".to_string()
//! };
//!
//! // Resolve paths deterministically
//! let base_dir = storage.resolve_base_dir()?;
//! let checkpoint_path = storage.checkpoint_file_path("checkpoint-1")?;
//!
//! // Same inputs always produce same outputs
//! assert_eq!(
//!     storage.checkpoint_file_path("test")?,
//!     storage.checkpoint_file_path("test")?
//! );
//! # Ok::<(), anyhow::Error>(())
//! ```

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Explicit storage strategy for checkpoints
///
/// This enum makes checkpoint storage location explicit and type-safe.
/// Each variant represents a different storage strategy with different
/// trade-offs and use cases.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckpointStorage {
    /// Local project storage (.prodigy/checkpoints/)
    ///
    /// Checkpoints are stored within the project directory.
    /// Use for backwards compatibility or testing scenarios.
    Local(PathBuf),

    /// Global repository-scoped storage (~/.prodigy/state/{repo}/checkpoints/)
    ///
    /// Checkpoints are stored in global Prodigy directory, scoped by repository name.
    /// Use for repository-level metadata shared across sessions.
    Global {
        /// Repository name for scoping
        repo_name: String,
    },

    /// Session-scoped storage (~/.prodigy/state/{session_id}/checkpoints/)
    ///
    /// Checkpoints are stored in global Prodigy directory, scoped by session ID.
    /// This is the recommended default for workflow checkpoints as it provides
    /// isolation between sessions and survives worktree cleanup.
    ///
    /// NOTE: Deprecated in favor of UnifiedSession. Kept for backward compatibility.
    Session {
        /// Session ID for scoping
        session_id: String,
    },

    /// Unified session storage (~/.prodigy/sessions/{session_id}/checkpoint.json) (Spec 184)
    ///
    /// Stores checkpoint alongside UnifiedSession data for tight integration.
    /// This is the new recommended approach for all workflows.
    ///
    /// Benefits:
    /// - Co-located with session metadata
    /// - Simpler path structure
    /// - Better integration with session management
    UnifiedSession {
        /// Session ID for scoping
        session_id: String,
    },
}

impl CheckpointStorage {
    /// Pure function: resolve base directory for checkpoint storage
    ///
    /// Returns the base directory where checkpoints should be stored based on
    /// the storage strategy. This is a pure function - same inputs always produce
    /// the same output with no side effects.
    ///
    /// # Examples
    ///
    /// ```
    /// use prodigy::cook::workflow::checkpoint_path::CheckpointStorage;
    /// use std::path::PathBuf;
    ///
    /// // Local storage uses provided path directly
    /// let local = CheckpointStorage::Local(PathBuf::from("/tmp/checkpoints"));
    /// assert_eq!(local.resolve_base_dir()?, PathBuf::from("/tmp/checkpoints"));
    ///
    /// // Session storage constructs path under ~/.prodigy
    /// let session = CheckpointStorage::Session {
    ///     session_id: "test-session".to_string()
    /// };
    /// let base = session.resolve_base_dir()?;
    /// assert!(base.to_string_lossy().contains(".prodigy/state/test-session/checkpoints"));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn resolve_base_dir(&self) -> Result<PathBuf> {
        match self {
            Self::Local(path) => Ok(path.clone()),
            Self::Global { repo_name } => {
                let global_base = resolve_global_base_dir()?;
                Ok(global_base
                    .join("state")
                    .join(repo_name)
                    .join("checkpoints"))
            }
            Self::Session { session_id } => {
                let global_base = resolve_global_base_dir()?;
                Ok(global_base
                    .join("state")
                    .join(session_id)
                    .join("checkpoints"))
            }
            Self::UnifiedSession { session_id } => {
                let global_base = resolve_global_base_dir()?;
                Ok(global_base.join("sessions").join(session_id))
            }
        }
    }

    /// Pure function: construct file path for specific checkpoint
    ///
    /// Combines the base directory with the checkpoint ID to produce the full
    /// path to the checkpoint file. This is a pure function with no side effects.
    ///
    /// For UnifiedSession storage, the checkpoint is always named "checkpoint.json"
    /// regardless of the checkpoint_id parameter, as there's only one checkpoint
    /// per session in the unified model.
    ///
    /// # Examples
    ///
    /// ```
    /// use prodigy::cook::workflow::checkpoint_path::CheckpointStorage;
    ///
    /// let storage = CheckpointStorage::Session {
    ///     session_id: "session-123".to_string()
    /// };
    ///
    /// let path = storage.checkpoint_file_path("checkpoint-1")?;
    /// assert!(path.to_string_lossy().ends_with("checkpoint-1.checkpoint.json"));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn checkpoint_file_path(&self, checkpoint_id: &str) -> Result<PathBuf> {
        let base = self.resolve_base_dir()?;
        match self {
            Self::UnifiedSession { .. } => {
                // UnifiedSession always uses "checkpoint.json"
                Ok(base.join("checkpoint.json"))
            }
            _ => {
                // Other storage types use checkpoint_id
                Ok(base.join(format!("{}.checkpoint.json", checkpoint_id)))
            }
        }
    }
}

/// Pure function: get global Prodigy storage directory
///
/// Returns `~/.prodigy` as the base directory for all global Prodigy storage.
/// This is a pure function that derives the path from system home directory.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined (rare on modern systems).
///
/// # Examples
///
/// ```
/// use prodigy::cook::workflow::checkpoint_path::resolve_global_base_dir;
///
/// let base = resolve_global_base_dir()?;
/// assert!(base.to_string_lossy().ends_with(".prodigy"));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn resolve_global_base_dir() -> Result<PathBuf> {
    let base_dirs = directories::BaseDirs::new().context("Could not determine home directory")?;
    Ok(base_dirs.home_dir().join(".prodigy"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_local_storage_uses_provided_path() {
        let custom_path = PathBuf::from("/tmp/checkpoints");
        let storage = CheckpointStorage::Local(custom_path.clone());

        let base = storage.resolve_base_dir().unwrap();
        assert_eq!(base, custom_path);
    }

    #[test]
    fn test_session_storage_path_resolution() {
        let storage = CheckpointStorage::Session {
            session_id: "test-session-123".to_string(),
        };

        let base = storage.resolve_base_dir().unwrap();
        assert!(base
            .to_string_lossy()
            .ends_with(".prodigy/state/test-session-123/checkpoints"));

        let file = storage.checkpoint_file_path("checkpoint-1").unwrap();
        assert!(file
            .to_string_lossy()
            .ends_with("checkpoint-1.checkpoint.json"));
    }

    #[test]
    fn test_global_storage_path_resolution() {
        let storage = CheckpointStorage::Global {
            repo_name: "my-repo".to_string(),
        };

        let base = storage.resolve_base_dir().unwrap();
        assert!(base
            .to_string_lossy()
            .ends_with(".prodigy/state/my-repo/checkpoints"));
    }

    #[test]
    fn test_unified_session_storage_path_resolution() {
        let storage = CheckpointStorage::UnifiedSession {
            session_id: "test-session-456".to_string(),
        };

        let base = storage.resolve_base_dir().unwrap();
        assert!(base
            .to_string_lossy()
            .ends_with(".prodigy/sessions/test-session-456"));

        let file = storage.checkpoint_file_path("ignored-id").unwrap();
        assert!(file.to_string_lossy().ends_with("/checkpoint.json"));
        assert!(!file.to_string_lossy().contains("ignored-id"));
    }

    #[test]
    fn test_unified_session_always_uses_checkpoint_json() {
        let storage = CheckpointStorage::UnifiedSession {
            session_id: "test-session".to_string(),
        };

        // Regardless of checkpoint_id, should always be "checkpoint.json"
        let path1 = storage.checkpoint_file_path("checkpoint-1").unwrap();
        let path2 = storage.checkpoint_file_path("checkpoint-2").unwrap();
        let path3 = storage.checkpoint_file_path("any-id").unwrap();

        assert_eq!(path1, path2);
        assert_eq!(path2, path3);
        assert!(path1.to_string_lossy().ends_with("/checkpoint.json"));
    }

    #[test]
    fn test_path_resolution_is_deterministic() {
        let storage = CheckpointStorage::Session {
            session_id: "session-abc".to_string(),
        };

        let path1 = storage.checkpoint_file_path("test").unwrap();
        let path2 = storage.checkpoint_file_path("test").unwrap();

        assert_eq!(path1, path2, "Same inputs must produce same path");
    }

    #[test]
    fn test_checkpoint_file_path_includes_checkpoint_id() {
        let storage = CheckpointStorage::Session {
            session_id: "test-session".to_string(),
        };

        let path = storage.checkpoint_file_path("my-checkpoint").unwrap();
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("my-checkpoint"));
        assert!(path.to_string_lossy().ends_with(".checkpoint.json"));
    }

    #[test]
    fn test_different_session_ids_produce_different_paths() {
        let storage1 = CheckpointStorage::Session {
            session_id: "session-1".to_string(),
        };
        let storage2 = CheckpointStorage::Session {
            session_id: "session-2".to_string(),
        };

        let path1 = storage1.checkpoint_file_path("test").unwrap();
        let path2 = storage2.checkpoint_file_path("test").unwrap();

        assert_ne!(
            path1, path2,
            "Different session IDs must produce different paths"
        );
    }

    #[test]
    fn test_global_base_dir_contains_prodigy() {
        let base = resolve_global_base_dir().unwrap();
        assert!(base.to_string_lossy().ends_with(".prodigy"));
    }

    #[test]
    fn test_storage_equality() {
        let storage1 = CheckpointStorage::Session {
            session_id: "test".to_string(),
        };
        let storage2 = CheckpointStorage::Session {
            session_id: "test".to_string(),
        };
        let storage3 = CheckpointStorage::Session {
            session_id: "different".to_string(),
        };

        assert_eq!(storage1, storage2);
        assert_ne!(storage1, storage3);
    }

    // Property-based tests using proptest to verify path resolution invariants

    proptest! {
        /// Property: Same storage strategy and checkpoint ID always produce the same path
        /// This verifies deterministic path resolution across arbitrary inputs
        #[test]
        fn prop_same_strategy_same_path(
            session_id in "[a-zA-Z0-9-]{1,50}",
            checkpoint_id in "[a-zA-Z0-9-]{1,50}"
        ) {
            let storage = CheckpointStorage::Session {
                session_id: session_id.clone(),
            };

            let path1 = storage.checkpoint_file_path(&checkpoint_id).unwrap();
            let path2 = storage.checkpoint_file_path(&checkpoint_id).unwrap();

            prop_assert_eq!(path1, path2, "Same inputs must always produce same path");
        }

        /// Property: Different session IDs always produce different paths
        /// This verifies proper isolation between sessions
        #[test]
        fn prop_different_sessions_different_paths(
            session_id1 in "[a-zA-Z0-9-]{1,50}",
            session_id2 in "[a-zA-Z0-9-]{1,50}",
            checkpoint_id in "[a-zA-Z0-9-]{1,50}"
        ) {
            prop_assume!(session_id1 != session_id2);

            let storage1 = CheckpointStorage::Session {
                session_id: session_id1,
            };
            let storage2 = CheckpointStorage::Session {
                session_id: session_id2,
            };

            let path1 = storage1.checkpoint_file_path(&checkpoint_id).unwrap();
            let path2 = storage2.checkpoint_file_path(&checkpoint_id).unwrap();

            prop_assert_ne!(path1, path2, "Different session IDs must produce different paths");
        }

        /// Property: Checkpoint file paths always end with .checkpoint.json
        /// This verifies consistent file naming conventions
        #[test]
        fn prop_checkpoint_paths_have_correct_extension(
            session_id in "[a-zA-Z0-9-]{1,50}",
            checkpoint_id in "[a-zA-Z0-9-]{1,50}"
        ) {
            let storage = CheckpointStorage::Session { session_id };
            let path = storage.checkpoint_file_path(&checkpoint_id).unwrap();

            prop_assert!(
                path.to_string_lossy().ends_with(".checkpoint.json"),
                "Checkpoint paths must end with .checkpoint.json"
            );
        }

        /// Property: Checkpoint ID is preserved in the file name
        /// This verifies that checkpoint IDs are properly included in paths
        #[test]
        fn prop_checkpoint_id_in_path(
            session_id in "[a-zA-Z0-9-]{1,50}",
            checkpoint_id in "[a-zA-Z0-9-]{1,50}"
        ) {
            let storage = CheckpointStorage::Session { session_id };
            let path = storage.checkpoint_file_path(&checkpoint_id).unwrap();

            prop_assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains(&checkpoint_id),
                "Checkpoint ID must be in the file name"
            );
        }

        /// Property: Global storage paths always contain repo name
        /// This verifies proper repository scoping
        #[test]
        fn prop_global_storage_contains_repo_name(
            repo_name in "[a-zA-Z0-9-]{1,50}"
        ) {
            let storage = CheckpointStorage::Global {
                repo_name: repo_name.clone(),
            };
            let base = storage.resolve_base_dir().unwrap();

            prop_assert!(
                base.to_string_lossy().contains(&repo_name),
                "Global storage paths must contain repo name"
            );
        }

        /// Property: Session storage paths always contain session ID
        /// This verifies proper session scoping
        #[test]
        fn prop_session_storage_contains_session_id(
            session_id in "[a-zA-Z0-9-]{1,50}"
        ) {
            let storage = CheckpointStorage::Session {
                session_id: session_id.clone(),
            };
            let base = storage.resolve_base_dir().unwrap();

            prop_assert!(
                base.to_string_lossy().contains(&session_id),
                "Session storage paths must contain session ID"
            );
        }

        /// Property: Local storage always returns the exact path provided
        /// This verifies pure function behavior for local storage
        #[test]
        fn prop_local_storage_returns_exact_path(
            path_str in "[a-zA-Z0-9/_-]{1,100}"
        ) {
            let expected_path = PathBuf::from(path_str);
            let storage = CheckpointStorage::Local(expected_path.clone());
            let resolved = storage.resolve_base_dir().unwrap();

            prop_assert_eq!(resolved, expected_path, "Local storage must return exact path");
        }
    }
}
