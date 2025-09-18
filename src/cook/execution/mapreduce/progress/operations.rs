//! Agent operation tracking for progress monitoring

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Represents the current operation being performed by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentOperation {
    /// Agent is idle
    Idle,
    /// Agent is performing setup
    Setup(String),
    /// Agent is executing a Claude command
    Claude(String),
    /// Agent is executing a shell command
    Shell(String),
    /// Agent is running tests
    Test(String),
    /// Agent is executing a handler
    Handler(String),
    /// Agent is retrying a failed item
    Retrying(String, u32),
    /// Agent has completed processing
    Complete,
}

impl AgentOperation {
    /// Check if the operation indicates the agent is busy
    pub fn is_busy(&self) -> bool {
        !matches!(self, AgentOperation::Idle | AgentOperation::Complete)
    }

    /// Get a short description of the operation
    pub fn description(&self) -> String {
        match self {
            AgentOperation::Idle => "Idle".to_string(),
            AgentOperation::Setup(cmd) => format!("Setup: {}", cmd),
            AgentOperation::Claude(cmd) => format!("Claude: {}", cmd),
            AgentOperation::Shell(cmd) => format!("Shell: {}", cmd),
            AgentOperation::Test(cmd) => format!("Test: {}", cmd),
            AgentOperation::Handler(name) => format!("Handler: {}", name),
            AgentOperation::Retrying(item, attempt) => {
                format!("Retry {} (#{}))", item, attempt)
            }
            AgentOperation::Complete => "Complete".to_string(),
        }
    }

    /// Get the operation type as a string
    pub fn operation_type(&self) -> &'static str {
        match self {
            AgentOperation::Idle => "idle",
            AgentOperation::Setup(_) => "setup",
            AgentOperation::Claude(_) => "claude",
            AgentOperation::Shell(_) => "shell",
            AgentOperation::Test(_) => "test",
            AgentOperation::Handler(_) => "handler",
            AgentOperation::Retrying(_, _) => "retry",
            AgentOperation::Complete => "complete",
        }
    }
}

/// Tracks agent operations across the MapReduce execution
pub struct AgentOperationTracker {
    /// Current operations for each agent
    operations: Arc<RwLock<HashMap<usize, AgentOperationState>>>,
    /// Operation history for metrics
    history: Arc<RwLock<Vec<OperationHistoryEntry>>>,
    /// Maximum history entries to keep
    max_history: usize,
}

/// State of an agent's operation
#[derive(Debug, Clone)]
pub struct AgentOperationState {
    /// Current operation
    pub operation: AgentOperation,
    /// When the operation started
    pub started_at: Instant,
    /// Items processed by this agent
    pub items_processed: usize,
    /// Failed items for this agent
    pub items_failed: usize,
    /// Total time spent on operations
    pub total_operation_time: std::time::Duration,
}

impl Default for AgentOperationState {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOperationState {
    /// Create a new idle agent state
    pub fn new() -> Self {
        Self {
            operation: AgentOperation::Idle,
            started_at: Instant::now(),
            items_processed: 0,
            items_failed: 0,
            total_operation_time: std::time::Duration::ZERO,
        }
    }

    /// Get the duration of the current operation
    pub fn current_duration(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Get average time per item
    pub fn average_time_per_item(&self) -> Option<std::time::Duration> {
        if self.items_processed == 0 {
            return None;
        }
        Some(self.total_operation_time / self.items_processed as u32)
    }
}

/// Entry in the operation history
#[derive(Debug, Clone)]
pub struct OperationHistoryEntry {
    /// Agent index
    pub agent_index: usize,
    /// Operation that was performed
    pub operation: AgentOperation,
    /// When it started
    pub started_at: Instant,
    /// When it ended
    pub ended_at: Instant,
    /// Whether it succeeded
    pub success: bool,
}

impl OperationHistoryEntry {
    /// Get the duration of this operation
    pub fn duration(&self) -> std::time::Duration {
        self.ended_at.duration_since(self.started_at)
    }
}

impl Default for AgentOperationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOperationTracker {
    /// Create a new operation tracker
    pub fn new() -> Self {
        Self::with_max_history(1000)
    }

    /// Create a tracker with specified maximum history
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            operations: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_history,
        }
    }

    /// Start a new operation for an agent
    pub async fn start_operation(&self, agent_index: usize, operation: AgentOperation) {
        let mut operations = self.operations.write().await;

        // Record the end of the previous operation if any
        if let Some(state) = operations.get(&agent_index) {
            if state.operation.is_busy() {
                let entry = OperationHistoryEntry {
                    agent_index,
                    operation: state.operation.clone(),
                    started_at: state.started_at,
                    ended_at: Instant::now(),
                    success: true, // Assume success if starting a new operation
                };
                self.add_history_entry(entry).await;
            }
        }

        // Start the new operation
        let state = operations
            .entry(agent_index)
            .or_insert_with(AgentOperationState::new);

        state.operation = operation;
        state.started_at = Instant::now();
    }

    /// Complete the current operation for an agent
    pub async fn complete_operation(&self, agent_index: usize, success: bool) {
        let mut operations = self.operations.write().await;

        if let Some(state) = operations.get_mut(&agent_index) {
            // Record history
            let entry = OperationHistoryEntry {
                agent_index,
                operation: state.operation.clone(),
                started_at: state.started_at,
                ended_at: Instant::now(),
                success,
            };

            // Update state
            let duration = entry.duration();
            state.total_operation_time += duration;

            if success {
                state.items_processed += 1;
            } else {
                state.items_failed += 1;
            }

            state.operation = AgentOperation::Complete;

            // Add to history
            self.add_history_entry(entry).await;
        }
    }

