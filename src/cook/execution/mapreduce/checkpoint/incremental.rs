//! Incremental checkpointing for MapReduce workflows
//!
//! This module provides incremental checkpointing that integrates with
//! the map phase execution to save checkpoints periodically during processing.
//!
//! ## Key Features
//!
//! - Checkpoints after every N agent completions (configurable)
//! - Checkpoints at time intervals (configurable)
//! - Checkpoints on signal (SIGINT/SIGTERM) for graceful shutdown
//! - Checkpoints at phase transitions
//! - In-progress items reset to pending for safe resume
//!
//! ## Storage Location
//!
//! All checkpoints are stored in `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`

use super::environment::CheckpointEnv;
use super::pure::preparation::{self, create_initial_checkpoint, update_phase};
use super::pure::triggers::{self, CheckpointTriggerConfig};
use super::{
    CheckpointReason, CheckpointStorage, CompletedWorkItem, FailedWorkItem, MapReduceCheckpoint,
    PhaseType, WorkItem, WorkItemProgress,
};
use crate::cook::execution::mapreduce::agent::AgentResult;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Incremental checkpoint controller
///
/// This controller manages incremental checkpointing during map phase execution.
/// It is designed to be called after each agent completes to track progress
/// and save checkpoints when triggers are met.
#[derive(Clone)]
pub struct IncrementalCheckpointController {
    /// Job identifier
    job_id: String,
    /// Storage implementation
    storage: Arc<dyn CheckpointStorage>,
    /// Current checkpoint state
    current_checkpoint: Arc<RwLock<Option<MapReduceCheckpoint>>>,
    /// Trigger configuration
    trigger_config: CheckpointTriggerConfig,
    /// Items processed since last checkpoint
    items_since_checkpoint: Arc<AtomicUsize>,
    /// Time of last checkpoint
    last_checkpoint_time: Arc<RwLock<DateTime<Utc>>>,
    /// Whether checkpointing is enabled
    enabled: Arc<AtomicBool>,
    /// Storage path for checkpoints
    storage_path: PathBuf,
}

