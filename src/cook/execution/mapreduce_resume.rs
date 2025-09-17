//! Enhanced MapReduce resume functionality
//!
//! Provides robust resume capabilities for MapReduce workflows with improved
//! state restoration, work item management, and cross-worktree synchronization.

use super::dlq::DeadLetterQueue;
use super::errors::MapReduceError;
use super::errors::MapReduceResult as MRResult;
use super::events::{EventLogger, MapReduceEvent};
use super::mapreduce::{AgentResult, MapPhase, MapReduceExecutor, ReducePhase};
use super::state::{JobStateManager, MapReduceJobState};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::worktree::{WorktreePool, WorktreePoolConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Enhanced resume options with additional control parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedResumeOptions {
    /// Force resume even if job appears complete
    pub force: bool,
    /// Maximum additional retries for failed items
    pub max_additional_retries: u32,
    /// Skip validation of checkpoint integrity
    pub skip_validation: bool,
    /// Specific checkpoint version to resume from (None for latest)
    pub from_checkpoint: Option<u32>,
    /// Maximum parallel agents (None uses original config)
    pub max_parallel: Option<usize>,
    /// Force recreation of worktrees
    pub force_recreation: bool,
    /// Include DLQ items in resume
    pub include_dlq_items: bool,
    /// Validate environment consistency
    pub validate_environment: bool,
    /// Reset failed agents for retry
    pub reset_failed_agents: bool,
}

impl Default for EnhancedResumeOptions {
    fn default() -> Self {
        Self {
            force: false,
            max_additional_retries: 2,
            skip_validation: false,
            from_checkpoint: None,
            max_parallel: None,
            force_recreation: false,
            include_dlq_items: true,
            validate_environment: true,
            reset_failed_agents: false,
        }
    }
}

/// Current phase of MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MapReducePhase {
    Setup,
    Map,
    Reduce,
}

/// Result of a phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: MapReducePhase,
    pub completed_at: DateTime<Utc>,
    pub success: bool,
    pub items_processed: usize,
    pub output: Option<Value>,
    pub error: Option<String>,
}

/// Metadata about resume operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeMetadata {
    pub original_start_time: DateTime<Utc>,
    pub last_checkpoint_time: DateTime<Utc>,
    pub resume_attempts: u32,
    pub interruption_reason: Option<String>,
    pub environment_snapshot: EnvironmentSnapshot,
}

/// Environment snapshot for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    pub working_directory: PathBuf,
    pub project_root: PathBuf,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
}

/// Enhanced MapReduce resume state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceResumeState {
    pub job_id: String,
    pub current_phase: MapReducePhase,
    pub completed_items: HashSet<String>,
    pub failed_items: Vec<WorkItem>,
    pub agent_assignments: HashMap<String, String>, // agent_id -> worktree_path
    pub phase_results: HashMap<String, PhaseResult>, // phase name -> result
    pub resume_metadata: ResumeMetadata,
}

/// Work item representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub data: Value,
    pub retry_count: u32,
    pub last_error: Option<String>,
}

/// Result of resume operation
#[derive(Debug, Clone)]
pub enum EnhancedResumeResult {
    MapOnlyCompleted(MapResult),
    FullWorkflowCompleted(FullMapReduceResult),
    PartialResume {
        phase: MapReducePhase,
        progress: f64,
    },
}

/// Map phase result
#[derive(Debug, Clone)]
pub struct MapResult {
    pub successful: usize,
    pub failed: usize,
    pub total: usize,
    pub results: Vec<AgentResult>,
}

/// Full MapReduce workflow result
#[derive(Debug, Clone)]
pub struct FullMapReduceResult {
    pub map_result: MapResult,
    pub reduce_result: Option<Value>,
}

/// Manager for enhanced MapReduce resume functionality
pub struct MapReduceResumeManager {
    state_manager: Arc<dyn JobStateManager>,
    event_logger: Arc<EventLogger>,
    dlq: Arc<DeadLetterQueue>,
    project_root: PathBuf,
}

