//! Pure state transition functions (no I/O, no side effects)
//!
//! All functions in this module are pure - they take state as input and return new state,
//! with no mutations or side effects. This makes them easy to test and reason about.

use super::types::{FailureRecord, MapReduceJobState, ReducePhaseState, WorktreeInfo};
use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;

/// Apply agent completion result to job state (pure function)
///
/// This is a pure function - it has no side effects and always produces
/// the same output for the same inputs. This makes it easy to test and
/// reason about.
///
/// # Examples
///
/// See the unit tests in this module for usage examples.
pub fn apply_agent_result(mut state: MapReduceJobState, result: AgentResult) -> MapReduceJobState {
    let item_id = result.item_id.clone();

    // Update counts based on status
    match &result.status {
        AgentStatus::Success => {
            state.successful_count += 1;
            state.failed_agents.remove(&item_id);
        }
        AgentStatus::Failed(_) | AgentStatus::Timeout => {
            // Get or create failure record
            let failure = state
                .failed_agents
                .entry(item_id.clone())
                .or_insert_with(|| create_initial_failure_record(&item_id));

            // Update failure record
            failure.attempts += 1;
            failure.last_attempt = Utc::now();
            failure.last_error = extract_error_message(&result.status);

            // Update worktree info if available
            if let Some(worktree_info) = extract_worktree_info(&result) {
                failure.worktree_info = Some(worktree_info);
            }

            state.failed_count += 1;
        }
        _ => {}
    }

    // Store result
    state.agent_results.insert(item_id.clone(), result);
    state.completed_agents.insert(item_id.clone());

    // Remove from pending
    state.pending_items.retain(|id| id != &item_id);

    // Update metadata
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;

    state
}

/// Determine if job should transition to reduce phase (pure)
pub fn should_transition_to_reduce(state: &MapReduceJobState) -> bool {
    state.pending_items.is_empty() && state.completed_agents.len() == state.total_items
}

/// Determine if job is complete (pure)
pub fn is_job_complete(state: &MapReduceJobState) -> bool {
    state.is_complete
}

/// Check if map phase is complete (pure)
pub fn is_map_phase_complete(state: &MapReduceJobState) -> bool {
    state.pending_items.is_empty() && state.completed_agents.len() == state.total_items
}

/// Get items that can be retried (pure)
pub fn get_retriable_items(state: &MapReduceJobState, max_retries: u32) -> Vec<String> {
    state
        .failed_agents
        .iter()
        .filter(|(_, failure)| failure.attempts < max_retries)
        .map(|(id, _)| id.clone())
        .collect()
}

/// Start reduce phase (pure)
pub fn start_reduce_phase(mut state: MapReduceJobState) -> MapReduceJobState {
    state.reduce_phase_state = Some(ReducePhaseState {
        started: true,
        completed: false,
        executed_commands: Vec::new(),
        output: None,
        error: None,
        started_at: Some(Utc::now()),
        completed_at: None,
    });
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;
    state
}

/// Complete reduce phase (pure)
pub fn complete_reduce_phase(
    mut state: MapReduceJobState,
    output: Option<String>,
) -> MapReduceJobState {
    if let Some(ref mut reduce_state) = state.reduce_phase_state {
        reduce_state.completed = true;
        reduce_state.output = output;
        reduce_state.completed_at = Some(Utc::now());
    }
    state.is_complete = true;
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;
    state
}

/// Mark job as complete (pure)
pub fn mark_complete(mut state: MapReduceJobState) -> MapReduceJobState {
    state.is_complete = true;
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;
    state
}

/// Find a work item by ID (pure)
pub fn find_work_item(state: &MapReduceJobState, item_id: &str) -> Option<Value> {
    // Extract index from item_id (format: "item_0", "item_1", etc.)
    if let Some(idx) = item_id
        .strip_prefix("item_")
        .and_then(|s| s.parse::<usize>().ok())
    {
        if idx < state.work_items.len() {
            return Some(state.work_items[idx].clone());
        }
    }
    None
}

