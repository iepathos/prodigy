//! State machine transitions for MapReduce jobs
//!
//! Manages valid state transitions and enforces state machine invariants.

use super::{JobState, PhaseType, StateError, StateEvent, StateEventType, StateManager};
use chrono::Utc;
use tracing::{debug, error, info};

impl StateManager {
    /// Transition job to a new phase
    pub async fn transition_to_phase(
        &self,
        job_id: &str,
        new_phase: PhaseType,
    ) -> Result<(), StateError> {
        let state = self
            .get_state(job_id)
            .await?
            .ok_or_else(|| StateError::NotFound(job_id.to_string()))?;

        let old_phase = state.phase;

        // Check if transition is valid
        if !self.transitions.is_valid_transition(old_phase, new_phase) {
            return Err(StateError::InvalidTransition {
                from: old_phase,
                to: new_phase,
            });
        }

        // Update state with new phase
        self.update_state(job_id, |state| {
            state.phase = new_phase;

            // Handle phase-specific updates
            match new_phase {
                PhaseType::Completed => {
                    state.is_complete = true;
                    info!("Job {} completed successfully", job_id);
                }
                PhaseType::Failed => {
                    state.is_complete = true;
                    error!("Job {} failed", job_id);
                }
                _ => {}
            }

            Ok(())
        })
        .await?;

        debug!(
            "Job {} transitioned from {:?} to {:?}",
            job_id, old_phase, new_phase
        );

        Ok(())
    }

    /// Mark job as started (transition from Setup to Map)
    pub async fn mark_job_started(&self, job_id: &str) -> Result<(), StateError> {
        self.transition_to_phase(job_id, PhaseType::Map).await
    }

    /// Mark job as entering reduce phase
    pub async fn mark_reduce_started(&self, job_id: &str) -> Result<(), StateError> {
        self.transition_to_phase(job_id, PhaseType::Reduce).await
    }

    /// Mark job as completed
    pub async fn mark_job_completed(&self, job_id: &str) -> Result<(), StateError> {
        self.update_state(job_id, |state| {
            // Determine which phase we can complete from
            let valid_completion = matches!(state.phase, PhaseType::Map | PhaseType::Reduce);

            if !valid_completion {
                return Err(StateError::InvalidTransition {
                    from: state.phase,
                    to: PhaseType::Completed,
                });
            }

            state.phase = PhaseType::Completed;
            state.is_complete = true;
            Ok(())
        })
        .await?;

        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::JobCompleted,
            job_id: job_id.to_string(),
            details: None,
        })
        .await;

        info!("Job {} marked as completed", job_id);
        Ok(())
    }

    /// Mark job as failed
    pub async fn mark_job_failed(&self, job_id: &str, reason: String) -> Result<(), StateError> {
        self.update_state(job_id, |state| {
            // Can fail from any non-terminal state
            if state.phase == PhaseType::Completed || state.phase == PhaseType::Failed {
                return Err(StateError::InvalidTransition {
                    from: state.phase,
                    to: PhaseType::Failed,
                });
            }

            state.phase = PhaseType::Failed;
            state.is_complete = true;
            Ok(())
        })
        .await?;

        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::JobFailed {
                reason: reason.clone(),
            },
            job_id: job_id.to_string(),
            details: Some(reason),
        })
        .await;

        error!("Job {} marked as failed", job_id);
        Ok(())
    }

    /// Get valid next phases from current state
    pub async fn get_valid_transitions(&self, job_id: &str) -> Result<Vec<PhaseType>, StateError> {
        let state = self
            .get_state(job_id)
            .await?
            .ok_or_else(|| StateError::NotFound(job_id.to_string()))?;

        Ok(self.transitions.get_valid_transitions(state.phase))
    }

    /// Check if a specific transition is valid
    pub async fn can_transition(
        &self,
        job_id: &str,
        to_phase: PhaseType,
    ) -> Result<bool, StateError> {
        let state = self
            .get_state(job_id)
            .await?
            .ok_or_else(|| StateError::NotFound(job_id.to_string()))?;

        Ok(self.transitions.is_valid_transition(state.phase, to_phase))
    }
}

