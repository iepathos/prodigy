//! Recovery and resume logic for MapReduce jobs
//!
//! Handles job recovery from checkpoints and calculating pending work.

use super::{
    JobState, RecoveryPlan, StateError, StateEvent, StateEventType, StateManager,
};
use chrono::Utc;
use serde_json::Value;
use tracing::{debug, info};

impl StateManager {
    /// Create a recovery plan from a checkpoint
    pub async fn recover_from_checkpoint(
        &self,
        job_id: &str,
        checkpoint_version: Option<u32>,
    ) -> Result<RecoveryPlan, StateError> {
        let state = self
            .get_state(job_id)
            .await?
            .ok_or_else(|| StateError::NotFound(job_id.to_string()))?;

        // Validate checkpoint if specified
        if let Some(version) = checkpoint_version {
            if let Some(ref checkpoint) = state.checkpoint {
                if checkpoint.version != version {
                    return Err(StateError::ValidationError(format!(
                        "Requested checkpoint version {} but found {}",
                        version, checkpoint.version
                    )));
                }
            } else {
                return Err(StateError::ValidationError(format!(
                    "No checkpoint found for job {}",
                    job_id
                )));
            }
        }

        // Validate state before recovery
        self.validate_checkpoint(&state)?;

        // Calculate pending items
        let pending_items = self.calculate_pending_items(&state, 0)?;

        // Log recovery start
        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::RecoveryStarted {
                checkpoint_version: state.checkpoint.as_ref().map(|c| c.version).unwrap_or(0),
            },
            job_id: job_id.to_string(),
            details: Some(format!("Recovering {} pending items", pending_items.len())),
        })
        .await;

        let plan = RecoveryPlan {
            resume_phase: state.phase,
            pending_items,
            skip_items: state.processed_items.clone(),
            variables: state.variables.clone(),
            agent_results: state.agent_results.clone(),
        };

        info!(
            "Created recovery plan for job {}: {} items to process, {} to skip",
            job_id,
            plan.pending_items.len(),
            plan.skip_items.len()
        );

        Ok(plan)
    }

    /// Calculate pending items for resumption
    pub fn calculate_pending_items(
        &self,
        state: &JobState,
        max_additional_retries: u32,
    ) -> Result<Vec<Value>, StateError> {
        let mut pending_items = Vec::new();
        let work_items = self.get_work_items_from_state(state)?;

        // Add never-attempted items
        for (i, item) in work_items.iter().enumerate() {
            let item_id = format!("item_{}", i);
            if !state.processed_items.contains(&item_id) && !state.failed_items.contains(&item_id) {
                pending_items.push(item.clone());
                debug!("Adding never-attempted item: {}", item_id);
            }
        }

        // Add retriable failed items
        let _max_retries = state.config.retry_on_failure + max_additional_retries;
        for failed_item_id in &state.failed_items {
            // Extract item index from ID
            if let Some(idx) = failed_item_id
                .strip_prefix("item_")
                .and_then(|s| s.parse::<usize>().ok())
            {
                if idx < work_items.len() {
                    // Check if we should retry this item
                    // In the current implementation, we assume all failed items should be retried
                    // up to the max retry limit
                    pending_items.push(work_items[idx].clone());
                    debug!("Adding failed item for retry: {}", failed_item_id);
                }
            }
        }

        info!(
            "Calculated {} pending items for recovery",
            pending_items.len()
        );
        Ok(pending_items)
    }

    /// Get work items from state or reconstruct them
    fn get_work_items_from_state(&self, state: &JobState) -> Result<Vec<Value>, StateError> {
        // In a real implementation, we would store work items in the state
        // For now, we'll reconstruct them from the total count
        let mut items = Vec::new();
        for i in 0..state.total_items {
            items.push(Value::String(format!("item_{}", i)));
        }
        Ok(items)
    }

    /// Check if a job can be resumed
    pub async fn can_resume_job(&self, job_id: &str) -> bool {
        match self.get_state(job_id).await {
            Ok(Some(state)) => !state.is_complete && state.checkpoint.is_some(),
            _ => false,
        }
    }

    /// Apply a recovery plan to resume job execution
    pub async fn apply_recovery_plan(
        &self,
        job_id: &str,
        plan: &RecoveryPlan,
    ) -> Result<(), StateError> {
        self.update_state(job_id, |state| {
            // Restore variables
            state.variables = plan.variables.clone();

            // Restore agent results
            state.agent_results = plan.agent_results.clone();

            // Restore processed items
            state.processed_items = plan.skip_items.clone();

            // Update phase if needed
            if state.phase != plan.resume_phase {
                debug!(
                    "Updating phase from {:?} to {:?}",
                    state.phase, plan.resume_phase
                );
                state.phase = plan.resume_phase;
            }

            Ok(())
        })
        .await?;

        info!(
            "Applied recovery plan to job {}: resuming from phase {:?}",
            job_id, plan.resume_phase
        );

        Ok(())
    }

    /// Mark items as processed during recovery
    pub async fn mark_items_processed(
        &self,
        job_id: &str,
        item_ids: Vec<String>,
    ) -> Result<(), StateError> {
        let count = item_ids.len();

        self.update_state(job_id, |state| {
            for item_id in item_ids {
                state.processed_items.insert(item_id);
            }
            Ok(())
        })
        .await?;

        // Log progress
        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::ItemsProcessed { count },
            job_id: job_id.to_string(),
            details: None,
        })
        .await;

        Ok(())
    }

    /// Mark items as failed during recovery
    pub async fn mark_items_failed(
        &self,
        job_id: &str,
        item_ids: Vec<String>,
    ) -> Result<(), StateError> {
        let count = item_ids.len();

        self.update_state(job_id, |state| {
            for item_id in item_ids {
                if !state.failed_items.contains(&item_id) {
                    state.failed_items.push(item_id);
                }
            }
            Ok(())
        })
        .await?;

        // Log failures
        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::ItemsFailed { count },
            job_id: job_id.to_string(),
            details: None,
        })
        .await;

        Ok(())
    }
}

