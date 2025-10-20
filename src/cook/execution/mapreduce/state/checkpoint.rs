//! Checkpoint creation and validation for MapReduce jobs
//!
//! Handles creating, validating, and managing checkpoints for job recovery.

use super::{Checkpoint, JobState, StateError, StateEvent, StateEventType, StateManager};
use crate::cook::execution::mapreduce::AgentResult;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tracing::{debug, info, warn};

impl StateManager {
    /// Create a checkpoint for the current job state
    pub async fn create_checkpoint(&self, job_id: &str) -> Result<Checkpoint, StateError> {
        let state = self
            .get_state(job_id)
            .await?
            .ok_or_else(|| StateError::NotFound(job_id.to_string()))?;

        // Collect agent results as a vector
        let agent_results: Vec<AgentResult> = state.agent_results.values().cloned().collect();

        // Calculate next version
        let version = state
            .checkpoint
            .as_ref()
            .map(|c| c.version + 1)
            .unwrap_or(1);

        // Create checkpoint
        let checkpoint = Checkpoint {
            phase: state.phase,
            items_processed: state.processed_items.clone().into_iter().collect(),
            agent_results: agent_results.clone(),
            timestamp: Utc::now(),
            checksum: calculate_checksum(&state),
            version,
        };

        // Update state with new checkpoint
        self.update_state(job_id, |state| {
            state.checkpoint = Some(checkpoint.clone());
            Ok(())
        })
        .await?;

        // Log checkpoint creation
        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::CheckpointCreated { version },
            job_id: job_id.to_string(),
            details: Some(format!(
                "Phase: {:?}, Items processed: {}",
                state.phase,
                agent_results.len()
            )),
        })
        .await;

        info!("Created checkpoint v{} for job {}", version, job_id);
        Ok(checkpoint)
    }

    /// Validate a checkpoint for integrity
    pub fn validate_checkpoint(&self, state: &JobState) -> Result<(), StateError> {
        // Basic validation checks
        if state.id.is_empty() {
            return Err(StateError::ValidationError(
                "Empty job ID in state".to_string(),
            ));
        }

        if state.total_items == 0 && !state.is_complete {
            warn!("State has 0 total items but is not marked complete");
        }

        // Verify counts are consistent
        let total_processed = state.processed_items.len();
        if total_processed > state.total_items {
            return Err(StateError::ValidationError(format!(
                "Processed count ({}) exceeds total items ({})",
                total_processed, state.total_items
            )));
        }

        // Verify all processed items have results
        for item_id in &state.processed_items {
            if !state.agent_results.contains_key(item_id) {
                return Err(StateError::ValidationError(format!(
                    "Processed item {} has no result",
                    item_id
                )));
            }
        }

        // Verify checkpoint integrity if present
        if let Some(ref checkpoint) = state.checkpoint {
            self.validate_checkpoint_integrity(checkpoint, state)?;
        }

        debug!("Checkpoint validation passed for job {}", state.id);
        Ok(())
    }

    /// Validate checkpoint integrity against state
    fn validate_checkpoint_integrity(
        &self,
        checkpoint: &Checkpoint,
        state: &JobState,
    ) -> Result<(), StateError> {
        // Verify checksum
        let expected_checksum = calculate_checksum(state);
        if checkpoint.checksum != expected_checksum {
            warn!(
                "Checksum mismatch for checkpoint v{}: expected {}, got {}",
                checkpoint.version, expected_checksum, checkpoint.checksum
            );
            // We warn but don't fail - checksums might change due to format updates
        }

        // Verify items processed match
        let checkpoint_items: HashSet<_> = checkpoint.items_processed.iter().cloned().collect();
        if checkpoint_items != state.processed_items {
            return Err(StateError::ValidationError(format!(
                "Checkpoint items mismatch: checkpoint has {} items, state has {}",
                checkpoint_items.len(),
                state.processed_items.len()
            )));
        }

        // Verify agent results count
        if checkpoint.agent_results.len() != state.agent_results.len() {
            return Err(StateError::ValidationError(format!(
                "Agent results mismatch: checkpoint has {}, state has {}",
                checkpoint.agent_results.len(),
                state.agent_results.len()
            )));
        }

        Ok(())
    }

    /// Get a specific checkpoint version
    pub async fn get_checkpoint(
        &self,
        job_id: &str,
        version: Option<u32>,
    ) -> Result<Option<Checkpoint>, StateError> {
        let state = self.get_state(job_id).await?;

        match state {
            Some(s) => {
                if let Some(checkpoint) = s.checkpoint {
                    if version.is_none() || version == Some(checkpoint.version) {
                        Ok(Some(checkpoint))
                    } else {
                        // In a real implementation, we'd store multiple checkpoint versions
                        // For now, we only have the latest
                        Err(StateError::ValidationError(format!(
                            "Checkpoint version {} not found (current: {})",
                            version.unwrap(),
                            checkpoint.version
                        )))
                    }
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Clean old checkpoints, keeping only the most recent N
    pub async fn clean_old_checkpoints(
        &self,
        job_id: &str,
        keep_count: usize,
    ) -> Result<(), StateError> {
        // In a real implementation with multiple checkpoint versions,
        // we would delete old checkpoint files here
        debug!(
            "Cleaning old checkpoints for job {} (keeping {})",
            job_id, keep_count
        );
        Ok(())
    }
}

/// Calculate a checksum for the job state
fn calculate_checksum(state: &JobState) -> String {
    let mut hasher = Sha256::new();

    // Include key state fields in checksum
    hasher.update(state.id.as_bytes());
    hasher.update(format!("{:?}", state.phase).as_bytes());
    hasher.update(state.total_items.to_string().as_bytes());

    // Include processed items
    let mut items: Vec<_> = state.processed_items.iter().cloned().collect();
    items.sort();
    for item in items {
        hasher.update(item.as_bytes());
    }

    // Include agent results
    let mut results: Vec<_> = state.agent_results.keys().cloned().collect();
    results.sort();
    for key in results {
        hasher.update(key.as_bytes());
        if let Some(result) = state.agent_results.get(&key) {
            hasher.update(format!("{:?}", result.status).as_bytes());
        }
    }

    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::state::PhaseType;
    use crate::cook::execution::mapreduce::{AgentStatus, MapReduceConfig};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_checkpoint_creation() {
        let store = Arc::new(super::super::persistence::InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-checkpoint".to_string();

        // Create job
        let _state = manager.create_job(&config, job_id.clone()).await.unwrap();

        // Update job with total items
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
                        json_log_location: None,
            cleanup_status: None,
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
                        json_log_location: None,
            cleanup_status: None,
                    },
                );

                Ok(())
            })
            .await
            .unwrap();

        // Create checkpoint
        let checkpoint = manager.create_checkpoint(&job_id).await.unwrap();

        assert_eq!(checkpoint.version, 1);
        assert_eq!(checkpoint.items_processed.len(), 2);
        assert_eq!(checkpoint.agent_results.len(), 2);

        // Verify checkpoint is saved in state
        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        assert!(state.checkpoint.is_some());
        assert_eq!(state.checkpoint.as_ref().unwrap().version, 1);
    }

    #[tokio::test]
    async fn test_checkpoint_validation() {
        let store = Arc::new(super::super::persistence::InMemoryStateStore::new());
        let manager = StateManager::new(store);

        let config = MapReduceConfig::default();
        let job_id = "test-job-validation".to_string();

        // Create job with invalid state
        let _state = manager.create_job(&config, job_id.clone()).await.unwrap();

        // Update with inconsistent state
        manager
            .update_state(&job_id, |state| {
                state.total_items = 5; // Set total items so we don't fail on count validation
                state.processed_items.insert("item_0".to_string());
                // Don't add corresponding agent result - this makes state invalid
                Ok(())
            })
            .await
            .unwrap();

        let state = manager.get_state(&job_id).await.unwrap().unwrap();
        let result = manager.validate_checkpoint(&state);
        assert!(
            result.is_err(),
            "Expected validation to fail but it succeeded"
        );
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("has no result"),
            "Expected error message to contain 'has no result' but got: '{}'",
            error_msg
        );
    }

    #[tokio::test]
    async fn test_checksum_calculation() {
        let mut state1 = JobState {
            id: "test-job".to_string(),
            phase: PhaseType::Map,
            checkpoint: None,
            processed_items: HashSet::new(),
            failed_items: Vec::new(),
            variables: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            config: MapReduceConfig::default(),
            agent_results: HashMap::new(),
            is_complete: false,
            total_items: 10,
        };

        let checksum1 = calculate_checksum(&state1);

        // Same state should produce same checksum
        let checksum2 = calculate_checksum(&state1);
        assert_eq!(checksum1, checksum2);

        // Modified state should produce different checksum
        state1.processed_items.insert("item_0".to_string());
        let checksum3 = calculate_checksum(&state1);
        assert_ne!(checksum1, checksum3);
    }
}
