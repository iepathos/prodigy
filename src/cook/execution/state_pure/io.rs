//! Effect-based I/O operations for state management
//!
//! All I/O operations are wrapped in Effect types, enabling lazy evaluation,
//! composition, and easy testing with mock environments.

use super::pure;
use super::types::MapReduceJobState;
use crate::cook::execution::mapreduce::AgentResult;
use anyhow::{Context as AnyhowContext, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use stillwater::Effect;

/// Storage backend trait for checkpoint persistence
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn write_checkpoint(&self, job_id: &str, data: &str) -> Result<()>;
    async fn read_checkpoint(&self, job_id: &str) -> Result<String>;
}

/// Event log trait for tracking state changes
#[async_trait::async_trait]
pub trait EventLog: Send + Sync {
    async fn log_checkpoint_saved(&self, job_id: &str) -> Result<()>;
    async fn log_phase_transition(&self, job_id: &str, phase: &str) -> Result<()>;
}

/// Environment for state I/O operations
pub struct StateEnv {
    pub storage: Arc<dyn StorageBackend>,
    pub event_log: Arc<dyn EventLog>,
}

/// Type alias for state effects
pub type StateEffect<T> = Effect<T, anyhow::Error, StateEnv>;

/// Save checkpoint (I/O wrapper)
pub fn save_checkpoint(state: MapReduceJobState) -> StateEffect<()> {
    Effect::from_async(|env: &StateEnv| async move {
        let serialized = serde_json::to_string_pretty(&state)
            .with_context(|| "Failed to serialize job state")?;

        env.storage
            .write_checkpoint(&state.job_id, &serialized)
            .await?;

        // Log event
        env.event_log.log_checkpoint_saved(&state.job_id).await?;

        Ok(())
    })
}

/// Load checkpoint (I/O)
pub fn load_checkpoint(job_id: String) -> StateEffect<MapReduceJobState> {
    Effect::from_async(move |env: &StateEnv| async move {
        let data = env.storage.read_checkpoint(&job_id).await?;
        let state: MapReduceJobState =
            serde_json::from_str(&data).with_context(|| "Failed to deserialize job state")?;
        Ok(state)
    })
}

