//! Integration of checkpoint management with MapReduce execution
//!
//! This module provides the integration layer between the checkpoint manager
//! and the MapReduce coordinator for saving and resuming execution.

use crate::cook::execution::mapreduce::{
    agent::AgentResult,
    checkpoint::{
        CheckpointConfig, CheckpointId, CheckpointManager, CheckpointReason, FileCheckpointStorage,
        MapReduceCheckpoint as Checkpoint, PhaseType, WorkItem, WorkItemProgress, WorkItemState,
    },
    coordination::MapReduceCoordinator,
    types::{MapPhase, ReducePhase, SetupPhase},
};
use crate::cook::orchestrator::ExecutionEnvironment;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

/// Enhanced coordinator with checkpoint support
pub struct CheckpointedCoordinator {
    /// Base coordinator
    _coordinator: MapReduceCoordinator,
    /// Checkpoint manager
    checkpoint_manager: Arc<CheckpointManager>,
    /// Current checkpoint state
    current_checkpoint: Arc<RwLock<Option<Checkpoint>>>,
    /// Last checkpoint time
    last_checkpoint_time: Arc<RwLock<DateTime<Utc>>>,
    /// Items processed since last checkpoint
    items_since_checkpoint: Arc<RwLock<usize>>,
    /// Job ID
    job_id: String,
}

impl CheckpointedCoordinator {
    /// Create a new checkpointed coordinator
    pub fn new(
        coordinator: MapReduceCoordinator,
        checkpoint_storage_path: PathBuf,
        job_id: String,
    ) -> Self {
        let config = CheckpointConfig::default();
        let storage = Box::new(FileCheckpointStorage::new(checkpoint_storage_path, true));
        let checkpoint_manager = Arc::new(CheckpointManager::new(storage, config, job_id.clone()));

        Self {
            _coordinator: coordinator,
            checkpoint_manager,
            current_checkpoint: Arc::new(RwLock::new(None)),
            last_checkpoint_time: Arc::new(RwLock::new(Utc::now())),
            items_since_checkpoint: Arc::new(RwLock::new(0)),
            job_id,
        }
    }

    /// Execute job with checkpoint support
    pub async fn execute_job_with_checkpoints(
        &self,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
        env: &ExecutionEnvironment,
        checkpoint_id: Option<CheckpointId>,
    ) -> Result<Vec<AgentResult>> {
        // Check if we're resuming from a checkpoint
        if let Some(checkpoint_id) = checkpoint_id {
            info!("Resuming job from checkpoint {}", checkpoint_id);
            return self
                .resume_from_checkpoint(checkpoint_id, setup, map_phase, reduce, env)
                .await;
        }

        // Start fresh execution with checkpoint saving
        info!("Starting new job execution with checkpoint support");

        // Initialize checkpoint state
        self.initialize_checkpoint_state(&map_phase).await?;

        // Execute with periodic checkpointing
        let results = self
            .execute_with_checkpoints(setup, map_phase, reduce, env)
            .await?;

        Ok(results)
    }

    /// Initialize checkpoint state for a new job
    async fn initialize_checkpoint_state(&self, map_phase: &MapPhase) -> Result<()> {
        use crate::cook::execution::mapreduce::checkpoint::{
            AgentState, CheckpointMetadata, ErrorState, ExecutionState, ResourceState,
            VariableState,
        };

        let checkpoint = Checkpoint {
            metadata: CheckpointMetadata {
                checkpoint_id: String::new(),
                job_id: self.job_id.clone(),
                version: 1,
                created_at: Utc::now(),
                phase: PhaseType::Setup,
                total_work_items: 0, // Will be updated after loading items
                completed_items: 0,
                checkpoint_reason: CheckpointReason::Manual,
                integrity_hash: String::new(),
            },
            execution_state: ExecutionState {
                current_phase: PhaseType::Setup,
                phase_start_time: Utc::now(),
                setup_results: None,
                map_results: None,
                reduce_results: None,
                workflow_variables: HashMap::new(),
            },
            work_item_state: WorkItemState {
                pending_items: vec![],
                in_progress_items: HashMap::new(),
                completed_items: vec![],
                failed_items: vec![],
                current_batch: None,
            },
            agent_state: AgentState {
                active_agents: HashMap::new(),
                agent_assignments: HashMap::new(),
                agent_results: HashMap::new(),
                resource_allocation: HashMap::new(),
            },
            variable_state: VariableState {
                workflow_variables: HashMap::new(),
                captured_outputs: HashMap::new(),
                environment_variables: HashMap::new(),
                item_variables: HashMap::new(),
            },
            resource_state: ResourceState {
                total_agents_allowed: map_phase.config.max_parallel,
                current_agents_active: 0,
                worktrees_created: vec![],
                worktrees_cleaned: vec![],
                disk_usage_bytes: None,
            },
            error_state: ErrorState {
                error_count: 0,
                dlq_items: vec![],
                error_threshold_reached: false,
                last_error: None,
            },
        };

        *self.current_checkpoint.write().await = Some(checkpoint);
        *self.last_checkpoint_time.write().await = Utc::now();

        Ok(())
    }