/// Record agent failure (pure)
pub fn record_agent_failure(
    mut state: MapReduceJobState,
    _agent_id: &str,
    item_id: &str,
    error: String,
) -> MapReduceJobState {
    let failure = state
        .failed_agents
        .entry(item_id.to_string())
        .or_insert_with(|| create_initial_failure_record(item_id));

    failure.attempts += 1;
    failure.last_attempt = Utc::now();
    failure.last_error = error;

    state.failed_count += 1;
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;

    state
}

/// Create an initial failure record for a work item (pure)
fn create_initial_failure_record(item_id: &str) -> FailureRecord {
    FailureRecord {
        item_id: item_id.to_string(),
        attempts: 0,
        last_error: String::new(),
        last_attempt: Utc::now(),
        worktree_info: None,
    }
}

/// Extract error message from agent status (pure)
fn extract_error_message(status: &AgentStatus) -> String {
    match status {
        AgentStatus::Failed(err) => err.clone(),
        AgentStatus::Timeout => "Agent execution timed out".to_string(),
        _ => String::new(),
    }
}

/// Extract worktree info from agent result if available (pure)
fn extract_worktree_info(result: &AgentResult) -> Option<WorktreeInfo> {
    match (&result.worktree_path, &result.branch_name) {
        (Some(path), Some(name)) => Some(WorktreeInfo {
            path: path.clone(),
            name: name.clone(),
            branch: result.branch_name.clone(),
            session_id: result.worktree_session_id.clone(),
        }),
        _ => None,
    }
}

/// Update state with setup completion (pure)
pub fn mark_setup_complete(
    mut state: MapReduceJobState,
    output: Option<String>,
) -> MapReduceJobState {
    state.setup_completed = true;
    state.setup_output = output;
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;
    state
}

/// Update state with variables (pure)
pub fn update_variables(
    mut state: MapReduceJobState,
    variables: HashMap<String, Value>,
) -> MapReduceJobState {
    state.variables = variables;
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;
    state
}