impl MapReduceResumeManager {
    /// Create a new resume manager
    pub async fn new(
        job_id: String,
        state_manager: Arc<dyn JobStateManager>,
        event_logger: Arc<EventLogger>,
        project_root: PathBuf,
    ) -> anyhow::Result<Self> {
        let dlq = Arc::new(
            DeadLetterQueue::new(
                job_id,
                project_root.clone(),
                1000,                       // max_items
                30,                         // retention_days
                Some(event_logger.clone()), // event_logger
            )
            .await?,
        );

        Ok(Self {
            state_manager,
            event_logger,
            dlq,
            project_root,
        })
    }

    /// Resume a MapReduce job with enhanced options
    pub async fn resume_job(
        &self,
        job_id: &str,
        options: EnhancedResumeOptions,
        env: &ExecutionEnvironment,
    ) -> MRResult<EnhancedResumeResult> {
        info!("Starting enhanced resume for job {}", job_id);

        // Load job state from checkpoint
        let mut job_state = self.load_and_validate_state(job_id, &options).await?;

        // Validate resume feasibility
        self.validate_resume_conditions(&job_state, &options, env)
            .await?;

        // Calculate remaining work items
        let remaining_items = self
            .calculate_remaining_items(&mut job_state, &options)
            .await?;

        // Determine current phase based on job state
        let current_phase = self.determine_current_phase(&job_state);

        info!(
            "Resume state: phase={:?}, completed={}, remaining={}",
            current_phase,
            job_state.completed_agents.len(),
            remaining_items.len()
        );

        // Log resume event
        self.event_logger
            .log(MapReduceEvent::JobResumed {
                job_id: job_id.to_string(),
                checkpoint_version: job_state.checkpoint_version,
                pending_items: remaining_items.len(),
            })
            .await
            .unwrap_or_else(|e| warn!("Failed to log resume event: {}", e));

        // Resume based on current phase
        match current_phase {
            MapReducePhase::Setup => {
                self.resume_from_setup(&mut job_state, remaining_items, env, &options)
                    .await
            }
            MapReducePhase::Map => {
                self.resume_from_map(&mut job_state, remaining_items, env, &options)
                    .await
            }
            MapReducePhase::Reduce => self.resume_from_reduce(&mut job_state, env, &options).await,
        }
    }

    /// Load and validate job state from checkpoint
    async fn load_and_validate_state(
        &self,
        job_id: &str,
        options: &EnhancedResumeOptions,
    ) -> MRResult<MapReduceJobState> {
        // Load from specific version or latest
        let state = if let Some(version) = options.from_checkpoint {
            self.state_manager
                .get_job_state_from_checkpoint(job_id, Some(version))
                .await
                .map_err(|e| MapReduceError::CheckpointLoadFailed {
                    job_id: job_id.to_string(),
                    details: e.to_string(),
                })?
        } else {
            self.state_manager
                .get_job_state(job_id)
                .await
                .map_err(|e| MapReduceError::CheckpointLoadFailed {
                    job_id: job_id.to_string(),
                    details: e.to_string(),
                })?
        };

        // Validate unless skipped
        if !options.skip_validation {
            self.validate_checkpoint_integrity(&state)?;
        }

        Ok(state)
    }