/// Update state and save (composition of pure + I/O)
pub fn update_with_agent_result(
    state: MapReduceJobState,
    result: AgentResult,
) -> StateEffect<MapReduceJobState> {
    // Pure state update
    let new_state = pure::apply_agent_result(state, result);

    // Save updated state
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

/// Complete agent batch (pure + I/O composition)
pub fn complete_batch(
    state: MapReduceJobState,
    results: Vec<AgentResult>,
) -> StateEffect<MapReduceJobState> {
    // Pure: apply all results
    let mut new_state = state;
    for result in results {
        new_state = pure::apply_agent_result(new_state, result);
    }

    // I/O: save checkpoint
    save_checkpoint(new_state.clone()).and_then(move |_| {
        // Pure: check if transition needed
        if pure::should_transition_to_reduce(&new_state) {
            transition_to_reduce(new_state)
        } else {
            Effect::pure(new_state)
        }
    })
}

/// Transition to reduce phase (I/O)
fn transition_to_reduce(state: MapReduceJobState) -> StateEffect<MapReduceJobState> {
    // Pure state update
    let new_state = pure::start_reduce_phase(state);

    let effect = Effect::from_async(move |env: &StateEnv| async move {
        // Log phase transition
        env.event_log
            .log_phase_transition(&new_state.job_id, "reduce")
            .await?;

        Ok(new_state.clone())
    });

    effect.and_then(|s| save_checkpoint(s.clone()).map(move |_| s))
}

/// Start reduce phase with save
pub fn start_reduce_phase_with_save(state: MapReduceJobState) -> StateEffect<MapReduceJobState> {
    let new_state = pure::start_reduce_phase(state);
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

/// Complete reduce phase with save
pub fn complete_reduce_phase_with_save(
    state: MapReduceJobState,
    output: Option<String>,
) -> StateEffect<MapReduceJobState> {
    let new_state = pure::complete_reduce_phase(state, output);
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

/// Mark job complete with save
pub fn mark_complete_with_save(state: MapReduceJobState) -> StateEffect<MapReduceJobState> {
    let new_state = pure::mark_complete(state);
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

/// Mark setup complete with save
pub fn mark_setup_complete_with_save(
    state: MapReduceJobState,
    output: Option<String>,
) -> StateEffect<MapReduceJobState> {
    let new_state = pure::mark_setup_complete(state, output);
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

/// Update variables with save
pub fn update_variables_with_save(
    state: MapReduceJobState,
    variables: HashMap<String, Value>,
) -> StateEffect<MapReduceJobState> {
    let new_state = pure::update_variables(state, variables);
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

/// Set parent worktree with save
pub fn set_parent_worktree_with_save(
    state: MapReduceJobState,
    worktree: Option<String>,
) -> StateEffect<MapReduceJobState> {
    let new_state = pure::set_parent_worktree(state, worktree);
    save_checkpoint(new_state.clone()).map(move |_| new_state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::{AgentStatus, MapReduceConfig};
    use chrono::Utc;
    use std::collections::HashSet;
    use std::sync::Mutex;

    struct MockStorage {
        checkpoints: Arc<Mutex<HashMap<String, String>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                checkpoints: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl StorageBackend for MockStorage {
        async fn write_checkpoint(&self, job_id: &str, data: &str) -> Result<()> {
            self.checkpoints
                .lock()
                .unwrap()
                .insert(job_id.to_string(), data.to_string());
            Ok(())
        }

        async fn read_checkpoint(&self, job_id: &str) -> Result<String> {
            self.checkpoints
                .lock()
                .unwrap()
                .get(job_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Checkpoint not found"))
        }
    }

    struct MockEventLog;

    #[async_trait::async_trait]
    impl EventLog for MockEventLog {
        async fn log_checkpoint_saved(&self, _job_id: &str) -> Result<()> {
            Ok(())
        }

        async fn log_phase_transition(&self, _job_id: &str, _phase: &str) -> Result<()> {
            Ok(())
        }
    }

    fn test_env() -> Arc<StateEnv> {
        Arc::new(StateEnv {
            storage: Arc::new(MockStorage::new()),
            event_log: Arc::new(MockEventLog),
        })
    }

    fn test_state() -> MapReduceJobState {
        MapReduceJobState {
            job_id: "job-123".to_string(),
            config: MapReduceConfig {
                max_parallel: 5,
                ..Default::default()
            },
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: HashSet::new(),
            failed_agents: HashMap::new(),
            pending_items: vec!["item-0".to_string()],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 1,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        }
    }

    fn test_agent_result(item_id: &str, status: AgentStatus) -> AgentResult {
        AgentResult {
            item_id: item_id.to_string(),
            agent_id: format!("agent-{}", item_id),
            status,
            commits: vec![],
            duration: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
        }
    }

    #[tokio::test]
    async fn test_save_checkpoint() {
        let env = test_env();
        let state = test_state();

        let result = save_checkpoint(state.clone()).run(&env).await;

        // Just verify the operation succeeded
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_checkpoint() {
        let env = test_env();
        let state = test_state();

        // Save first
        save_checkpoint(state.clone()).run(&env).await.unwrap();

        // Load
        let loaded_state = load_checkpoint("job-123".to_string())
            .run(&env)
            .await
            .unwrap();

        assert_eq!(loaded_state.job_id, "job-123");
    }

    #[tokio::test]
    async fn test_update_with_agent_result() {
        let env = test_env();
        let state = test_state();
        let result = test_agent_result("item-0", AgentStatus::Success);

        let new_state = update_with_agent_result(state, result)
            .run(&env)
            .await
            .unwrap();

        assert_eq!(new_state.successful_count, 1);
        assert!(new_state.pending_items.is_empty());
    }

    #[tokio::test]
    async fn test_complete_batch() {
        let env = test_env();
        let state = test_state();
        let results = vec![test_agent_result("item-0", AgentStatus::Success)];

        let new_state = complete_batch(state, results).run(&env).await.unwrap();

        assert_eq!(new_state.successful_count, 1);
        assert!(new_state.pending_items.is_empty());
    }

    #[tokio::test]
    async fn test_start_reduce_phase_with_save() {
        let env = test_env();
        let state = test_state();

        let new_state = start_reduce_phase_with_save(state).run(&env).await.unwrap();

        assert!(new_state.reduce_phase_state.is_some());
        assert!(new_state.reduce_phase_state.as_ref().unwrap().started);
    }

    #[tokio::test]
    async fn test_complete_reduce_phase_with_save() {
        let env = test_env();
        let mut state = test_state();
        state.reduce_phase_state = Some(super::super::types::ReducePhaseState {
            started: true,
            completed: false,
            executed_commands: vec![],
            output: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: None,
        });

        let new_state = complete_reduce_phase_with_save(state, Some("output".to_string()))
            .run(&env)
            .await
            .unwrap();

        assert!(new_state.is_complete);
        assert!(new_state.reduce_phase_state.as_ref().unwrap().completed);
    }

    #[tokio::test]
    async fn test_mark_complete_with_save() {
        let env = test_env();
        let state = test_state();

        let new_state = mark_complete_with_save(state).run(&env).await.unwrap();

        assert!(new_state.is_complete);
    }
}