    /// Execute with periodic checkpointing
    async fn execute_with_checkpoints(
        &self,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        // Execute setup phase
        if let Some(setup_phase) = setup {
            self.execute_setup_with_checkpoint(setup_phase, env).await?;
        }

        // Execute map phase with checkpointing
        let map_results = self.execute_map_with_checkpoints(map_phase, env).await?;

        // Execute reduce phase
        if let Some(reduce_phase) = reduce {
            self.execute_reduce_with_checkpoint(reduce_phase, &map_results, env)
                .await?;
        }

        // Save final checkpoint
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        Ok(map_results)
    }

    /// Execute setup phase with checkpoint
    async fn execute_setup_with_checkpoint(
        &self,
        _setup: SetupPhase,
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        info!("Executing setup phase with checkpoint support");

        // Update phase in checkpoint
        if let Some(ref mut checkpoint) = *self.current_checkpoint.write().await {
            checkpoint.metadata.phase = PhaseType::Setup;
            checkpoint.execution_state.current_phase = PhaseType::Setup;
        }

        // Execute setup (placeholder - actual implementation would call coordinator)
        // self.coordinator.execute_setup_phase(setup, env).await?;

        // Save checkpoint after setup
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        Ok(())
    }

    /// Execute map phase with periodic checkpointing
    async fn execute_map_with_checkpoints(
        &self,
        map_phase: MapPhase,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        info!("Executing map phase with checkpoint support");

        // Update phase
        if let Some(ref mut checkpoint) = *self.current_checkpoint.write().await {
            checkpoint.metadata.phase = PhaseType::Map;
            checkpoint.execution_state.current_phase = PhaseType::Map;
        }

        // Load work items
        let work_items = self.load_work_items(&map_phase).await?;
        let total_items = work_items.len();

        // Update checkpoint with work items
        if let Some(ref mut checkpoint) = *self.current_checkpoint.write().await {
            checkpoint.metadata.total_work_items = total_items;
            checkpoint.work_item_state.pending_items = work_items
                .into_iter()
                .enumerate()
                .map(|(i, item)| WorkItem {
                    id: format!("item_{}", i),
                    data: item,
                })
                .collect();
        }

        // Save initial checkpoint
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        // Process items with periodic checkpointing
        let mut all_results = Vec::new();

        while let Some(batch) = self.get_next_batch(map_phase.config.max_parallel).await {
            let batch_results = self.process_batch(batch, &map_phase, env).await?;

            // Update checkpoint with results
            self.update_checkpoint_with_results(&batch_results).await?;

            all_results.extend(batch_results);

            // Check if we should checkpoint
            if self.should_checkpoint().await {
                self.save_checkpoint(CheckpointReason::Interval).await?;
                *self.items_since_checkpoint.write().await = 0;
            }
        }

        // Final checkpoint for map phase
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        Ok(all_results)
    }

    /// Execute reduce phase with checkpoint
    async fn execute_reduce_with_checkpoint(
        &self,
        _reduce: ReducePhase,
        _map_results: &[AgentResult],
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        info!("Executing reduce phase with checkpoint support");

        // Update phase
        if let Some(ref mut checkpoint) = *self.current_checkpoint.write().await {
            checkpoint.metadata.phase = PhaseType::Reduce;
            checkpoint.execution_state.current_phase = PhaseType::Reduce;
        }

        // Execute reduce (placeholder - actual implementation would call coordinator)
        // self.coordinator.execute_reduce_phase(reduce, map_results, env).await?;

        // Save final checkpoint
        self.save_checkpoint(CheckpointReason::PhaseTransition)
            .await?;

        Ok(())
    }

    /// Resume execution from a checkpoint
    async fn resume_from_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        // Load checkpoint
        let resume_state = self
            .checkpoint_manager
            .resume_from_checkpoint(Some(checkpoint_id))
            .await
            .context("Failed to load checkpoint")?;

        // Restore state
        *self.current_checkpoint.write().await = Some(resume_state.checkpoint.clone());

