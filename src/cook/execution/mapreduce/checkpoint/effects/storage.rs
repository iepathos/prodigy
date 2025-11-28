//! Checkpoint storage effects
//!
//! This module provides Effect-based operations for checkpoint storage.
//! Effects encapsulate I/O operations and enable composition, testing, and
//! dependency injection via the Reader pattern.

use crate::cook::execution::mapreduce::checkpoint::pure::preparation;
use crate::cook::execution::mapreduce::checkpoint::{
    CheckpointReason, CheckpointStorage, MapReduceCheckpoint,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Environment for checkpoint storage effects
pub trait CheckpointStorageEnv: Clone + Send + Sync {
    /// Get the checkpoint storage implementation
    fn storage(&self) -> Arc<dyn CheckpointStorage>;

    /// Get the current checkpoint state
    fn current_checkpoint(&self) -> Arc<RwLock<Option<MapReduceCheckpoint>>>;

    /// Get the storage path for checkpoints
    fn storage_path(&self) -> PathBuf;
}

/// Error during checkpoint storage operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum CheckpointStorageError {
    #[error("No checkpoint to save")]
    NoCheckpoint,

    #[error("Failed to save checkpoint: {0}")]
    SaveFailed(String),

    #[error("Failed to load checkpoint: {0}")]
    LoadFailed(String),

    #[error("Checkpoint not found: {0}")]
    NotFound(String),
}

/// Save the current checkpoint
///
/// This function:
/// 1. Reads the current checkpoint from state
/// 2. Prepares it for saving (update timestamps, reset in-progress items)
/// 3. Saves to storage
pub async fn save_checkpoint<E: CheckpointStorageEnv>(
    env: &E,
    reason: CheckpointReason,
) -> Result<String, CheckpointStorageError> {
    let storage = env.storage();
    let current_checkpoint = env.current_checkpoint();

    // Read current checkpoint
    let checkpoint_guard = current_checkpoint.read().await;
    let checkpoint = match checkpoint_guard.as_ref() {
        Some(cp) => cp.clone(),
        None => return Err(CheckpointStorageError::NoCheckpoint),
    };
    drop(checkpoint_guard);

    // Prepare checkpoint for saving (pure function)
    let prepared = preparation::prepare_checkpoint(&checkpoint, reason);
    let checkpoint_id = prepared.metadata.checkpoint_id.clone();

    // Save to storage
    storage
        .save_checkpoint(&prepared)
        .await
        .map_err(|e| CheckpointStorageError::SaveFailed(e.to_string()))?;

    Ok(checkpoint_id)
}

/// Load a checkpoint by ID
pub async fn load_checkpoint<E: CheckpointStorageEnv>(
    env: &E,
    checkpoint_id: String,
) -> Result<MapReduceCheckpoint, CheckpointStorageError> {
    use crate::cook::execution::mapreduce::checkpoint::CheckpointId;

    let storage = env.storage();
    let id = CheckpointId::from_string(checkpoint_id);

    storage
        .load_checkpoint(&id)
        .await
        .map_err(|e| CheckpointStorageError::LoadFailed(e.to_string()))
}

/// Update the current checkpoint state
pub async fn update_checkpoint_state<E: CheckpointStorageEnv>(
    env: &E,
    checkpoint: MapReduceCheckpoint,
) -> Result<(), CheckpointStorageError> {
    let current_checkpoint = env.current_checkpoint();
    let mut guard = current_checkpoint.write().await;
    *guard = Some(checkpoint);
    Ok(())
}

/// Check if a checkpoint should be created
///
/// Uses the checkpoint trigger configuration and state
/// to determine if a new checkpoint should be saved.
pub fn should_save_checkpoint(
    items_since_last: usize,
    last_checkpoint_time: chrono::DateTime<chrono::Utc>,
    config: &super::super::pure::triggers::CheckpointTriggerConfig,
) -> bool {
    use super::super::pure::triggers::should_checkpoint;

    should_checkpoint(
        items_since_last,
        last_checkpoint_time,
        chrono::Utc::now(),
        config,
    )
}

