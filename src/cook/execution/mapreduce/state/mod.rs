//! MapReduce state management module
//!
//! Provides comprehensive job state tracking, checkpoint management, and recovery capabilities
//! for MapReduce jobs.

pub mod checkpoint;
pub mod persistence;
pub mod recovery;
pub mod transitions;

#[cfg(test)]
mod tests;

use crate::cook::execution::errors::MapReduceError;
use crate::cook::execution::mapreduce::{AgentResult, MapReduceConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::error;

/// State manager for MapReduce jobs
///
/// Centralizes all state management operations including persistence,
/// checkpointing, recovery, and state transitions.
pub struct StateManager {
    /// Underlying state store for persistence
    store: Arc<dyn StateStore + Send + Sync>,
    /// State machine for managing transitions
    transitions: StateMachine,
    /// Audit log of state events
    audit_log: Arc<RwLock<Vec<StateEvent>>>,
}

/// Trait for state storage implementations
#[async_trait::async_trait]
pub trait StateStore: Send + Sync {
    /// Save job state
    async fn save(&self, state: &JobState) -> Result<(), StateError>;

    /// Load job state
    async fn load(&self, job_id: &str) -> Result<Option<JobState>, StateError>;

    /// List all job summaries
    async fn list(&self) -> Result<Vec<JobSummary>, StateError>;

    /// Delete job state
    async fn delete(&self, job_id: &str) -> Result<(), StateError>;
}

/// Extended job state with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobState {
    /// Unique job identifier
    pub id: String,
    /// Current phase of execution
    pub phase: PhaseType,
    /// Current checkpoint if any
    pub checkpoint: Option<Checkpoint>,
    /// Set of processed item IDs
    pub processed_items: HashSet<String>,
    /// Failed items with error details
    pub failed_items: Vec<String>,
    /// Workflow variables
    pub variables: HashMap<String, Value>,
    /// Job creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Job configuration
    pub config: MapReduceConfig,
    /// Agent results
    pub agent_results: HashMap<String, AgentResult>,
    /// Whether job is complete
    pub is_complete: bool,
    /// Total number of work items
    pub total_items: usize,
}

/// Checkpoint for job recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Phase when checkpoint was created
    pub phase: PhaseType,
    /// Items that have been processed
    pub items_processed: Vec<String>,
    /// Results from completed agents
    pub agent_results: Vec<AgentResult>,
    /// Checkpoint creation timestamp
    pub timestamp: DateTime<Utc>,
    /// Checksum for validation
    pub checksum: String,
    /// Version number
    pub version: u32,
}

/// Job summary for listing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    /// Job identifier
    pub job_id: String,
    /// Current phase
    pub phase: PhaseType,
    /// Progress information
    pub progress: JobProgress,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Whether job is complete
    pub is_complete: bool,
}

/// Progress tracking for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    /// Total items to process
    pub total_items: usize,
    /// Items completed successfully
    pub completed_items: usize,
    /// Items that failed
    pub failed_items: usize,
    /// Items pending processing
    pub pending_items: usize,
    /// Completion percentage
    pub completion_percentage: f64,
}

/// Phase types in MapReduce execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhaseType {
    /// Setup phase
    Setup,
    /// Map phase
    Map,
    /// Reduce phase
    Reduce,
    /// Completed
    Completed,
    /// Failed
    Failed,
}

/// State transition events for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: StateEventType,
    /// Job ID
    pub job_id: String,
    /// Additional event details
    pub details: Option<String>,
}

/// Types of state events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateEventType {
    /// Job created
    JobCreated,
    /// Phase transition
    PhaseTransition { from: PhaseType, to: PhaseType },
    /// Checkpoint created
    CheckpointCreated { version: u32 },
    /// Recovery started
    RecoveryStarted { checkpoint_version: u32 },
    /// Items processed
    ItemsProcessed { count: usize },
    /// Items failed
    ItemsFailed { count: usize },
    /// Job completed
    JobCompleted,
    /// Job failed
    JobFailed { reason: String },
}

/// Recovery plan for resuming a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPlan {
    /// Phase to resume from
    pub resume_phase: PhaseType,
    /// Items that still need processing
    pub pending_items: Vec<Value>,
    /// Items to skip (already processed)
    pub skip_items: HashSet<String>,
    /// Variables to restore
    pub variables: HashMap<String, Value>,
    /// Agent results to restore
    pub agent_results: HashMap<String, AgentResult>,
}

/// State machine for managing transitions
pub struct StateMachine {
    /// Valid transitions between phases
    transitions: HashMap<PhaseType, Vec<PhaseType>>,
}