    /// Add an entry to the history
    async fn add_history_entry(&self, entry: OperationHistoryEntry) {
        let mut history = self.history.write().await;
        history.push(entry);

        // Trim history if needed
        if history.len() > self.max_history {
            let drain_count = history.len() - self.max_history;
            history.drain(0..drain_count);
        }
    }

    /// Get the current operation for an agent
    pub async fn get_operation(&self, agent_index: usize) -> Option<AgentOperation> {
        let operations = self.operations.read().await;
        operations
            .get(&agent_index)
            .map(|state| state.operation.clone())
    }

    /// Get all current operations
    pub async fn get_all_operations(&self) -> HashMap<usize, AgentOperation> {
        let operations = self.operations.read().await;
        operations
            .iter()
            .map(|(idx, state)| (*idx, state.operation.clone()))
            .collect()
    }

    /// Get operation state for an agent
    pub async fn get_state(&self, agent_index: usize) -> Option<AgentOperationState> {
        let operations = self.operations.read().await;
        operations.get(&agent_index).cloned()
    }

    /// Get all operation states
    pub async fn get_all_states(&self) -> HashMap<usize, AgentOperationState> {
        let operations = self.operations.read().await;
        operations.clone()
    }

    /// Get operation history
    pub async fn get_history(&self) -> Vec<OperationHistoryEntry> {
        let history = self.history.read().await;
        history.clone()
    }

    /// Get statistics about operations
    pub async fn get_statistics(&self) -> OperationStatistics {
        let operations = self.operations.read().await;
        let history = self.history.read().await;

        let total_agents = operations.len();
        let busy_agents = operations
            .values()
            .filter(|state| state.operation.is_busy())
            .count();
        let idle_agents = operations
            .values()
            .filter(|state| matches!(state.operation, AgentOperation::Idle))
            .count();

        let total_processed: usize = operations.values().map(|state| state.items_processed).sum();
        let total_failed: usize = operations.values().map(|state| state.items_failed).sum();

        // Calculate operation type counts from history
        let mut operation_counts = HashMap::new();
        for entry in history.iter() {
            *operation_counts
                .entry(entry.operation.operation_type())
                .or_insert(0) += 1;
        }

        // Calculate average durations by operation type
        let mut operation_durations: HashMap<&'static str, Vec<std::time::Duration>> =
            HashMap::new();
        for entry in history.iter() {
            operation_durations
                .entry(entry.operation.operation_type())
                .or_default()
                .push(entry.duration());
        }

        let average_durations: HashMap<&'static str, std::time::Duration> = operation_durations
            .into_iter()
            .map(|(op_type, durations)| {
                let total: std::time::Duration = durations.iter().sum();
                let avg = total / durations.len() as u32;
                (op_type, avg)
            })
            .collect();

        OperationStatistics {
            total_agents,
            busy_agents,
            idle_agents,
            total_processed,
            total_failed,
            operation_counts,
            average_durations,
        }
    }

    /// Clear all tracked operations
    pub async fn clear(&self) {
        let mut operations = self.operations.write().await;
        let mut history = self.history.write().await;
        operations.clear();
        history.clear();
    }
}

/// Statistics about agent operations
#[derive(Debug, Clone)]
pub struct OperationStatistics {
    /// Total number of agents
    pub total_agents: usize,
    /// Number of busy agents
    pub busy_agents: usize,
    /// Number of idle agents
    pub idle_agents: usize,
    /// Total items processed
    pub total_processed: usize,
    /// Total items failed
    pub total_failed: usize,
    /// Count of each operation type
    pub operation_counts: HashMap<&'static str, usize>,
    /// Average duration by operation type
    pub average_durations: HashMap<&'static str, std::time::Duration>,
}

impl OperationStatistics {
    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.total_processed + self.total_failed;
        if total == 0 {
            return 100.0;
        }
        (self.total_processed as f64 / total as f64) * 100.0
    }

    /// Get agent utilization as a percentage
    pub fn agent_utilization(&self) -> f64 {
        if self.total_agents == 0 {
            return 0.0;
        }
        (self.busy_agents as f64 / self.total_agents as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_operation_tracker() {
        let tracker = AgentOperationTracker::new();

        // Start an operation
        tracker
            .start_operation(0, AgentOperation::Setup("test".to_string()))
            .await;

        // Check it's tracked
        let op = tracker.get_operation(0).await;
        assert!(matches!(op, Some(AgentOperation::Setup(_))));

        // Complete the operation
        tracker.complete_operation(0, true).await;

        // Check state
        let state = tracker.get_state(0).await.unwrap();
        assert_eq!(state.items_processed, 1);
        assert_eq!(state.items_failed, 0);

        // Check history
        let history = tracker.get_history().await;
        assert!(!history.is_empty());
    }

    #[tokio::test]
    async fn test_statistics() {
        let tracker = AgentOperationTracker::new();

        // Set up some operations
        tracker
            .start_operation(0, AgentOperation::Claude("cmd1".to_string()))
            .await;
        tracker
            .start_operation(1, AgentOperation::Shell("cmd2".to_string()))
            .await;
        tracker.start_operation(2, AgentOperation::Idle).await;

        // Get statistics
        let stats = tracker.get_statistics().await;
        assert_eq!(stats.total_agents, 3);
        assert_eq!(stats.busy_agents, 2);
        assert_eq!(stats.idle_agents, 1);
    }

    #[test]
    fn test_agent_operation() {
        let op = AgentOperation::Claude("test command".to_string());
        assert!(op.is_busy());
        assert_eq!(op.operation_type(), "claude");

        let op = AgentOperation::Idle;
        assert!(!op.is_busy());
        assert_eq!(op.operation_type(), "idle");
    }
}
