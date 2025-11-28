//! Checkpoint environment for Reader pattern effects
//!
//! This module provides the CheckpointEnv type and Reader pattern helpers
//! for accessing checkpoint-related components in Effect-based code.

use super::effects::storage::CheckpointStorageEnv;
use super::pure::triggers::CheckpointTriggerConfig;
use super::{CheckpointStorage, MapReduceCheckpoint};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use stillwater::{asks, local, Effect};
use tokio::sync::RwLock;

/// Environment for checkpoint operations
///
/// This environment provides all dependencies needed for checkpoint operations
/// via the Reader pattern. Effects can access these through helper functions.
#[derive(Clone)]
pub struct CheckpointEnv {
    /// Job identifier
    pub job_id: String,
    /// Checkpoint storage implementation
    pub storage: Arc<dyn CheckpointStorage>,
    /// Current checkpoint state
    pub current_checkpoint: Arc<RwLock<Option<MapReduceCheckpoint>>>,
    /// Storage path for checkpoints
    pub storage_path: PathBuf,
    /// Checkpoint trigger configuration
    pub trigger_config: CheckpointTriggerConfig,
    /// Items processed since last checkpoint
    pub items_since_checkpoint: Arc<AtomicUsize>,
    /// Time of last checkpoint
    pub last_checkpoint_time: Arc<RwLock<DateTime<Utc>>>,
    /// Whether checkpointing is enabled
    pub enabled: bool,
}

impl CheckpointEnv {
    /// Create a new checkpoint environment
    pub fn new(
        job_id: String,
        storage: Arc<dyn CheckpointStorage>,
        storage_path: PathBuf,
        trigger_config: CheckpointTriggerConfig,
    ) -> Self {
        Self {
            job_id,
            storage,
            current_checkpoint: Arc::new(RwLock::new(None)),
            storage_path,
            trigger_config,
            items_since_checkpoint: Arc::new(AtomicUsize::new(0)),
            last_checkpoint_time: Arc::new(RwLock::new(Utc::now())),
            enabled: true,
        }
    }

    /// Create a disabled checkpoint environment (for testing)
    pub fn disabled() -> Self {
        use super::FileCheckpointStorage;

        let temp_path = std::env::temp_dir().join("prodigy_disabled_checkpoints");
        let storage: Arc<dyn CheckpointStorage> =
            Arc::new(FileCheckpointStorage::new(temp_path.clone(), false));

        Self {
            job_id: "disabled".to_string(),
            storage,
            current_checkpoint: Arc::new(RwLock::new(None)),
            storage_path: temp_path,
            trigger_config: CheckpointTriggerConfig::none(),
            items_since_checkpoint: Arc::new(AtomicUsize::new(0)),
            last_checkpoint_time: Arc::new(RwLock::new(Utc::now())),
            enabled: false,
        }
    }

    /// Increment items processed count
    pub fn increment_items(&self, count: usize) {
        self.items_since_checkpoint
            .fetch_add(count, Ordering::SeqCst);
    }

    /// Reset items processed count
    pub fn reset_items(&self) {
        self.items_since_checkpoint.store(0, Ordering::SeqCst);
    }

    /// Get current items processed count
    pub fn get_items(&self) -> usize {
        self.items_since_checkpoint.load(Ordering::Acquire)
    }
}

// Implement CheckpointStorageEnv for CheckpointEnv
impl CheckpointStorageEnv for CheckpointEnv {
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

// =============================================================================
// Reader Pattern Helpers
// =============================================================================

/// Error type for checkpoint operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum CheckpointError {
    #[error("Checkpointing is disabled")]
    Disabled,

    #[error("No checkpoint available")]
    NoCheckpoint,

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

/// Get the job ID from the checkpoint environment.
pub fn get_checkpoint_job_id(
) -> impl Effect<Output = String, Error = CheckpointError, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.job_id.clone())
}

/// Get the checkpoint trigger configuration.
pub fn get_trigger_config(
) -> impl Effect<Output = CheckpointTriggerConfig, Error = CheckpointError, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.trigger_config.clone())
}

/// Get the checkpoint storage.
pub fn get_checkpoint_storage(
) -> impl Effect<Output = Arc<dyn CheckpointStorage>, Error = CheckpointError, Env = CheckpointEnv>
{
    asks(|env: &CheckpointEnv| env.storage.clone())
}

/// Get items processed since last checkpoint.
pub fn get_items_since_checkpoint(
) -> impl Effect<Output = usize, Error = CheckpointError, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.items_since_checkpoint.load(Ordering::Acquire))
}

/// Check if checkpointing is enabled.
pub fn is_checkpointing_enabled(
) -> impl Effect<Output = bool, Error = CheckpointError, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.enabled)
}

/// Get the storage path.
pub fn get_checkpoint_storage_path(
) -> impl Effect<Output = PathBuf, Error = CheckpointError, Env = CheckpointEnv> {
    asks(|env: &CheckpointEnv| env.storage_path.clone())
}

// =============================================================================
// Local Override Utilities
// =============================================================================

/// Run an effect with checkpointing disabled.
pub fn with_checkpointing_disabled<E>(
    effect: E,
) -> impl Effect<Output = E::Output, Error = CheckpointError, Env = CheckpointEnv>
where
    E: Effect<Error = CheckpointError, Env = CheckpointEnv>,
{
    local(
        |env: &CheckpointEnv| CheckpointEnv {
            enabled: false,
            ..env.clone()
        },
        effect,
    )
}