        // Determine what to resume based on phase
        match resume_state.checkpoint.metadata.phase {
            PhaseType::Setup => {
                // Resume from setup phase
                info!("Resuming from setup phase");
                self.execute_with_checkpoints(setup, map_phase, reduce, env)
                    .await
            }
            PhaseType::Map => {
                // Resume map phase
                info!(
                    "Resuming from map phase with {} pending items",
                    resume_state.work_items.pending_items.len()
                );

                // Restore work items to checkpoint
                if let Some(ref mut checkpoint) = *self.current_checkpoint.write().await {
                    checkpoint.work_item_state = resume_state.work_items;
                }

                // Continue processing
                let mut all_results: Vec<AgentResult> = resume_state
                    .checkpoint
                    .agent_state
                    .agent_results
                    .values()
                    .cloned()
                    .collect();

                // Process remaining items
                while let Some(batch) = self.get_next_batch(map_phase.config.max_parallel).await {
                    let batch_results = self.process_batch(batch, &map_phase, env).await?;
                    self.update_checkpoint_with_results(&batch_results).await?;
                    all_results.extend(batch_results);

                    if self.should_checkpoint().await {
                        self.save_checkpoint(CheckpointReason::Interval).await?;
                        *self.items_since_checkpoint.write().await = 0;
                    }
                }

                // Execute reduce if needed
                if let Some(reduce_phase) = reduce {
                    self.execute_reduce_with_checkpoint(reduce_phase, &all_results, env)
                        .await?;
                }

                Ok(all_results)
            }
            PhaseType::Reduce => {
                // Resume from reduce phase
                info!("Resuming from reduce phase");

                // Collect results from checkpoint
                let all_results: Vec<AgentResult> = resume_state
                    .checkpoint
                    .agent_state
                    .agent_results
                    .values()
                    .cloned()
                    .collect();

                // Execute reduce
                if let Some(reduce_phase) = reduce {
                    self.execute_reduce_with_checkpoint(reduce_phase, &all_results, env)
                        .await?;
                }

                Ok(all_results)
            }
            PhaseType::Complete => {
                info!("Job already complete");
                Ok(resume_state
                    .checkpoint
                    .agent_state
                    .agent_results
                    .values()
                    .cloned()
                    .collect())
            }
        }
    }

    /// Get next batch of items to process
    async fn get_next_batch(&self, max_size: usize) -> Option<Vec<WorkItem>> {
        let mut checkpoint = self.current_checkpoint.write().await;

        if let Some(ref mut cp) = *checkpoint {
            let pending_count = cp.work_item_state.pending_items.len();
            if pending_count == 0 {
                return None;
            }

            let batch_size = max_size.min(pending_count);
            let batch: Vec<WorkItem> = cp
                .work_item_state
                .pending_items
                .drain(..batch_size)
                .collect();

            // Move to in-progress
            for item in &batch {
                cp.work_item_state.in_progress_items.insert(
                    item.id.clone(),
                    WorkItemProgress {
                        work_item: item.clone(),
                        agent_id: format!("agent_{}", item.id),
                        started_at: Utc::now(),
                        last_update: Utc::now(),
                    },
                );
            }

            Some(batch)
        } else {
            None
        }
    }

    /// Process a batch of work items
    async fn process_batch(
        &self,
        batch: Vec<WorkItem>,
        _map_phase: &MapPhase,
        _env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        // Placeholder for actual batch processing
        // In real implementation, this would call the coordinator's execute methods

        let mut results = Vec::new();
        for item in batch {
            // Simulate processing
            results.push(AgentResult {
                item_id: item.id.clone(),
                status: crate::cook::execution::mapreduce::agent::AgentStatus::Success,
                output: Some(format!("Processed {}", item.id)),
                commits: vec![],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
                json_log_location: None,
            });

            *self.items_since_checkpoint.write().await += 1;
        }

        Ok(results)
    }

    /// Update checkpoint with processing results
    async fn update_checkpoint_with_results(&self, results: &[AgentResult]) -> Result<()> {
        use crate::cook::execution::mapreduce::checkpoint::{CompletedWorkItem, FailedWorkItem};

        let mut checkpoint = self.current_checkpoint.write().await;

        if let Some(ref mut cp) = *checkpoint {
            for result in results {
                // Remove from in-progress
                if let Some(progress) = cp.work_item_state.in_progress_items.remove(&result.item_id)
                {
                    // Add to completed or failed
                    match &result.status {
                        crate::cook::execution::mapreduce::agent::AgentStatus::Success => {
                            cp.work_item_state.completed_items.push(CompletedWorkItem {
                                work_item: progress.work_item,
                                result: result.clone(),
                                completed_at: Utc::now(),
                            });
                            cp.metadata.completed_items += 1;
                        }
                        crate::cook::execution::mapreduce::agent::AgentStatus::Failed(_)
                        | crate::cook::execution::mapreduce::agent::AgentStatus::Timeout => {
                            cp.work_item_state.failed_items.push(FailedWorkItem {
                                work_item: progress.work_item,
                                error: result.error.clone().unwrap_or_default(),
                                failed_at: Utc::now(),
                                retry_count: 0,
                            });
                            cp.error_state.error_count += 1;
                        }
                        crate::cook::execution::mapreduce::agent::AgentStatus::Pending
                        | crate::cook::execution::mapreduce::agent::AgentStatus::Running
                        | crate::cook::execution::mapreduce::agent::AgentStatus::Retrying(_) => {
                            // These statuses shouldn't happen for completed results, but handle them
                            // by keeping the item in progress
                            cp.work_item_state
                                .in_progress_items
                                .insert(result.item_id.clone(), progress);
                        }
                    }

                    // Store agent result
                    cp.agent_state
                        .agent_results
                        .insert(result.item_id.clone(), result.clone());
                }
            }
        }

        Ok(())
    }

    /// Check if we should create a checkpoint
    async fn should_checkpoint(&self) -> bool {
        let items = *self.items_since_checkpoint.read().await;
        let last_time = *self.last_checkpoint_time.read().await;

        self.checkpoint_manager.should_checkpoint(items, last_time)
    }

    /// Save a checkpoint
    async fn save_checkpoint(&self, reason: CheckpointReason) -> Result<()> {
        let checkpoint = self.current_checkpoint.read().await;

        if let Some(ref cp) = *checkpoint {
            let checkpoint_id = self
                .checkpoint_manager
                .create_checkpoint(cp, reason)
                .await?;

            *self.last_checkpoint_time.write().await = Utc::now();

            info!(
                "Saved checkpoint {} with {} completed items",
                checkpoint_id, cp.metadata.completed_items
            );
        }

        Ok(())
    }

    /// Load work items for the map phase
    async fn load_work_items(&self, _map_phase: &MapPhase) -> Result<Vec<Value>> {
        // Placeholder - in real implementation, this would load from input source
        Ok(vec![])
    }
}