/// Extension methods for JobState phase management
impl JobState {
    /// Check if the job is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self.phase, PhaseType::Completed | PhaseType::Failed)
    }

    /// Check if the job can be resumed
    pub fn can_resume(&self) -> bool {
        !self.is_terminal() && self.checkpoint.is_some()
    }

    /// Check if the job is in the setup phase
    pub fn is_setup(&self) -> bool {
        matches!(self.phase, PhaseType::Setup)
    }

    /// Check if the job is in the map phase
    pub fn is_map(&self) -> bool {
        matches!(self.phase, PhaseType::Map)
    }

    /// Check if the job is in the reduce phase
    pub fn is_reduce(&self) -> bool {
        matches!(self.phase, PhaseType::Reduce)
    }

    /// Get a human-readable status string
    pub fn status_string(&self) -> String {
        match self.phase {
            PhaseType::Setup => "Setting up".to_string(),
            PhaseType::Map => format!(
                "Mapping ({}/{} items)",
                self.processed_items.len(),
                self.total_items
            ),
            PhaseType::Reduce => "Reducing results".to_string(),
            PhaseType::Completed => "Completed".to_string(),
            PhaseType::Failed => "Failed".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::state::persistence::InMemoryStateStore;
    use crate::cook::execution::mapreduce::MapReduceConfig;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_valid_transitions() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-transitions".to_string();

        // Create job (starts in Setup phase)
        manager.create_job(&config, job_id.clone()).await.unwrap();

        // Valid transition: Setup -> Map
        manager.mark_job_started(&job_id).await.unwrap();

        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Map);

        // Valid transition: Map -> Reduce
        manager.mark_reduce_started(&job_id).await.unwrap();

        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Reduce);

        // Valid transition: Reduce -> Completed
        manager.mark_job_completed(&job_id).await.unwrap();

        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Completed);
        assert!(state.is_complete);
    }

    #[tokio::test]
    async fn test_invalid_transitions() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-invalid".to_string();

        // Create job (starts in Setup phase)
        manager.create_job(&config, job_id.clone()).await.unwrap();

        // Invalid transition: Setup -> Reduce (must go through Map)
        let result = manager
            .transition_to_phase(&job_id, PhaseType::Reduce)
            .await;
        assert!(result.is_err());

        // Invalid transition: Setup -> Completed
        let result = manager.mark_job_completed(&job_id).await;
        assert!(result.is_err());

        // State should still be Setup
        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Setup);
    }

    #[tokio::test]
    async fn test_terminal_states() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-terminal".to_string();

        // Create job and move to completed
        manager.create_job(&config, job_id.clone()).await.unwrap();
        manager.mark_job_started(&job_id).await.unwrap();
        manager.mark_job_completed(&job_id).await.unwrap();

        // Cannot transition from Completed
        let result = manager.transition_to_phase(&job_id, PhaseType::Map).await;
        assert!(result.is_err());

        // Create another job and mark it as failed
        let job_id2 = "test-job-failed".to_string();
        manager.create_job(&config, job_id2.clone()).await.unwrap();
        manager
            .mark_job_failed(&job_id2, "Test failure".to_string())
            .await
            .unwrap();

        // Cannot transition from Failed
        let result = manager.transition_to_phase(&job_id2, PhaseType::Map).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_valid_transitions() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-valid-trans".to_string();

        // Create job in Setup phase
        manager.create_job(&config, job_id.clone()).await.unwrap();

        // From Setup, can transition to Map or Failed
        let transitions = manager.get_valid_transitions(&job_id).await.unwrap();
        assert!(transitions.contains(&PhaseType::Map));
        assert!(transitions.contains(&PhaseType::Failed));
        assert_eq!(transitions.len(), 2);

        // Move to Map phase
        manager.mark_job_started(&job_id).await.unwrap();

        // From Map, can transition to Reduce, Completed, or Failed
        let transitions = manager.get_valid_transitions(&job_id).await.unwrap();
        assert!(transitions.contains(&PhaseType::Reduce));
        assert!(transitions.contains(&PhaseType::Completed));
        assert!(transitions.contains(&PhaseType::Failed));
    }

    #[tokio::test]
    async fn test_job_state_helpers() {
        let mut state = JobState {
            id: "test".to_string(),
            phase: PhaseType::Setup,
            checkpoint: None,
            processed_items: Default::default(),
            failed_items: Vec::new(),
            variables: Default::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            config: MapReduceConfig::default(),
            agent_results: Default::default(),
            is_complete: false,
            total_items: 10,
        };

        assert!(state.is_setup());
        assert!(!state.is_map());
        assert!(!state.is_terminal());

        state.phase = PhaseType::Map;
        assert!(state.is_map());
        assert!(!state.is_terminal());

        state.phase = PhaseType::Completed;
        assert!(state.is_terminal());
        assert!(!state.can_resume());
    }
}