/// Errors specific to state management
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// State persistence failed
    #[error("Failed to persist state: {0}")]
    PersistenceError(String),

    /// State loading failed
    #[error("Failed to load state: {0}")]
    LoadError(String),

    /// Invalid state transition
    #[error("Invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: PhaseType, to: PhaseType },

    /// Checkpoint validation failed
    #[error("Checkpoint validation failed: {0}")]
    ValidationError(String),

    /// State not found
    #[error("State not found for job: {0}")]
    NotFound(String),

    /// Concurrent modification error
    #[error("State was modified concurrently")]
    ConcurrentModification,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(store: Arc<dyn StateStore + Send + Sync>) -> Self {
        Self {
            store,
            transitions: StateMachine::new(),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new job state
    pub async fn create_job(
        &self,
        config: &MapReduceConfig,
        job_id: String,
    ) -> Result<JobState, StateError> {
        let state = JobState {
            id: job_id.clone(),
            phase: PhaseType::Setup,
            checkpoint: None,
            processed_items: HashSet::new(),
            failed_items: Vec::new(),
            variables: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            config: config.clone(),
            agent_results: HashMap::new(),
            is_complete: false,
            total_items: 0,
        };

        self.store.save(&state).await?;
        self.log_event(StateEvent {
            timestamp: Utc::now(),
            event_type: StateEventType::JobCreated,
            job_id,
            details: None,
        })
        .await;

        Ok(state)
    }

    /// Update job state with a closure
    pub async fn update_state<F>(&self, job_id: &str, updater: F) -> Result<JobState, StateError>
    where
        F: FnOnce(&mut JobState) -> Result<(), StateError>,
    {
        let mut state = self
            .store
            .load(job_id)
            .await?
            .ok_or_else(|| StateError::NotFound(job_id.to_string()))?;
        let original_phase = state.phase;

        updater(&mut state)?;

        state.updated_at = Utc::now();
        self.store.save(&state).await?;

        if state.phase != original_phase {
            self.log_event(StateEvent {
                timestamp: Utc::now(),
                event_type: StateEventType::PhaseTransition {
                    from: original_phase,
                    to: state.phase,
                },
                job_id: job_id.to_string(),
                details: None,
            })
            .await;
        }

        Ok(state)
    }

    /// Get job state
    pub async fn get_state(&self, job_id: &str) -> Result<Option<JobState>, StateError> {
        self.store.load(job_id).await
    }

    /// List all jobs
    pub async fn list_jobs(&self) -> Result<Vec<JobSummary>, StateError> {
        self.store.list().await
    }

    /// Get state history for a job
    pub async fn get_state_history(&self, job_id: &str) -> Vec<StateEvent> {
        let log = self.audit_log.read().await;
        log.iter()
            .filter(|event| event.job_id == job_id)
            .cloned()
            .collect()
    }

    /// Log a state event
    async fn log_event(&self, event: StateEvent) {
        let mut log = self.audit_log.write().await;
        log.push(event);
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachine {
    /// Create a new state machine with valid transitions
    pub fn new() -> Self {
        let mut transitions = HashMap::new();

        // Define valid transitions
        transitions.insert(PhaseType::Setup, vec![PhaseType::Map, PhaseType::Failed]);
        transitions.insert(
            PhaseType::Map,
            vec![PhaseType::Reduce, PhaseType::Completed, PhaseType::Failed],
        );
        transitions.insert(
            PhaseType::Reduce,
            vec![PhaseType::Completed, PhaseType::Failed],
        );
        transitions.insert(PhaseType::Failed, vec![]); // Terminal state
        transitions.insert(PhaseType::Completed, vec![]); // Terminal state

        Self { transitions }
    }

    /// Check if a transition is valid
    pub fn is_valid_transition(&self, from: PhaseType, to: PhaseType) -> bool {
        self.transitions
            .get(&from)
            .map(|valid| valid.contains(&to))
            .unwrap_or(false)
    }

    /// Get valid next phases
    pub fn get_valid_transitions(&self, from: PhaseType) -> Vec<PhaseType> {
        self.transitions.get(&from).cloned().unwrap_or_default()
    }
}

impl From<StateError> for MapReduceError {
    fn from(err: StateError) -> Self {
        match err {
            StateError::NotFound(job_id) => MapReduceError::JobNotFound { job_id },
            StateError::ValidationError(details) => MapReduceError::CheckpointCorrupted {
                job_id: String::new(),
                version: 0,
                details,
            },
            _ => MapReduceError::General {
                message: err.to_string(),
                source: None,
            },
        }
    }
}