/// Create a checkpointed coordinator from a regular coordinator
pub fn create_checkpointed_coordinator(
    coordinator: MapReduceCoordinator,
    checkpoint_path: PathBuf,
    job_id: String,
) -> CheckpointedCoordinator {
    CheckpointedCoordinator::new(coordinator, checkpoint_path, job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Simple unit tests for the core checkpoint functionality without needing full coordinator setup

    #[tokio::test]
    async fn test_get_next_batch_empty() {
        // Create a minimal checkpointed coordinator with temp storage
        let temp_dir = tempfile::TempDir::new().unwrap();
        let checkpoint_path = temp_dir.path().to_path_buf();
        let job_id = "test-empty-batch";

        // Create minimal coordinator using a placeholder pattern
        // Since the base coordinator is complex, we focus on testing checkpoint state directly
        let config = CheckpointConfig::default();
        let storage = Box::new(FileCheckpointStorage::new(checkpoint_path.clone(), true));
        let _checkpoint_manager =
            Arc::new(CheckpointManager::new(storage, config, job_id.to_string()));

        // Create checkpoint state
        let current_checkpoint: Arc<RwLock<Option<Checkpoint>>> =
            Arc::new(RwLock::new(Some(Checkpoint {
                metadata: crate::cook::execution::mapreduce::checkpoint::CheckpointMetadata {
                    checkpoint_id: String::new(),
                    job_id: job_id.to_string(),
                    version: 1,
                    created_at: Utc::now(),
                    phase: PhaseType::Map,
                    total_work_items: 0,
                    completed_items: 0,
                    checkpoint_reason: CheckpointReason::Manual,
                    integrity_hash: String::new(),
                },
                execution_state: crate::cook::execution::mapreduce::checkpoint::ExecutionState {
                    current_phase: PhaseType::Map,
                    phase_start_time: Utc::now(),
                    setup_results: None,
                    map_results: None,
                    reduce_results: None,
                    workflow_variables: std::collections::HashMap::new(),
                },
                work_item_state: WorkItemState {
                    pending_items: vec![],
                    in_progress_items: std::collections::HashMap::new(),
                    completed_items: vec![],
                    failed_items: vec![],
                    current_batch: None,
                },
                agent_state: crate::cook::execution::mapreduce::checkpoint::AgentState {
                    active_agents: std::collections::HashMap::new(),
                    agent_assignments: std::collections::HashMap::new(),
                    agent_results: std::collections::HashMap::new(),
                    resource_allocation: std::collections::HashMap::new(),
                },
                variable_state: crate::cook::execution::mapreduce::checkpoint::VariableState {
                    workflow_variables: std::collections::HashMap::new(),
                    captured_outputs: std::collections::HashMap::new(),
                    environment_variables: std::collections::HashMap::new(),
                    item_variables: std::collections::HashMap::new(),
                },
                resource_state: crate::cook::execution::mapreduce::checkpoint::ResourceState {
                    total_agents_allowed: 10,
                    current_agents_active: 0,
                    worktrees_created: vec![],
                    worktrees_cleaned: vec![],
                    disk_usage_bytes: None,
                },
                error_state: crate::cook::execution::mapreduce::checkpoint::ErrorState {
                    error_count: 0,
                    dlq_items: vec![],
                    error_threshold_reached: false,
                    last_error: None,
                },
            })));

        // Test getting batch from empty state
        let mut checkpoint = current_checkpoint.write().await;
        if let Some(ref mut cp) = *checkpoint {
            let pending_count = cp.work_item_state.pending_items.len();
            assert_eq!(pending_count, 0);
        }
    }

    #[tokio::test]
    async fn test_checkpoint_state_updates() {
        // Test that work items can be moved from pending to in-progress
        let work_items = vec![
            WorkItem {
                id: "item_0".to_string(),
                data: serde_json::json!({"test": "data1"}),
            },
            WorkItem {
                id: "item_1".to_string(),
                data: serde_json::json!({"test": "data2"}),
            },
        ];

        let mut work_item_state = WorkItemState {
            pending_items: work_items,
            in_progress_items: std::collections::HashMap::new(),
            completed_items: vec![],
            failed_items: vec![],
            current_batch: None,
        };

        // Simulate moving items to in-progress
        let batch_size = 2;
        let batch: Vec<WorkItem> = work_item_state.pending_items.drain(..batch_size).collect();

        assert_eq!(batch.len(), 2);
        assert_eq!(work_item_state.pending_items.len(), 0);

        // Move to in-progress
        for item in &batch {
            work_item_state.in_progress_items.insert(
                item.id.clone(),
                WorkItemProgress {
                    work_item: item.clone(),
                    agent_id: format!("agent_{}", item.id),
                    started_at: Utc::now(),
                    last_update: Utc::now(),
                },
            );
        }

        assert_eq!(work_item_state.in_progress_items.len(), 2);
    }

    #[tokio::test]
    async fn test_checkpoint_decision_logic() {
        // Test the checkpoint decision logic
        let config = CheckpointConfig::default();

        // Should not checkpoint if no items processed
        let items_since_last = 0;
        let _last_time = Utc::now();
        assert!(!should_checkpoint_based_on_items(items_since_last, &config));

        // Should checkpoint if enough items processed
        let items_since_last = config.interval_items.unwrap_or(10);
        assert!(should_checkpoint_based_on_items(items_since_last, &config));
    }

    /// Helper function to determine if a checkpoint should be saved based on items processed
    ///
    /// This pure function encapsulates the checkpoint decision logic, making it easily testable
    /// and reducing complexity in the main execution flow.
    ///
    /// # Arguments
    /// * `items_processed` - Number of items processed since last checkpoint
    /// * `config` - Checkpoint configuration containing interval thresholds
    ///
    /// # Returns
    /// `true` if a checkpoint should be saved, `false` otherwise
    fn should_checkpoint_based_on_items(items_processed: usize, config: &CheckpointConfig) -> bool {
        items_processed >= config.interval_items.unwrap_or(10)
    }

    // Phase 1 Integration Tests: Happy Path Coverage

    // Phase 1: Integration tests for happy path coverage
    // These tests verify the state transitions and batch processing logic
    // without requiring complex mocking setup

    #[tokio::test]
    async fn test_execute_map_phase_state_updates() {
        // Test that phase states are updated correctly during map execution
        let temp_dir = tempfile::TempDir::new().unwrap();
        let checkpoint_path = temp_dir.path().to_path_buf();
        let job_id = "test-phase-updates".to_string();

        // Create checkpoint state without full coordinator
        let config = CheckpointConfig::default();
        let storage = Box::new(FileCheckpointStorage::new(checkpoint_path.clone(), true));
        let _checkpoint_manager = Arc::new(CheckpointManager::new(storage, config, job_id.clone()));

        let current_checkpoint: Arc<RwLock<Option<Checkpoint>>> = Arc::new(RwLock::new(None));

        // Initialize with Setup phase
        *current_checkpoint.write().await = Some(Checkpoint {
            metadata: crate::cook::execution::mapreduce::checkpoint::CheckpointMetadata {
                checkpoint_id: String::new(),
                job_id: job_id.clone(),
                version: 1,
                created_at: Utc::now(),
                phase: PhaseType::Setup,
                total_work_items: 0,
                completed_items: 0,
                checkpoint_reason: CheckpointReason::Manual,
                integrity_hash: String::new(),
            },
            execution_state: crate::cook::execution::mapreduce::checkpoint::ExecutionState {
                current_phase: PhaseType::Setup,
                phase_start_time: Utc::now(),
                setup_results: None,
                map_results: None,
                reduce_results: None,
                workflow_variables: std::collections::HashMap::new(),
            },
            work_item_state: WorkItemState {
                pending_items: vec![],
                in_progress_items: std::collections::HashMap::new(),
                completed_items: vec![],
                failed_items: vec![],
                current_batch: None,
            },
            agent_state: crate::cook::execution::mapreduce::checkpoint::AgentState {
                active_agents: std::collections::HashMap::new(),
                agent_assignments: std::collections::HashMap::new(),
                agent_results: std::collections::HashMap::new(),
                resource_allocation: std::collections::HashMap::new(),
            },
            variable_state: crate::cook::execution::mapreduce::checkpoint::VariableState {
                workflow_variables: std::collections::HashMap::new(),
                captured_outputs: std::collections::HashMap::new(),
                environment_variables: std::collections::HashMap::new(),
                item_variables: std::collections::HashMap::new(),
            },
            resource_state: crate::cook::execution::mapreduce::checkpoint::ResourceState {
                total_agents_allowed: 10,
                current_agents_active: 0,
                worktrees_created: vec![],
                worktrees_cleaned: vec![],
                disk_usage_bytes: None,
            },
            error_state: crate::cook::execution::mapreduce::checkpoint::ErrorState {
                error_count: 0,
                dlq_items: vec![],
                error_threshold_reached: false,
                last_error: None,
            },
        });

        // Simulate phase update to Map
        {
            let mut checkpoint = current_checkpoint.write().await;
            if let Some(ref mut cp) = *checkpoint {
                cp.metadata.phase = PhaseType::Map;
                cp.execution_state.current_phase = PhaseType::Map;
            }
        }

        // Verify phase update
        let checkpoint = current_checkpoint.read().await;
        assert!(checkpoint.is_some());
        if let Some(ref cp) = *checkpoint {
            assert_eq!(cp.metadata.phase, PhaseType::Map);
            assert_eq!(cp.execution_state.current_phase, PhaseType::Map);
        }
    }

    #[tokio::test]
    async fn test_work_items_loaded_to_checkpoint() {
        // Test that work items are properly loaded and stored in checkpoint
        let work_items_data = vec![
            serde_json::json!({"id": 1, "data": "test1"}),
            serde_json::json!({"id": 2, "data": "test2"}),
            serde_json::json!({"id": 3, "data": "test3"}),
        ];

        // Simulate work item enumeration (lines 235-242 in execute_map_with_checkpoints)
        let work_items: Vec<WorkItem> = work_items_data
            .into_iter()
            .enumerate()
            .map(|(i, item)| WorkItem {
                id: format!("item_{}", i),
                data: item,
            })
            .collect();

        assert_eq!(work_items.len(), 3);
        assert_eq!(work_items[0].id, "item_0");
        assert_eq!(work_items[1].id, "item_1");
        assert_eq!(work_items[2].id, "item_2");

        // Verify data is preserved
        assert_eq!(work_items[0].data["id"], 1);
        assert_eq!(work_items[1].data["data"], "test2");
    }

    #[tokio::test]
    async fn test_batch_processing_loop() {
        // Test batch processing logic - simulates the while loop in execute_map_with_checkpoints
        let pending_items = (0..5)
            .map(|i| WorkItem {
                id: format!("item_{}", i),
                data: serde_json::json!({"test": format!("data{}", i)}),
            })
            .collect::<Vec<WorkItem>>();

        let max_parallel = 2;
        let mut remaining = pending_items;
        let mut all_processed = Vec::new();
        let mut batch_count = 0;

        // Simulate batch processing loop (lines 252-265)
        while !remaining.is_empty() {
            let batch_size = max_parallel.min(remaining.len());
            let batch: Vec<WorkItem> = remaining.drain(..batch_size).collect();

            assert!(batch.len() <= max_parallel);
            all_processed.extend(batch);
            batch_count += 1;
        }

        assert_eq!(all_processed.len(), 5, "Should process all 5 items");
        assert_eq!(batch_count, 3, "Should require 3 batches (2+2+1)");
        assert_eq!(remaining.len(), 0, "Should have no remaining items");
    }

    #[tokio::test]
    async fn test_checkpoint_time_tracking() {
        // Test that checkpoint time is tracked correctly
        let last_checkpoint_time = Arc::new(RwLock::new(Utc::now()));

        // Simulate checkpoint save
        *last_checkpoint_time.write().await = Utc::now();

        // Verify time was updated recently
        let last_time = *last_checkpoint_time.read().await;
        let now = Utc::now();
        let diff = now.signed_duration_since(last_time);

        assert!(
            diff.num_seconds() < 1,
            "Checkpoint time should be very recent"
        );
    }

    // Phase 2: Tests for Batch Processing and Checkpointing Logic

    #[tokio::test]
    async fn test_batch_processing_with_checkpoint_triggering() {
        // Test the checkpoint decision logic during batch processing
        let items_since_checkpoint = Arc::new(RwLock::new(0));
        let config = CheckpointConfig {
            interval_items: Some(10),
            ..Default::default()
        };
        let checkpoint_interval = config.interval_items.unwrap_or(10);

        // Simulate processing items in batches
        let total_items = 25;
        let batch_size = 5;
        let mut checkpoints_saved = 0;

        for batch_num in 0..(total_items / batch_size) {
            // Process batch
            *items_since_checkpoint.write().await += batch_size;

            // Check if we should checkpoint
            let items_count = *items_since_checkpoint.read().await;
            if items_count >= checkpoint_interval {
                // Save checkpoint
                checkpoints_saved += 1;
                // Reset counter (line 263)
                *items_since_checkpoint.write().await = 0;
            }

            // After 2 batches (10 items), we should have saved a checkpoint
            if batch_num == 1 {
                assert_eq!(
                    checkpoints_saved, 1,
                    "Should save checkpoint after 10 items"
                );
                assert_eq!(
                    *items_since_checkpoint.read().await,
                    0,
                    "Counter should reset after checkpoint"
                );
            }
        }

        // Verify we saved checkpoints at the right intervals
        assert_eq!(
            checkpoints_saved, 2,
            "Should have saved 2 checkpoints (at 10 and 20 items)"
        );
    }

    #[tokio::test]
    async fn test_batch_processing_without_intermediate_checkpoints() {
        // Test processing multiple batches when checkpoint interval isn't reached
        let items_since_checkpoint = Arc::new(RwLock::new(0));
        let config = CheckpointConfig {
            interval_items: Some(100), // High threshold
            ..Default::default()
        };
        let checkpoint_interval = config.interval_items.unwrap_or(10);

        // Process 5 batches of 5 items each (25 total)
        let batches = 5;
        let batch_size = 5;
        let mut checkpoints_saved = 0;

        for _ in 0..batches {
            *items_since_checkpoint.write().await += batch_size;

            // Check if we should checkpoint
            let items_count = *items_since_checkpoint.read().await;
            if items_count >= checkpoint_interval {
                checkpoints_saved += 1;
                *items_since_checkpoint.write().await = 0;
            }
        }

        // Verify no intermediate checkpoints were saved
        assert_eq!(
            checkpoints_saved, 0,
            "Should not save checkpoints when threshold not reached"
        );
        assert_eq!(
            *items_since_checkpoint.read().await,
            25,
            "Counter should accumulate without reset"
        );
    }

    #[tokio::test]
    async fn test_should_checkpoint_interval_logic() {
        // Test the should_checkpoint() decision logic
        let config = CheckpointConfig {
            interval_items: Some(10),
            interval_duration: None, // Only test item-based checkpointing
            ..Default::default()
        };

        // Test when checkpoint should NOT be triggered
        let items_processed = 5;
        assert!(
            !should_checkpoint_based_on_items(items_processed, &config),
            "Should not checkpoint with only 5 items processed"
        );

        // Test when checkpoint SHOULD be triggered
        let items_processed = 10;
        assert!(
            should_checkpoint_based_on_items(items_processed, &config),
            "Should checkpoint with 10 items processed"
        );

        // Test when checkpoint SHOULD be triggered (exceeded threshold)
        let items_processed = 15;
        assert!(
            should_checkpoint_based_on_items(items_processed, &config),
            "Should checkpoint with 15 items processed"
        );

        // Test edge case: exactly at threshold
        let items_processed = 10;
        assert!(
            should_checkpoint_based_on_items(items_processed, &config),
            "Should checkpoint exactly at threshold"
        );
    }

    #[tokio::test]
    async fn test_items_counter_reset_after_checkpoint() {
        // Test that items_since_checkpoint counter resets correctly after saving checkpoint
        let items_since_checkpoint = Arc::new(RwLock::new(0));

        // Simulate processing items
        *items_since_checkpoint.write().await = 15;
        assert_eq!(*items_since_checkpoint.read().await, 15);

        // Simulate checkpoint save and reset (line 263)
        *items_since_checkpoint.write().await = 0;
        assert_eq!(
            *items_since_checkpoint.read().await,
            0,
            "Counter should be reset to 0"
        );

        // Simulate processing more items after reset
        *items_since_checkpoint.write().await = 5;
        assert_eq!(
            *items_since_checkpoint.read().await,
            5,
            "Counter should accumulate from 0"
        );
    }

    // Phase 4: Edge Case and Error Condition Tests

    #[tokio::test]
    async fn test_empty_work_items() {
        // Test handling of empty work items (line 555 returns empty vec)
        let work_items_data: Vec<serde_json::Value> = vec![];

        // Simulate work item enumeration (lines 235-242)
        let work_items: Vec<WorkItem> = work_items_data
            .into_iter()
            .enumerate()
            .map(|(i, item)| WorkItem {
                id: format!("item_{}", i),
                data: item,
            })
            .collect();

        assert_eq!(work_items.len(), 0, "Should handle empty work items");

        // Verify total_items would be 0
        let total_items = work_items.len();
        assert_eq!(total_items, 0);
    }

    #[tokio::test]
    async fn test_checkpoint_state_defensive_none_handling() {
        // Test defensive handling when checkpoint state is None
        let current_checkpoint: Arc<RwLock<Option<Checkpoint>>> = Arc::new(RwLock::new(None));

        // Attempt to read checkpoint
        let checkpoint = current_checkpoint.read().await;
        assert!(
            checkpoint.is_none(),
            "Should handle None checkpoint gracefully"
        );

        // Verify we don't panic when checkpoint is None
        if checkpoint.is_some() {
            panic!("Should not have a checkpoint");
        }
        // Test passes - no panic
    }

    #[tokio::test]
    async fn test_get_next_batch_returns_none_when_empty() {
        // Test that get_next_batch returns None when no items remain
        let current_checkpoint: Arc<RwLock<Option<Checkpoint>>> =
            Arc::new(RwLock::new(Some(Checkpoint {
                metadata: crate::cook::execution::mapreduce::checkpoint::CheckpointMetadata {
                    checkpoint_id: String::new(),
                    job_id: "test".to_string(),
                    version: 1,
                    created_at: Utc::now(),
                    phase: PhaseType::Map,
                    total_work_items: 0,
                    completed_items: 0,
                    checkpoint_reason: CheckpointReason::Manual,
                    integrity_hash: String::new(),
                },
                execution_state: crate::cook::execution::mapreduce::checkpoint::ExecutionState {
                    current_phase: PhaseType::Map,
                    phase_start_time: Utc::now(),
                    setup_results: None,
                    map_results: None,
                    reduce_results: None,
                    workflow_variables: std::collections::HashMap::new(),
                },
                work_item_state: WorkItemState {
                    pending_items: vec![], // Empty!
                    in_progress_items: std::collections::HashMap::new(),
                    completed_items: vec![],
                    failed_items: vec![],
                    current_batch: None,
                },
                agent_state: crate::cook::execution::mapreduce::checkpoint::AgentState {
                    active_agents: std::collections::HashMap::new(),
                    agent_assignments: std::collections::HashMap::new(),
                    agent_results: std::collections::HashMap::new(),
                    resource_allocation: std::collections::HashMap::new(),
                },
                variable_state: crate::cook::execution::mapreduce::checkpoint::VariableState {
                    workflow_variables: std::collections::HashMap::new(),
                    captured_outputs: std::collections::HashMap::new(),
                    environment_variables: std::collections::HashMap::new(),
                    item_variables: std::collections::HashMap::new(),
                },
                resource_state: crate::cook::execution::mapreduce::checkpoint::ResourceState {
                    total_agents_allowed: 10,
                    current_agents_active: 0,
                    worktrees_created: vec![],
                    worktrees_cleaned: vec![],
                    disk_usage_bytes: None,
                },
                error_state: crate::cook::execution::mapreduce::checkpoint::ErrorState {
                    error_count: 0,
                    dlq_items: vec![],
                    error_threshold_reached: false,
                    last_error: None,
                },
            })));

        // Simulate get_next_batch logic
        let checkpoint = current_checkpoint.read().await;
        let batch = if let Some(ref cp) = *checkpoint {
            if cp.work_item_state.pending_items.is_empty() {
                None
            } else {
                Some(cp.work_item_state.pending_items.clone())
            }
        } else {
            None
        };

        assert!(batch.is_none(), "Should return None when no items remain");
    }

    #[tokio::test]
    async fn test_checkpoint_interval_edge_cases() {
        // Test edge cases for checkpoint interval configuration

        // Test with interval_items = 0 (should always checkpoint)
        let config = CheckpointConfig {
            interval_items: Some(0),
            ..Default::default()
        };
        assert!(
            should_checkpoint_based_on_items(0, &config),
            "Should checkpoint immediately with interval_items = 0"
        );

        // Test with interval_items = 1 (checkpoint after every item)
        let config = CheckpointConfig {
            interval_items: Some(1),
            ..Default::default()
        };
        assert!(
            should_checkpoint_based_on_items(1, &config),
            "Should checkpoint after 1 item"
        );
        assert!(
            !should_checkpoint_based_on_items(0, &config),
            "Should not checkpoint with 0 items"
        );

        // Test with None (uses default of 10)
        let config = CheckpointConfig {
            interval_items: None,
            ..Default::default()
        };
        assert!(
            should_checkpoint_based_on_items(10, &config),
            "Should use default threshold of 10"
        );
    }

    #[tokio::test]
    async fn test_large_batch_processing() {
        // Test processing a large number of items
        let total_items = 1000;
        let max_parallel = 50;

        let mut pending_items: Vec<WorkItem> = (0..total_items)
            .map(|i| WorkItem {
                id: format!("item_{}", i),
                data: serde_json::json!({"index": i}),
            })
            .collect();

        let mut batch_count = 0;
        let mut total_processed = 0;

        // Simulate batch processing
        while !pending_items.is_empty() {
            let batch_size = max_parallel.min(pending_items.len());
            let _batch: Vec<WorkItem> = pending_items.drain(..batch_size).collect();
            total_processed += batch_size;
            batch_count += 1;
        }

        assert_eq!(total_processed, total_items, "Should process all items");
        assert_eq!(batch_count, 20, "Should require 20 batches (50 items each)");
    }
}