impl IncrementalCheckpointController {
    /// Create a new incremental checkpoint controller
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
            trigger_config,
            items_since_checkpoint: Arc::new(AtomicUsize::new(0)),
            last_checkpoint_time: Arc::new(RwLock::new(Utc::now())),
            enabled: Arc::new(AtomicBool::new(true)),
            storage_path,
        }
    }

    /// Create from a CheckpointEnv
    pub fn from_env(env: &CheckpointEnv) -> Self {
        Self {
            job_id: env.job_id.clone(),
            storage: Arc::clone(&env.storage),
            current_checkpoint: Arc::clone(&env.current_checkpoint),
            trigger_config: env.trigger_config.clone(),
            items_since_checkpoint: Arc::clone(&env.items_since_checkpoint),
            last_checkpoint_time: Arc::clone(&env.last_checkpoint_time),
            enabled: Arc::new(AtomicBool::new(env.enabled)),
            storage_path: env.storage_path.clone(),
        }
    }

    /// Disable checkpointing
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }

    /// Enable checkpointing
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }

    /// Check if checkpointing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Initialize checkpoint state for a new job
    pub async fn initialize(&self, total_items: usize) -> anyhow::Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let checkpoint = create_initial_checkpoint(&self.job_id, total_items, PhaseType::Setup);
        *self.current_checkpoint.write().await = Some(checkpoint);
        *self.last_checkpoint_time.write().await = Utc::now();
        self.items_since_checkpoint.store(0, Ordering::Release);

        info!(
            job_id = %self.job_id,
            total_items = total_items,
            "Initialized incremental checkpoint state"
        );

        Ok(())
    }

    /// Transition to map phase
    pub async fn transition_to_map_phase(
        &self,
        work_items: Vec<serde_json::Value>,
    ) -> anyhow::Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let mut guard = self.current_checkpoint.write().await;
        if let Some(ref mut checkpoint) = *guard {
            update_phase(checkpoint, PhaseType::Map);
            checkpoint.metadata.total_work_items = work_items.len();

            // Set pending items
            checkpoint.work_item_state.pending_items = work_items
                .into_iter()
                .enumerate()
                .map(|(idx, data)| WorkItem {
                    id: format!("item_{}", idx),
                    data,
                })
                .collect();
        }
        drop(guard);

        // Save phase transition checkpoint
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        info!(job_id = %self.job_id, "Transitioned to map phase");

        Ok(())
    }

    /// Transition to reduce phase
    pub async fn transition_to_reduce_phase(&self) -> anyhow::Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let mut guard = self.current_checkpoint.write().await;
        if let Some(ref mut checkpoint) = *guard {
            update_phase(checkpoint, PhaseType::Reduce);
        }
        drop(guard);

        // Save phase transition checkpoint
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        info!(job_id = %self.job_id, "Transitioned to reduce phase");

        Ok(())
    }

    /// Mark a work item as in-progress
    pub async fn mark_item_in_progress(&self, item_id: &str, agent_id: &str) {
        if !self.is_enabled() {
            return;
        }

        let mut guard = self.current_checkpoint.write().await;
        if let Some(ref mut checkpoint) = *guard {
            // Find and remove from pending
            if let Some(pos) = checkpoint
                .work_item_state
                .pending_items
                .iter()
                .position(|item| item.id == item_id)
            {
                let work_item = checkpoint.work_item_state.pending_items.remove(pos);

                // Add to in-progress
                checkpoint.work_item_state.in_progress_items.insert(
                    item_id.to_string(),
                    WorkItemProgress {
                        work_item,
                        agent_id: agent_id.to_string(),
                        started_at: Utc::now(),
                        last_update: Utc::now(),
                    },
                );
            }
        }
    }

    /// Record an agent completion and check if checkpoint should be saved
    ///
    /// Returns true if a checkpoint was saved.
    pub async fn record_agent_completion(
        &self,
        result: &AgentResult,
        original_item: &serde_json::Value,
    ) -> anyhow::Result<bool> {
        if !self.is_enabled() {
            return Ok(false);
        }

        // Update checkpoint state with the result
        self.update_checkpoint_with_result(result, original_item)
            .await;

        // Increment items counter
        let items = self.items_since_checkpoint.fetch_add(1, Ordering::SeqCst) + 1;

        // Check if we should checkpoint
        let last_time = *self.last_checkpoint_time.read().await;
        let should_save =
            triggers::should_checkpoint(items, last_time, Utc::now(), &self.trigger_config);

        if should_save {
            debug!(
                job_id = %self.job_id,
                items_since_last = items,
                "Trigger met, saving incremental checkpoint"
            );
            self.save_checkpoint(CheckpointReason::Interval).await?;
            self.items_since_checkpoint.store(0, Ordering::Release);
            *self.last_checkpoint_time.write().await = Utc::now();
            return Ok(true);
        }

        Ok(false)
    }

    /// Update checkpoint state with an agent result
    async fn update_checkpoint_with_result(
        &self,
        result: &AgentResult,
        original_item: &serde_json::Value,
    ) {
        let mut guard = self.current_checkpoint.write().await;
        if let Some(ref mut checkpoint) = *guard {
            // Remove from in-progress
            if let Some(progress) = checkpoint
                .work_item_state
                .in_progress_items
                .remove(&result.item_id)
            {
                // Determine if success or failure
                match &result.status {
                    crate::cook::execution::mapreduce::agent::AgentStatus::Success => {
                        checkpoint
                            .work_item_state
                            .completed_items
                            .push(CompletedWorkItem {
                                work_item: progress.work_item,
                                result: result.clone(),
                                completed_at: Utc::now(),
                            });
                    }
                    crate::cook::execution::mapreduce::agent::AgentStatus::Failed(error) => {
                        checkpoint
                            .work_item_state
                            .failed_items
                            .push(FailedWorkItem {
                                work_item: progress.work_item,
                                error: error.clone(),
                                failed_at: Utc::now(),
                                retry_count: 0,
                            });
                    }
                    _ => {
                        // Put back in pending for other statuses (cancelled, timeout, etc.)
                        checkpoint
                            .work_item_state
                            .pending_items
                            .push(progress.work_item);
                    }
                }
            } else {
                // Item not in in_progress - this can happen if we're retrying
                // Create a new work item from the original data
                let work_item = WorkItem {
                    id: result.item_id.clone(),
                    data: original_item.clone(),
                };

                match &result.status {
                    crate::cook::execution::mapreduce::agent::AgentStatus::Success => {
                        checkpoint
                            .work_item_state
                            .completed_items
                            .push(CompletedWorkItem {
                                work_item,
                                result: result.clone(),
                                completed_at: Utc::now(),
                            });
                    }
                    crate::cook::execution::mapreduce::agent::AgentStatus::Failed(error) => {
                        checkpoint
                            .work_item_state
                            .failed_items
                            .push(FailedWorkItem {
                                work_item,
                                error: error.clone(),
                                failed_at: Utc::now(),
                                retry_count: 0,
                            });
                    }
                    _ => {}
                }
            }

            // Update completed count
            preparation::update_completed_count(checkpoint);

            // Store agent result
            checkpoint
                .agent_state
                .agent_results
                .insert(result.item_id.clone(), result.clone());
        }
    }

    /// Force save a checkpoint immediately
    pub async fn force_checkpoint(&self, reason: CheckpointReason) -> anyhow::Result<String> {
        self.save_checkpoint(reason).await
    }

    /// Save checkpoint on signal (for graceful shutdown)
    pub async fn save_on_signal(&self) -> anyhow::Result<String> {
        info!(job_id = %self.job_id, "Saving checkpoint due to signal");
        self.save_checkpoint(CheckpointReason::BeforeShutdown).await
    }

    /// Save a checkpoint
    async fn save_checkpoint(&self, reason: CheckpointReason) -> anyhow::Result<String> {
        let checkpoint = {
            let guard = self.current_checkpoint.read().await;
            match guard.as_ref() {
                Some(cp) => preparation::prepare_checkpoint(cp, reason),
                None => {
                    return Err(anyhow::anyhow!("No checkpoint to save"));
                }
            }
        };

        let checkpoint_id = checkpoint.metadata.checkpoint_id.clone();
        let checkpoint_reason = checkpoint.metadata.checkpoint_reason.clone();

        self.storage
            .save_checkpoint(&checkpoint)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to save checkpoint: {}", e))?;

        info!(
            job_id = %self.job_id,
            checkpoint_id = %checkpoint_id,
            completed = checkpoint.metadata.completed_items,
            total = checkpoint.metadata.total_work_items,
            reason = ?checkpoint_reason,
            "Saved incremental checkpoint"
        );

        Ok(checkpoint_id)
    }

    /// Get the current checkpoint state (for inspection/testing)
    pub async fn get_current_checkpoint(&self) -> Option<MapReduceCheckpoint> {
        self.current_checkpoint.read().await.clone()
    }

    /// Get checkpoint statistics
    pub async fn get_stats(&self) -> CheckpointStats {
        let checkpoint = self.current_checkpoint.read().await;
        let last_time = *self.last_checkpoint_time.read().await;

        CheckpointStats {
            job_id: self.job_id.clone(),
            enabled: self.is_enabled(),
            items_since_last: self.items_since_checkpoint.load(Ordering::Acquire),
            last_checkpoint_time: last_time,
            completed_items: checkpoint
                .as_ref()
                .map(|cp| cp.metadata.completed_items)
                .unwrap_or(0),
            total_items: checkpoint
                .as_ref()
                .map(|cp| cp.metadata.total_work_items)
                .unwrap_or(0),
            current_phase: checkpoint.as_ref().map(|cp| cp.metadata.phase),
        }
    }
}

