//! Phase execution module for MapReduce workflows
//!
//! This module provides a clean separation between phase orchestration and
//! implementation details, making it easier to understand phase transitions,
//! modify execution strategies, and add new phase types.

pub mod coordinator;
pub mod map;
pub mod reduce;
pub mod setup;

use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::variables::VariableStore;
use crate::subprocess::SubprocessManager;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// Re-export key types for convenience
pub use coordinator::{PhaseCoordinator, PhaseTransition};
pub use map::MapPhaseExecutor;
pub use reduce::ReducePhaseExecutor;
pub use setup::SetupPhaseExecutor;

/// Type of phase in a MapReduce workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseType {
    Setup,
    Map,
    Reduce,
}

impl std::fmt::Display for PhaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhaseType::Setup => write!(f, "Setup"),
            PhaseType::Map => write!(f, "Map"),
            PhaseType::Reduce => write!(f, "Reduce"),
        }
    }
}

/// Result from executing a phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    /// The type of phase that was executed
    pub phase_type: PhaseType,
    /// Whether the phase succeeded
    pub success: bool,
    /// Any data produced by the phase
    pub data: Option<Value>,
    /// Error message if the phase failed
    pub error_message: Option<String>,
    /// Metrics collected during phase execution
    pub metrics: PhaseMetrics,
}

/// Metrics collected during phase execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhaseMetrics {
    /// Duration in seconds
    pub duration_secs: f64,
    /// Number of items processed
    pub items_processed: usize,
    /// Number of successful items
    pub items_successful: usize,
    /// Number of failed items
    pub items_failed: usize,
}

/// Context shared between phases
#[derive(Debug, Clone)]
pub struct PhaseContext {
    /// Variables available to the phase
    pub variables: HashMap<String, String>,
    /// Variable store for complex values
    pub variable_store: Arc<VariableStore>,
    /// Results from the map phase (if available)
    pub map_results: Option<Vec<crate::cook::execution::mapreduce::AgentResult>>,
    /// Checkpoint data for resumption
    pub checkpoint: Option<PhaseCheckpoint>,
    /// Environment configuration
    pub environment: ExecutionEnvironment,
    /// Subprocess manager for command execution
    pub subprocess_manager: Arc<SubprocessManager>,
}

impl PhaseContext {
    /// Create a new phase context
    pub fn new(
        environment: ExecutionEnvironment,
        subprocess_manager: Arc<SubprocessManager>,
    ) -> Self {
        Self {
            variables: HashMap::new(),
            variable_store: Arc::new(VariableStore::new()),
            map_results: None,
            checkpoint: None,
            environment,
            subprocess_manager,
        }
    }

    /// Update variables from another source
    pub fn update_variables(&mut self, variables: HashMap<String, String>) {
        self.variables.extend(variables);
    }
}

/// Checkpoint for resuming a phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseCheckpoint {
    /// The phase that was in progress
    pub phase_type: PhaseType,
    /// Progress within the phase
    pub progress: PhaseProgress,
    /// Saved state data
    pub state: Value,
}

/// Progress within a phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseProgress {
    NotStarted,
    InProgress { step: usize, total: usize },
    Completed,
}

/// Error that can occur during phase execution
#[derive(Debug, thiserror::Error)]
pub enum PhaseError {
    #[error("Phase execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("Phase transition error: {message}")]
    TransitionError { message: String },

    #[error("Phase validation failed: {message}")]
    ValidationError { message: String },

    #[error("Phase timeout: {message}")]
    Timeout { message: String },

    #[error("MapReduce error: {0}")]
    MapReduceError(#[from] crate::cook::execution::errors::MapReduceError),
}

/// Trait for executing a phase in a MapReduce workflow
#[async_trait]
pub trait PhaseExecutor: Send + Sync {
    /// Execute the phase
    async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError>;

    /// Get the phase type
    fn phase_type(&self) -> PhaseType;

    /// Check if the phase can be skipped
    fn can_skip(&self, context: &PhaseContext) -> bool {
        // By default, phases cannot be skipped
        false
    }

    /// Validate the context before execution
    fn validate_context(&self, context: &PhaseContext) -> Result<(), PhaseError> {
        // Default validation passes
        Ok(())
    }
}

/// Trait for handling phase transitions
pub trait PhaseTransitionHandler: Send + Sync {
    /// Determine if a phase should be executed
    fn should_execute(&self, phase: PhaseType, context: &PhaseContext) -> bool;

    /// Handle phase completion
    fn on_phase_complete(&self, phase: PhaseType, result: &PhaseResult);

    /// Handle phase error
    fn on_phase_error(&self, phase: PhaseType, error: &PhaseError) -> PhaseTransition;
}

/// Default implementation of phase transition handler
pub struct DefaultTransitionHandler;

impl PhaseTransitionHandler for DefaultTransitionHandler {
    fn should_execute(&self, _phase: PhaseType, _context: &PhaseContext) -> bool {
        true
    }

    fn on_phase_complete(&self, phase: PhaseType, result: &PhaseResult) {
        tracing::info!(
            "Phase {} completed successfully with {} items processed",
            phase,
            result.metrics.items_processed
        );
    }

    fn on_phase_error(&self, phase: PhaseType, error: &PhaseError) -> PhaseTransition {
        tracing::error!("Phase {} failed: {}", phase, error);
        PhaseTransition::Error(format!("{}", error))
    }
}
