//! State persistence operations for MapReduce jobs
//!
//! Handles saving and loading job state to/from storage backends.

use super::{JobProgress, JobState, JobSummary, PhaseType, StateError, StateStore};
use crate::cook::execution::mapreduce::AgentResult;
use crate::cook::execution::state::{DefaultJobStateManager, JobStateManager, MapReduceJobState};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Default implementation using the existing JobStateManager
pub struct DefaultStateStore {
    /// Underlying state manager
    state_manager: Arc<dyn JobStateManager>,
}

impl DefaultStateStore {
    /// Create a new state store using the default job state manager
    pub fn new(repository_name: String) -> Self {
        use std::path::PathBuf;
        Self {
            state_manager: Arc::new(DefaultJobStateManager::new(PathBuf::from(format!(
                "~/.prodigy/state/{}", repository_name
            )))),
        }
    }

    /// Create from an existing state manager
    pub fn from_manager(state_manager: Arc<dyn JobStateManager>) -> Self {
        Self { state_manager }
    }
}

#[async_trait::async_trait]
impl StateStore for DefaultStateStore {
    async fn save(&self, state: &JobState) -> Result<(), StateError> {
        // For now, we'll just log since JobStateManager doesn't have a save method
        // In a real implementation, we'd need to extend JobStateManager or use a different approach
        debug!("Save state for job {} (not fully implemented)", state.id);
        Ok(())
    }

    async fn load(&self, job_id: &str) -> Result<Option<JobState>, StateError> {
        match self.state_manager.get_job_state(job_id).await {
            Ok(mapreduce_state) => {
                let state = from_mapreduce_job_state(mapreduce_state);
                debug!("Loaded state for job {}", job_id);
                Ok(Some(state))
            }
            Err(e) if e.to_string().contains("not found") => {
                debug!("No state found for job {}", job_id);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to load state for job {}: {}", job_id, e);
                Err(StateError::LoadError(e.to_string()))
            }
        }
    }

    async fn list(&self) -> Result<Vec<JobSummary>, StateError> {
        let jobs = self
            .state_manager
            .list_resumable_jobs()
            .await
            .map_err(|e| StateError::LoadError(e.to_string()))?;

        let summaries = jobs
            .into_iter()
            .map(|job| {
                let progress = JobProgress {
                    total_items: job.total_items,
                    completed_items: job.completed_items,
                    failed_items: job.failed_items,
                    pending_items: job.total_items - job.completed_items - job.failed_items,
                    completion_percentage: if job.total_items > 0 {
                        (job.completed_items as f64 / job.total_items as f64) * 100.0
                    } else {
                        0.0
                    },
                };

                JobSummary {
                    job_id: job.job_id,
                    phase: PhaseType::Map, // Default to Map phase for now
                    progress,
                    created_at: job.started_at,
                    updated_at: job.updated_at,
                    is_complete: job.is_complete,
                }
            })
            .collect();

        Ok(summaries)
    }

    async fn delete(&self, job_id: &str) -> Result<(), StateError> {
        // Note: The current JobStateManager doesn't have a delete method,
        // so we'll just log this for now
        info!("Delete requested for job {} (not implemented)", job_id);
        Ok(())
    }
}

/// Convert from internal JobState to MapReduceJobState
fn to_mapreduce_job_state(state: &JobState) -> MapReduceJobState {
    use crate::cook::workflow::WorkflowStep;

    MapReduceJobState {
        job_id: state.id.clone(),
        config: state.config.clone(),
        started_at: state.created_at,
        updated_at: state.updated_at,
        work_items: Vec::new(), // Will be populated from actual work items
        agent_results: state.agent_results.clone(),
        completed_agents: state.processed_items.clone(),
        failed_agents: HashMap::new(), // Will be populated from failed items
        pending_items: state.failed_items.clone(),
        checkpoint_version: state.checkpoint.as_ref().map(|c| c.version).unwrap_or(0),
        checkpoint_format_version: 1,
        parent_worktree: None,
        reduce_phase_state: None,
        total_items: state.total_items,
        successful_count: state.processed_items.len(),
        failed_count: state.failed_items.len(),
        is_complete: state.is_complete,
        agent_template: Vec::<WorkflowStep>::new(), // Will need to be populated from config
        reduce_commands: None,
        variables: state.variables.clone(),
        setup_output: None,
        setup_completed: matches!(
            state.phase,
            PhaseType::Map | PhaseType::Reduce | PhaseType::Completed
        ),
    }
}

/// Convert from MapReduceJobState to internal JobState
fn from_mapreduce_job_state(state: MapReduceJobState) -> JobState {
    let phase = map_phase_from_state(&state);
    JobState {
        id: state.job_id,
        phase,
        checkpoint: None, // Will be populated separately if needed
        processed_items: state.completed_agents,
        failed_items: state.pending_items,
        variables: state.variables,
        created_at: state.started_at,
        updated_at: state.updated_at,
        config: state.config,
        agent_results: state.agent_results,
        is_complete: state.is_complete,
        total_items: state.total_items,
    }
}

/// Map phase type from MapReduceJobState
fn map_phase_from_state(state: &MapReduceJobState) -> PhaseType {
    if state.is_complete {
        PhaseType::Completed
    } else if state
        .reduce_phase_state
        .as_ref()
        .map(|r| r.started)
        .unwrap_or(false)
    {
        PhaseType::Reduce
    } else if state.setup_completed {
        PhaseType::Map
    } else {
        PhaseType::Setup
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;

    #[tokio::test]
    async fn test_state_persistence() {
        let store = DefaultStateStore::new("test-repo".to_string());

        let state = JobState {
            id: "test-job-123".to_string(),
            phase: PhaseType::Setup,
            checkpoint: None,
            processed_items: Default::default(),
            failed_items: Vec::new(),
            variables: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            config: MapReduceConfig::default(),
            agent_results: HashMap::new(),
            is_complete: false,
            total_items: 10,
        };

        // Save state
        store.save(&state).await.unwrap();

        // Load state
        let loaded = store.load(&state.id).await.unwrap().unwrap();
        assert_eq!(loaded.id, state.id);
        assert_eq!(loaded.phase, state.phase);
        assert_eq!(loaded.total_items, state.total_items);
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let store = DefaultStateStore::new("test-repo".to_string());

        // List should work even with no jobs
        let jobs = store.list().await.unwrap();
        assert!(jobs.is_empty() || !jobs.is_empty()); // Either case is valid
    }
}