    /// Validate checkpoint integrity
    fn validate_checkpoint_integrity(&self, state: &MapReduceJobState) -> MRResult<()> {
        // Verify job ID is present
        if state.job_id.is_empty() {
            return Err(MapReduceError::CheckpointCorrupted {
                job_id: "<empty>".to_string(),
                version: state.checkpoint_version,
                details: "Empty job ID".to_string(),
            });
        }

        // Verify work items exist
        if state.work_items.is_empty() {
            return Err(MapReduceError::CheckpointCorrupted {
                job_id: state.job_id.clone(),
                version: state.checkpoint_version,
                details: "No work items found".to_string(),
            });
        }

        // Verify counts are consistent
        let total_processed = state.completed_agents.len() + state.failed_agents.len();
        if total_processed > state.total_items {
            return Err(MapReduceError::CheckpointCorrupted {
                job_id: state.job_id.clone(),
                version: state.checkpoint_version,
                details: format!(
                    "Processed count {} exceeds total items {}",
                    total_processed, state.total_items
                ),
            });
        }

        Ok(())
    }

    /// Validate resume conditions
    async fn validate_resume_conditions(
        &self,
        state: &MapReduceJobState,
        options: &EnhancedResumeOptions,
        env: &ExecutionEnvironment,
    ) -> MRResult<()> {
        // Check if job is already complete
        if state.is_complete && !options.force {
            info!("Job {} is already complete, skipping resume", state.job_id);
            return Ok(());
        }

        // Validate environment if requested
        if options.validate_environment {
            self.validate_environment_consistency(state, env)?;
        }

        Ok(())
    }

    /// Validate environment consistency
    fn validate_environment_consistency(
        &self,
        _state: &MapReduceJobState,
        env: &ExecutionEnvironment,
    ) -> MRResult<()> {
        // Check working directory exists
        if !env.working_dir.exists() {
            return Err(MapReduceError::EnvironmentError {
                details: format!("Working directory {:?} does not exist", env.working_dir),
            });
        }

        // Additional environment checks can be added here
        Ok(())
    }

    /// Calculate remaining work items
    async fn calculate_remaining_items(
        &self,
        state: &mut MapReduceJobState,
        options: &EnhancedResumeOptions,
    ) -> MRResult<Vec<Value>> {
        let mut remaining = Vec::new();

        // Add pending items from state
        for item_id in &state.pending_items {
            if let Some(item) = state.find_work_item(item_id) {
                remaining.push(item);
            }
        }

        // Add failed items if retry is enabled
        if options.reset_failed_agents {
            for (item_id, failure) in &state.failed_agents {
                if failure.attempts < state.config.retry_on_failure + options.max_additional_retries
                {
                    if let Some(item) = state.find_work_item(item_id) {
                        remaining.push(item);
                    }
                }
            }
        }

        // Add DLQ items if requested
        if options.include_dlq_items {
            let dlq_items = self.load_dlq_items(&state.job_id).await?;
            remaining.extend(dlq_items);
        }

        Ok(remaining)
    }

    /// Load items from Dead Letter Queue
    async fn load_dlq_items(&self, _job_id: &str) -> MRResult<Vec<Value>> {
        use super::dlq::DLQFilter;

        let filter = DLQFilter {
            reprocess_eligible: Some(true),
            error_type: None,
            after: None,
            before: None,
            error_signature: None,
        };

        match self.dlq.list_items(filter).await {
            Ok(items) => {
                let values: Vec<Value> = items.into_iter().map(|item| item.item_data).collect();
                info!("Loaded {} items from DLQ for job {}", values.len(), _job_id);
                Ok(values)
            }
            Err(e) => {
                warn!("Failed to load DLQ items for job {}: {}", _job_id, e);
                Ok(Vec::new())
            }
        }
    }

    /// Determine current phase from job state
    fn determine_current_phase(&self, state: &MapReduceJobState) -> MapReducePhase {
        // If reduce phase has state and is started but not completed
        if let Some(ref reduce_state) = state.reduce_phase_state {
            if reduce_state.started && !reduce_state.completed {
                return MapReducePhase::Reduce;
            }
        }

        // If not all items are complete, we're still in map phase
        if state.completed_agents.len() < state.total_items {
            return MapReducePhase::Map;
        }

        // If all map items are done and reduce hasn't started
        if state.reduce_commands.is_some() {
            if state.reduce_phase_state.is_none()
                || !state.reduce_phase_state.as_ref().unwrap().started
            {
                return MapReducePhase::Reduce;
            }
        }

        // Default to Map phase
        MapReducePhase::Map
    }