// Effect-based wrappers for composition with stillwater
// Note: These are placeholder functions for future Effect integration.
// The async functions above can be used directly until Effect patterns are finalized.

/// Create an effect-like wrapper that saves the current checkpoint
/// This is a convenience function that returns a future.
pub async fn save_checkpoint_effect<E: CheckpointStorageEnv + Clone + 'static>(
    env: E,
    reason: CheckpointReason,
) -> Result<String, CheckpointStorageError> { save_checkpoint(&env, reason).await }

/// Create an effect-like wrapper that loads a checkpoint by ID
/// This is a convenience function that returns a future.
pub async fn load_checkpoint_effect<E: CheckpointStorageEnv + Clone + 'static>(
    env: E,
    checkpoint_id: String,
) -> Result<MapReduceCheckpoint, CheckpointStorageError> { load_checkpoint(&env, checkpoint_id).await }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::checkpoint::{
        FileCheckpointStorage, MapReduceCheckpoint, PhaseType,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Mock environment for testing
    #[derive(Clone)]
    struct MockStorageEnv {
        storage: Arc<dyn CheckpointStorage>,
        current_checkpoint: Arc<RwLock<Option<MapReduceCheckpoint>>>,
        storage_path: PathBuf,
    }

    impl CheckpointStorageEnv for MockStorageEnv {
        fn storage(&self) -> Arc<dyn CheckpointStorage> {
            Arc::clone(&self.storage)
        }

        fn current_checkpoint(&self) -> Arc<RwLock<Option<MapReduceCheckpoint>>> {
            Arc::clone(&self.current_checkpoint)
        }

        fn storage_path(&self) -> PathBuf {
            self.storage_path.clone()
        }
    }

    fn create_test_env(temp_dir: &TempDir) -> MockStorageEnv {
        let storage_path = temp_dir.path().to_path_buf();
        let storage: Arc<dyn CheckpointStorage> =
            Arc::new(FileCheckpointStorage::new(storage_path.clone(), true));
        let checkpoint =
            crate::cook::execution::mapreduce::checkpoint::pure::preparation::create_initial_checkpoint(
                "test-job",
                10,
                PhaseType::Map,
            );

        MockStorageEnv {
            storage,
            current_checkpoint: Arc::new(RwLock::new(Some(checkpoint))),
            storage_path,
        }
    }

    #[tokio::test]
    async fn test_save_checkpoint_no_checkpoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut env = create_test_env(&temp_dir);
        env.current_checkpoint = Arc::new(RwLock::new(None));

        let result = save_checkpoint(&env, CheckpointReason::Interval).await;

        assert!(matches!(result, Err(CheckpointStorageError::NoCheckpoint)));
    }

    #[tokio::test]
    async fn test_save_checkpoint_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let env = create_test_env(&temp_dir);

        let result = save_checkpoint(&env, CheckpointReason::Interval).await;

        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("cp-"));
    }

    #[tokio::test]
    async fn test_update_checkpoint_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let env = create_test_env(&temp_dir);

        // Create a new checkpoint to update
        let new_checkpoint =
            crate::cook::execution::mapreduce::checkpoint::pure::preparation::create_initial_checkpoint(
                "new-job",
                20,
                PhaseType::Reduce,
            );

        let result = update_checkpoint_state(&env, new_checkpoint).await;

        assert!(result.is_ok());

        // Verify state was updated
        let guard = env.current_checkpoint.read().await;
        let checkpoint = guard.as_ref().unwrap();
        assert_eq!(checkpoint.metadata.job_id, "new-job");
        assert_eq!(checkpoint.metadata.total_work_items, 20);
    }

    #[tokio::test]
    async fn test_should_save_checkpoint_function() {
        use super::super::super::pure::triggers::CheckpointTriggerConfig;

        let config = CheckpointTriggerConfig::item_interval(5);
        let now = chrono::Utc::now();

        // Below threshold
        assert!(!should_save_checkpoint(3, now, &config));

        // At threshold
        assert!(should_save_checkpoint(5, now, &config));
    }
}