/// Run an effect with a custom trigger configuration.
pub fn with_trigger_config<E>(
    config: CheckpointTriggerConfig,
    effect: E,
) -> impl Effect<Output = E::Output, Error = CheckpointError, Env = CheckpointEnv>
where
    E: Effect<Error = CheckpointError, Env = CheckpointEnv>,
{
    local(
        move |env: &CheckpointEnv| CheckpointEnv {
            trigger_config: config.clone(),
            ..env.clone()
        },
        effect,
    )
}

// =============================================================================
// Mock Environment Builder
// =============================================================================

/// Builder for creating mock CheckpointEnv instances for testing.
#[derive(Clone)]
pub struct MockCheckpointEnvBuilder {
    job_id: String,
    trigger_config: CheckpointTriggerConfig,
    enabled: bool,
    initial_checkpoint: Option<MapReduceCheckpoint>,
}

impl Default for MockCheckpointEnvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCheckpointEnvBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            job_id: "mock-job-123".to_string(),
            trigger_config: CheckpointTriggerConfig::default(),
            enabled: true,
            initial_checkpoint: None,
        }
    }

    /// Set the job ID.
    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = job_id.into();
        self
    }

    /// Set the trigger configuration.
    pub fn with_trigger_config(mut self, config: CheckpointTriggerConfig) -> Self {
        self.trigger_config = config;
        self
    }

    /// Disable checkpointing.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set initial checkpoint.
    pub fn with_checkpoint(mut self, checkpoint: MapReduceCheckpoint) -> Self {
        self.initial_checkpoint = Some(checkpoint);
        self
    }

    /// Build the mock environment.
    pub fn build(self) -> CheckpointEnv {
        use super::FileCheckpointStorage;

        let temp_dir = std::env::temp_dir().join(format!("prodigy_mock_{}", self.job_id));
        let _ = std::fs::create_dir_all(&temp_dir);
        let storage: Arc<dyn CheckpointStorage> =
            Arc::new(FileCheckpointStorage::new(temp_dir.clone(), true));

        let mut env = CheckpointEnv::new(self.job_id, storage, temp_dir, self.trigger_config);
        env.enabled = self.enabled;

        if let Some(checkpoint) = self.initial_checkpoint {
            let current_checkpoint = Arc::clone(&env.current_checkpoint);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    *current_checkpoint.write().await = Some(checkpoint);
                })
            });
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_checkpoint_job_id() {
        let env = MockCheckpointEnvBuilder::new()
            .with_job_id("my-test-job")
            .build();

        let effect = get_checkpoint_job_id();
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-test-job");
    }

    #[tokio::test]
    async fn test_get_trigger_config() {
        let config = CheckpointTriggerConfig::item_interval(10);
        let env = MockCheckpointEnvBuilder::new()
            .with_trigger_config(config)
            .build();

        let effect = get_trigger_config();
        let result = effect.run(&env).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().agent_completion_interval, Some(10));
    }

    #[tokio::test]
    async fn test_is_checkpointing_enabled() {
        let enabled_env = MockCheckpointEnvBuilder::new().build();
        let disabled_env = MockCheckpointEnvBuilder::new().disabled().build();

        // Test enabled environment
        let effect = is_checkpointing_enabled();
        assert!(effect.run(&enabled_env).await.unwrap());

        // Test disabled environment separately
        let effect = is_checkpointing_enabled();
        assert!(!effect.run(&disabled_env).await.unwrap());
    }

    #[tokio::test]
    async fn test_with_checkpointing_disabled() {
        let env = MockCheckpointEnvBuilder::new().build();

        // Normally enabled
        let effect = is_checkpointing_enabled();
        assert!(effect.run(&env).await.unwrap());

        // Disabled via local override
        let effect = with_checkpointing_disabled(is_checkpointing_enabled());
        assert!(!effect.run(&env).await.unwrap());

        // Original unchanged
        let effect = is_checkpointing_enabled();
        assert!(effect.run(&env).await.unwrap());
    }

    #[tokio::test]
    async fn test_with_trigger_config_override() {
        let env = MockCheckpointEnvBuilder::new()
            .with_trigger_config(CheckpointTriggerConfig::item_interval(5))
            .build();

        // Without override
        let effect = get_trigger_config();
        assert_eq!(
            effect.run(&env).await.unwrap().agent_completion_interval,
            Some(5)
        );

        // With override
        let new_config = CheckpointTriggerConfig::item_interval(100);
        let effect = with_trigger_config(new_config, get_trigger_config());
        assert_eq!(
            effect.run(&env).await.unwrap().agent_completion_interval,
            Some(100)
        );

        // Original unchanged
        let effect = get_trigger_config();
        assert_eq!(
            effect.run(&env).await.unwrap().agent_completion_interval,
            Some(5)
        );
    }

    #[test]
    fn test_checkpoint_env_increment_items() {
        let env = MockCheckpointEnvBuilder::new().build();

        assert_eq!(env.get_items(), 0);
        env.increment_items(5);
        assert_eq!(env.get_items(), 5);
        env.increment_items(3);
        assert_eq!(env.get_items(), 8);
        env.reset_items();
        assert_eq!(env.get_items(), 0);
    }
}