    /// Resume from setup phase
    async fn resume_from_setup(
        &self,
        state: &mut MapReduceJobState,
        remaining_items: Vec<Value>,
        env: &ExecutionEnvironment,
        options: &EnhancedResumeOptions,
    ) -> MRResult<EnhancedResumeResult> {
        info!(
            "Resuming from setup phase with {} items",
            remaining_items.len()
        );

        // Continue to map phase directly since setup is a one-time phase
        self.resume_from_map(state, remaining_items, env, options)
            .await
    }

    /// Resume from map phase
    async fn resume_from_map(
        &self,
        state: &mut MapReduceJobState,
        remaining_items: Vec<Value>,
        env: &ExecutionEnvironment,
        options: &EnhancedResumeOptions,
    ) -> MRResult<EnhancedResumeResult> {
        info!(
            "Resuming map phase with {} remaining items",
            remaining_items.len()
        );

        if remaining_items.is_empty() {
            // Map phase is complete, check for reduce
            if state.reduce_commands.is_some() {
                return self.resume_from_reduce(state, env, options).await;
            } else {
                // No reduce phase, return completed results
                let results: Vec<AgentResult> = state.agent_results.values().cloned().collect();
                return Ok(EnhancedResumeResult::MapOnlyCompleted(MapResult {
                    successful: state.successful_count,
                    failed: state.failed_count,
                    total: state.total_items,
                    results,
                }));
            }
        }

        // Execute remaining map items
        // Note: This would integrate with the existing MapReduceExecutor
        // For now, return partial progress
        let progress = state.completed_agents.len() as f64 / state.total_items as f64;
        Ok(EnhancedResumeResult::PartialResume {
            phase: MapReducePhase::Map,
            progress: progress * 100.0,
        })
    }

    /// Resume from reduce phase
    async fn resume_from_reduce(
        &self,
        state: &mut MapReduceJobState,
        _env: &ExecutionEnvironment,
        _options: &EnhancedResumeOptions,
    ) -> MRResult<EnhancedResumeResult> {
        info!("Resuming from reduce phase");

        // Check if reduce is already complete
        if let Some(reduce_state) = &state.reduce_phase_state {
            if reduce_state.completed {
                let results: Vec<AgentResult> = state.agent_results.values().cloned().collect();
                return Ok(EnhancedResumeResult::FullWorkflowCompleted(
                    FullMapReduceResult {
                        map_result: MapResult {
                            successful: state.successful_count,
                            failed: state.failed_count,
                            total: state.total_items,
                            results,
                        },
                        reduce_result: reduce_state
                            .output
                            .as_ref()
                            .and_then(|s| serde_json::from_str(s).ok()),
                    },
                ));
            }
        }

        // Execute reduce phase (would integrate with existing executor)
        Ok(EnhancedResumeResult::PartialResume {
            phase: MapReducePhase::Reduce,
            progress: 50.0, // Placeholder
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_resume_options_default() {
        let options = EnhancedResumeOptions::default();
        assert!(!options.force);
        assert_eq!(options.max_additional_retries, 2);
        assert!(!options.skip_validation);
        assert!(options.from_checkpoint.is_none());
        assert!(options.include_dlq_items);
        assert!(options.validate_environment);
    }

    #[test]
    fn test_work_item_serialization() {
        let item = WorkItem {
            id: "test-item".to_string(),
            data: serde_json::json!({"value": 42}),
            retry_count: 1,
            last_error: Some("Test error".to_string()),
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: WorkItem = serde_json::from_str(&json).unwrap();

        assert_eq!(item.id, deserialized.id);
        assert_eq!(item.retry_count, deserialized.retry_count);
        assert_eq!(item.last_error, deserialized.last_error);
    }
}