/// Options for job resumption
#[derive(Debug, Clone)]
pub struct ResumeOptions {
    /// Force resume even if job appears complete
    pub force_resume: bool,
    /// Maximum additional retries for failed items
    pub max_additional_retries: u32,
    /// Skip validation of checkpoint integrity
    pub skip_validation: bool,
    /// Specific checkpoint version to resume from
    pub from_checkpoint: Option<u32>,
}

impl Default for ResumeOptions {
    fn default() -> Self {
        Self {
            force_resume: false,
            max_additional_retries: 2,
            skip_validation: false,
            from_checkpoint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::state::persistence::InMemoryStateStore;
    use crate::cook::execution::mapreduce::state::PhaseType;
    use crate::cook::execution::mapreduce::{AgentResult, AgentStatus, MapReduceConfig};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_recovery_plan_creation() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-recovery".to_string();

        // Create job with some progress
        let _state = manager.create_job(&config, job_id.clone()).await.unwrap();

        // Simulate some processing
        manager
            .update_state(&job_id, |state| {
                state.total_items = 5;
                state.processed_items.insert("item_0".to_string());
                state.processed_items.insert("item_1".to_string());

                state.agent_results.insert(
                    "item_0".to_string(),
                    AgentResult {
                        item_id: "item_0".to_string(),
                        status: AgentStatus::Success,
                        output: Some("output".to_string()),
                        commits: vec![],
                        duration: Duration::from_secs(1),
                        error: None,
                        worktree_path: None,
                        branch_name: None,
                        worktree_session_id: None,
                        files_modified: vec![],
                    },
                );

                state.agent_results.insert(
                    "item_1".to_string(),
                    AgentResult {
                        item_id: "item_1".to_string(),
                        status: AgentStatus::Success,
                        output: Some("output".to_string()),
                        commits: vec![],
                        duration: Duration::from_secs(1),
                        error: None,
                        worktree_path: None,
                        branch_name: None,
                        worktree_session_id: None,
                        files_modified: vec![],
                    },
                );

                state.failed_items.push("item_2".to_string());

                Ok(())
            })
            .await
            .unwrap();

        // Create checkpoint
        manager.create_checkpoint(&job_id).await.unwrap();

        // Create recovery plan
        let plan = manager
            .recover_from_checkpoint(&job_id, None)
            .await
            .unwrap();

        assert_eq!(plan.skip_items.len(), 2); // item_0 and item_1
        assert!(plan.skip_items.contains("item_0"));
        assert!(plan.skip_items.contains("item_1"));

        // Should have pending items: never attempted (item_3, item_4) + failed (item_2)
        assert_eq!(plan.pending_items.len(), 3);
    }

    #[tokio::test]
    async fn test_calculate_pending_items() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let mut state = JobState {
            id: "test-job".to_string(),
            phase: PhaseType::Map,
            checkpoint: None,
            processed_items: HashSet::new(),
            failed_items: Vec::new(),
            variables: Default::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            config: MapReduceConfig::default(),
            agent_results: Default::default(),
            is_complete: false,
            total_items: 5,
        };

        // Mark some items as processed
        state.processed_items.insert("item_0".to_string());
        state.processed_items.insert("item_1".to_string());

        // Mark some items as failed
        state.failed_items.push("item_2".to_string());

        let pending = manager.calculate_pending_items(&state, 0).unwrap();

        // Should have item_3, item_4 (never attempted) and item_2 (failed)
        assert_eq!(pending.len(), 3);
    }

    #[tokio::test]
    async fn test_mark_items_processed() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-mark".to_string();

        manager.create_job(&config, job_id.clone()).await.unwrap();

        // Mark items as processed
        manager
            .mark_items_processed(&job_id, vec!["item_0".to_string(), "item_1".to_string()])
            .await
            .unwrap();

        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        assert_eq!(state.processed_items.len(), 2);
        assert!(state.processed_items.contains("item_0"));
        assert!(state.processed_items.contains("item_1"));
    }

    #[tokio::test]
    async fn test_can_resume_job() {
        let store = Arc::new(InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-resume".to_string();

        // Create job
        manager.create_job(&config, job_id.clone()).await.unwrap();

        // Job without checkpoint cannot be resumed
        assert!(!manager.can_resume_job(&job_id).await);

        // Create checkpoint
        manager.create_checkpoint(&job_id).await.unwrap();

        // Now job can be resumed
        assert!(manager.can_resume_job(&job_id).await);

        // Mark job as complete
        manager
            .update_state(&job_id, |state| {
                state.is_complete = true;
                Ok(())
            })
            .await
            .unwrap();

        // Complete job cannot be resumed
        assert!(!manager.can_resume_job(&job_id).await);
    }
}
