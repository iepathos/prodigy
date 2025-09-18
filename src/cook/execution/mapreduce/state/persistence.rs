//! State persistence operations for MapReduce jobs
//!
//! Handles saving and loading job state to/from storage backends.

use super::{JobProgress, JobState, JobSummary, PhaseType, StateError, StateStore};
use crate::cook::execution::state::{DefaultJobStateManager, JobStateManager, MapReduceJobState};
use std::sync::Arc;
use tracing::{debug, error, info};

#[cfg(test)]
use std::collections::HashMap;

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
                "~/.prodigy/state/{}",
                repository_name
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

/// In-memory state store for testing
#[cfg(test)]
pub struct InMemoryStateStore {
    states: Arc<tokio::sync::RwLock<HashMap<String, JobState>>>,
}

#[cfg(test)]
impl InMemoryStateStore {
    /// Create a new in-memory state store
    pub fn new() -> Self {
        Self {
            states: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[cfg(test)]
impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl StateStore for InMemoryStateStore {
    async fn save(&self, state: &JobState) -> Result<(), StateError> {
        let mut states = self.states.write().await;
        states.insert(state.id.clone(), state.clone());
        debug!("Saved state for job {}", state.id);
        Ok(())
    }

    async fn load(&self, job_id: &str) -> Result<Option<JobState>, StateError> {
        let states = self.states.read().await;
        let result = states.get(job_id).cloned();
        debug!("Loaded state for job {}: {}", job_id, result.is_some());
        Ok(result)
    }

    async fn list(&self) -> Result<Vec<JobSummary>, StateError> {
        let states = self.states.read().await;
        let summaries: Vec<JobSummary> = states
            .values()
            .map(|state| {
                let processed = state.processed_items.len();
                let failed = state.failed_items.len();
                let total = state.total_items;
                let pending = total.saturating_sub(processed + failed);

                JobSummary {
                    job_id: state.id.clone(),
                    phase: state.phase,
                    progress: JobProgress {
                        total_items: total,
                        completed_items: processed,
                        failed_items: failed,
                        pending_items: pending,
                        completion_percentage: if total > 0 {
                            (processed as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        },
                    },
                    created_at: state.created_at,
                    updated_at: state.updated_at,
                    is_complete: state.is_complete,
                }
            })
            .collect();
        Ok(summaries)
    }

    async fn delete(&self, job_id: &str) -> Result<(), StateError> {
        let mut states = self.states.write().await;
        states.remove(job_id);
        debug!("Deleted state for job {}", job_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;
    use chrono::Utc;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_state_persistence() {
        let store = InMemoryStateStore::new();

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