/// Set parent worktree (pure)
pub fn set_parent_worktree(
    mut state: MapReduceJobState,
    worktree: Option<String>,
) -> MapReduceJobState {
    state.parent_worktree = worktree;
    state.updated_at = Utc::now();
    state.checkpoint_version += 1;
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;

    fn test_config() -> MapReduceConfig {
        MapReduceConfig {
            max_parallel: 5,
            ..Default::default()
        }
    }

    fn test_agent_result(item_id: &str, status: AgentStatus) -> AgentResult {
        use std::time::Duration;
        AgentResult {
            item_id: item_id.to_string(),
            status,
            output: None,
            commits: vec![],
            files_modified: vec![],
            duration: Duration::from_secs(10),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
            cleanup_status: None,
        }
    }

    #[test]
    fn test_apply_agent_result_success() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        let result = test_agent_result("item-0", AgentStatus::Success);
        state = apply_agent_result(state, result);

        assert_eq!(state.successful_count, 1);
        assert_eq!(state.failed_count, 0);
        assert_eq!(state.completed_agents.len(), 1);
        assert!(state.pending_items.is_empty());
        assert_eq!(state.checkpoint_version, 1);
    }

    #[test]
    fn test_apply_agent_result_failure() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        let result = test_agent_result("item-0", AgentStatus::Failed("test error".to_string()));
        state = apply_agent_result(state, result);

        assert_eq!(state.successful_count, 0);
        assert_eq!(state.failed_count, 1);
        assert_eq!(state.failed_agents.len(), 1);
        assert!(state.failed_agents.contains_key("item-0"));
        assert_eq!(
            state.failed_agents.get("item-0").unwrap().last_error,
            "test error"
        );
    }

    #[test]
    fn test_should_transition_to_reduce() {
        let state_ready = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: vec!["item-0".to_string()].into_iter().collect(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 1,
            successful_count: 1,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        assert!(should_transition_to_reduce(&state_ready));

        let mut state_not_ready = state_ready.clone();
        state_not_ready.pending_items.push("item-1".to_string());

        assert!(!should_transition_to_reduce(&state_not_ready));
    }

    #[test]
    fn test_get_retriable_items() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null; 3],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 3,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        // Add some failed items
        state.failed_agents.insert(
            "item-0".to_string(),
            FailureRecord {
                item_id: "item-0".to_string(),
                attempts: 2,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        state.failed_agents.insert(
            "item-1".to_string(),
            FailureRecord {
                item_id: "item-1".to_string(),
                attempts: 5,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        let retriable = get_retriable_items(&state, 3);
        assert_eq!(retriable.len(), 1); // item-0 has 2 attempts, max is 3, so retriable

        let retriable = get_retriable_items(&state, 2);
        assert_eq!(retriable.len(), 0); // Both items have >= 2 attempts

        let retriable = get_retriable_items(&state, 6);
        assert_eq!(retriable.len(), 2); // Both items retriable
    }

    #[test]
    fn test_start_reduce_phase() {
        let state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        let state = start_reduce_phase(state);

        assert!(state.reduce_phase_state.is_some());
        assert!(state.reduce_phase_state.as_ref().unwrap().started);
        assert!(!state.reduce_phase_state.as_ref().unwrap().completed);
        assert_eq!(state.checkpoint_version, 1);
    }

    #[test]
    fn test_complete_reduce_phase() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: Some(ReducePhaseState {
                started: true,
                completed: false,
                executed_commands: vec![],
                output: None,
                error: None,
                started_at: Some(Utc::now()),
                completed_at: None,
            }),
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = complete_reduce_phase(state, Some("output".to_string()));

        assert!(state.is_complete);
        assert!(state.reduce_phase_state.as_ref().unwrap().completed);
        assert_eq!(
            state.reduce_phase_state.as_ref().unwrap().output,
            Some("output".to_string())
        );
    }

    #[test]
    fn test_mark_complete() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = mark_complete(state);

        assert!(state.is_complete);
        assert_eq!(state.checkpoint_version, 1);
    }

    #[test]
    fn test_find_work_item() {
        let state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![
                Value::String("item0".to_string()),
                Value::String("item1".to_string()),
            ],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 2,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        assert_eq!(
            find_work_item(&state, "item_0"),
            Some(Value::String("item0".to_string()))
        );
        assert_eq!(
            find_work_item(&state, "item_1"),
            Some(Value::String("item1".to_string()))
        );
        assert_eq!(find_work_item(&state, "item_99"), None);
        assert_eq!(find_work_item(&state, "invalid"), None);
    }

    #[test]
    fn test_record_agent_failure() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        state = record_agent_failure(state, "agent-0", "item-0", "test error".to_string());

        assert_eq!(state.failed_count, 1);
        assert!(state.failed_agents.contains_key("item-0"));
        assert_eq!(
            state.failed_agents.get("item-0").unwrap().last_error,
            "test error"
        );
        assert_eq!(state.failed_agents.get("item-0").unwrap().attempts, 1);
    }

    #[test]
    fn test_mark_setup_complete() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = mark_setup_complete(state, Some("setup output".to_string()));

        assert!(state.setup_completed);
        assert_eq!(state.setup_output, Some("setup output".to_string()));
        assert_eq!(state.checkpoint_version, 1);
    }

    #[test]
    fn test_update_variables() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), Value::String("value1".to_string()));

        state = update_variables(state, vars);

        assert_eq!(state.variables.len(), 1);
        assert_eq!(
            state.variables.get("key1"),
            Some(&Value::String("value1".to_string()))
        );
    }

    #[test]
    fn test_set_parent_worktree() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = set_parent_worktree(state, Some("worktree-123".to_string()));

        assert_eq!(state.parent_worktree, Some("worktree-123".to_string()));
        assert_eq!(state.checkpoint_version, 1);
    }

    #[test]
    fn test_is_job_complete() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        assert!(!is_job_complete(&state));

        state.is_complete = true;
        assert!(is_job_complete(&state));
    }

    #[test]
    fn test_is_map_phase_complete() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        assert!(!is_map_phase_complete(&state));

        state.pending_items.clear();
        assert!(!is_map_phase_complete(&state));

        state.completed_agents.insert("item-0".to_string());
        assert!(is_map_phase_complete(&state));
    }

    #[test]
    fn test_apply_agent_result_with_timeout() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        let result = test_agent_result("item-0", AgentStatus::Timeout);
        state = apply_agent_result(state, result);

        assert_eq!(state.failed_count, 1);
        assert_eq!(state.successful_count, 0);
        assert!(state.failed_agents.contains_key("item-0"));
        assert!(state.pending_items.is_empty());
    }

    #[test]
    fn test_apply_multiple_agent_results() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null, Value::Null, Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![
                "item-0".to_string(),
                "item-1".to_string(),
                "item-2".to_string(),
            ],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 3,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = apply_agent_result(state, test_agent_result("item-0", AgentStatus::Success));
        state = apply_agent_result(
            state,
            test_agent_result("item-1", AgentStatus::Failed("error".to_string())),
        );
        state = apply_agent_result(state, test_agent_result("item-2", AgentStatus::Success));

        assert_eq!(state.successful_count, 2);
        assert_eq!(state.failed_count, 1);
        assert_eq!(state.completed_agents.len(), 3);
        assert!(state.pending_items.is_empty());
    }

    #[test]
    fn test_checkpoint_version_increments() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        assert_eq!(state.checkpoint_version, 0);

        state = apply_agent_result(state, test_agent_result("item-0", AgentStatus::Success));
        assert_eq!(state.checkpoint_version, 1);

        state = mark_complete(state);
        assert_eq!(state.checkpoint_version, 2);
    }

    #[test]
    fn test_reduce_phase_completion() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: Some(ReducePhaseState {
                started: true,
                completed: false,
                executed_commands: vec![],
                output: None,
                error: None,
                started_at: Some(Utc::now()),
                completed_at: None,
            }),
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = complete_reduce_phase(state, Some("final output".to_string()));

        assert!(state.is_complete);
        assert!(state.reduce_phase_state.as_ref().unwrap().completed);
        assert_eq!(
            state.reduce_phase_state.as_ref().unwrap().output,
            Some("final output".to_string())
        );
        assert!(state
            .reduce_phase_state
            .as_ref()
            .unwrap()
            .completed_at
            .is_some());
    }

    #[test]
    fn test_update_empty_variables() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = update_variables(state, HashMap::new());
        assert!(state.variables.is_empty());
        assert_eq!(state.checkpoint_version, 1);
    }

    #[test]
    fn test_mark_setup_complete_with_output() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = mark_setup_complete(state, Some("setup complete".to_string()));

        assert!(state.setup_completed);
        assert_eq!(state.setup_output, Some("setup complete".to_string()));
    }

    #[test]
    fn test_mark_setup_complete_without_output() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 0,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 0,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
            item_retry_counts: HashMap::new(),
        };

        state = mark_setup_complete(state, None);

        assert!(state.setup_completed);
        assert!(state.setup_output.is_none());
    }

    #[test]
    fn test_failed_agent_retry_count_increments() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        // First failure
        state = apply_agent_result(
            state,
            test_agent_result("item-0", AgentStatus::Failed("error 1".to_string())),
        );
        assert_eq!(state.failed_agents.get("item-0").unwrap().attempts, 1);

        // Make it pending again for retry
        state.pending_items.push("item-0".to_string());
        state.completed_agents.remove("item-0");

        // Second failure
        state = apply_agent_result(
            state,
            test_agent_result("item-0", AgentStatus::Failed("error 2".to_string())),
        );
        assert_eq!(state.failed_agents.get("item-0").unwrap().attempts, 2);
    }

    #[test]
    fn test_remove_failed_agent_on_success() {
        let mut state = MapReduceJobState {
            job_id: "job-1".to_string(),
            config: test_config(),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![Value::Null],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
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
        };

        // First failure
        state = apply_agent_result(
            state,
            test_agent_result("item-0", AgentStatus::Failed("error".to_string())),
        );
        assert!(state.failed_agents.contains_key("item-0"));

        // Make it pending again for retry
        state.pending_items.push("item-0".to_string());
        state.completed_agents.remove("item-0");

        // Success on retry - should remove from failed_agents
        state = apply_agent_result(state, test_agent_result("item-0", AgentStatus::Success));
        assert!(!state.failed_agents.contains_key("item-0"));
        assert_eq!(state.successful_count, 1);
    }
}