/// Statistics about checkpoint state
#[derive(Debug, Clone)]
pub struct CheckpointStats {
    pub job_id: String,
    pub enabled: bool,
    pub items_since_last: usize,
    pub last_checkpoint_time: DateTime<Utc>,
    pub completed_items: usize,
    pub total_items: usize,
    pub current_phase: Option<PhaseType>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::{AgentResult, AgentStatus};
    use crate::cook::execution::mapreduce::checkpoint::FileCheckpointStorage;
    use serde_json::json;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_controller(temp_dir: &TempDir) -> IncrementalCheckpointController {
        let storage_path = temp_dir.path().to_path_buf();
        let storage: Arc<dyn CheckpointStorage> =
            Arc::new(FileCheckpointStorage::new(storage_path.clone(), true));

        let trigger_config = CheckpointTriggerConfig::item_interval(2);

        IncrementalCheckpointController::new(
            "test-job".to_string(),
            storage,
            storage_path,
            trigger_config,
        )
    }

    fn mock_agent_result(item_id: &str, success: bool) -> AgentResult {
        AgentResult {
            item_id: item_id.to_string(),
            status: if success {
                AgentStatus::Success
            } else {
                AgentStatus::Failed("Test failure".to_string())
            },
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: if success {
                None
            } else {
                Some("Test failure".to_string())
            },
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        }
    }

    #[tokio::test]
    async fn test_initialize_checkpoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let controller = create_test_controller(&temp_dir);

        controller.initialize(10).await.unwrap();

        let checkpoint = controller.get_current_checkpoint().await;
        assert!(checkpoint.is_some());

        let checkpoint = checkpoint.unwrap();
        assert_eq!(checkpoint.metadata.job_id, "test-job");
        assert_eq!(checkpoint.metadata.total_work_items, 10);
        assert_eq!(checkpoint.metadata.phase, PhaseType::Setup);
    }

    #[tokio::test]
    async fn test_transition_to_map_phase() {
        let temp_dir = tempfile::tempdir().unwrap();
        let controller = create_test_controller(&temp_dir);

        controller.initialize(3).await.unwrap();

        let items = vec![json!({"id": "a"}), json!({"id": "b"}), json!({"id": "c"})];
        controller.transition_to_map_phase(items).await.unwrap();

        let checkpoint = controller.get_current_checkpoint().await.unwrap();
        assert_eq!(checkpoint.metadata.phase, PhaseType::Map);
        assert_eq!(checkpoint.work_item_state.pending_items.len(), 3);
    }

    #[tokio::test]
    async fn test_record_agent_completion_triggers_checkpoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let controller = create_test_controller(&temp_dir);

        controller.initialize(5).await.unwrap();

        let items = vec![
            json!({"id": "a"}),
            json!({"id": "b"}),
            json!({"id": "c"}),
            json!({"id": "d"}),
            json!({"id": "e"}),
        ];
        controller
            .transition_to_map_phase(items.clone())
            .await
            .unwrap();

        // Mark items as in-progress and complete them
        controller.mark_item_in_progress("item_0", "agent-0").await;
        let saved = controller
            .record_agent_completion(&mock_agent_result("item_0", true), &items[0])
            .await
            .unwrap();
        assert!(!saved); // 1 item, threshold is 2

        controller.mark_item_in_progress("item_1", "agent-1").await;
        let saved = controller
            .record_agent_completion(&mock_agent_result("item_1", true), &items[1])
            .await
            .unwrap();
        assert!(saved); // 2 items, threshold met

        let stats = controller.get_stats().await;
        assert_eq!(stats.completed_items, 2);
        assert_eq!(stats.items_since_last, 0); // Reset after checkpoint
    }

    #[tokio::test]
    async fn test_failed_item_tracked() {
        let temp_dir = tempfile::tempdir().unwrap();
        let controller = create_test_controller(&temp_dir);

        controller.initialize(1).await.unwrap();

        let items = vec![json!({"id": "a"})];
        controller
            .transition_to_map_phase(items.clone())
            .await
            .unwrap();

        controller.mark_item_in_progress("item_0", "agent-0").await;
        controller
            .record_agent_completion(&mock_agent_result("item_0", false), &items[0])
            .await
            .unwrap();

        let checkpoint = controller.get_current_checkpoint().await.unwrap();
        assert_eq!(checkpoint.work_item_state.failed_items.len(), 1);
        assert_eq!(checkpoint.work_item_state.completed_items.len(), 0);
    }

    #[tokio::test]
    async fn test_disable_checkpointing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let controller = create_test_controller(&temp_dir);

        controller.disable();
        controller.initialize(10).await.unwrap();

        // Checkpoint should be None when disabled
        let checkpoint = controller.get_current_checkpoint().await;
        assert!(checkpoint.is_none());
    }

    #[tokio::test]
    async fn test_force_checkpoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let controller = create_test_controller(&temp_dir);

        controller.initialize(10).await.unwrap();

        let checkpoint_id = controller
            .force_checkpoint(CheckpointReason::Manual)
            .await
            .unwrap();

        assert!(checkpoint_id.starts_with("cp-"));

        // Verify checkpoint was saved
        let files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(!files.is_empty());
    }
}
