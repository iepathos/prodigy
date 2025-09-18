//! Phase execution module for MapReduce workflows
//!
//! This module provides a clean separation between phase orchestration and
//! implementation details, making it easier to understand phase transitions,
//! modify execution strategies, and add new phase types.
//!
//! # Adding Custom Phase Types
//!
//! To add a new custom phase type to the MapReduce workflow:
//!
//! 1. **Define the Phase Type**: Add a new variant to the `PhaseType` enum:
//!    ```rust
//!    pub enum PhaseType {
//!        Setup,
//!        Map,
//!        Reduce,
//!        PostProcess, // Your custom phase
//!    }
//!    ```
//!
//! 2. **Implement PhaseExecutor**: Create a new executor for your phase:
//!    ```rust
//!    pub struct PostProcessPhaseExecutor {
//!        // Phase configuration
//!    }
//!
//!    #[async_trait]
//!    impl PhaseExecutor for PostProcessPhaseExecutor {
//!        async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError> {
//!            // Implementation
//!        }
//!
//!        fn phase_type(&self) -> PhaseType {
//!            PhaseType::PostProcess
//!        }
//!    }
//!    ```
//!
//! 3. **Update Coordinator**: Add support in PhaseCoordinator for the new phase
//!
//! 4. **Handle Transitions**: Update transition logic to include the new phase
//!
//! # Phase Transition State Machine
//!
//! The MapReduce workflow follows a deterministic state machine for phase transitions:
//!
//! ```text
//! [Start] → [Setup] → [Map] → [Reduce] → [Complete]
//!            ↓         ↓        ↓          ↑
//!         [Skip]    [Skip]   [Skip] -------┘
//!            ↓         ↓        ↓
//!         [Error] ← [Error] ← [Error]
//! ```
//!
//! ## Transition Rules:
//!
//! - **Setup Phase**:
//!   - Can be skipped if no setup commands are defined
//!   - On success: Transitions to Map phase
//!   - On error: Transitions to Error state (workflow fails)
//!
//! - **Map Phase**:
//!   - Cannot be skipped (core phase of MapReduce)
//!   - On success: Transitions to Reduce phase
//!   - On error: Behavior depends on error policy
//!     - `fail_fast`: Transitions to Error state immediately
//!     - `continue_on_error`: Continues processing, then moves to Reduce
//!
//! - **Reduce Phase**:
//!   - Can be skipped if no reduce commands are defined OR no map results
//!   - On success: Transitions to Complete state
//!   - On error: Transitions to Error state
//!
//! ## Custom Transition Handlers:
//!
//! You can customize phase transitions by implementing `PhaseTransitionHandler`:
//!
//! ```rust
//! struct CustomTransitionHandler;
//!
//! impl PhaseTransitionHandler for CustomTransitionHandler {
//!     fn should_execute(&self, phase: PhaseType, context: &PhaseContext) -> bool {
//!         // Custom logic to determine if phase should run
//!         match phase {
//!             PhaseType::Reduce => !context.map_results.is_empty(),
//!             _ => true,
//!         }
//!     }
//!
//!     fn on_phase_error(&self, phase: PhaseType, error: &PhaseError) -> PhaseTransition {
//!         // Custom error handling
//!         match phase {
//!             PhaseType::Setup => PhaseTransition::Skip(PhaseType::Map),
//!             _ => PhaseTransition::Error(error.to_string()),
//!         }
//!     }
//! }
//! ```

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
///
/// Each phase has specific responsibilities and transition rules.
/// The phases execute in order: Setup → Map → Reduce
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseType {
    /// Initial setup phase for preparing the environment
    Setup,
    /// Map phase for parallel processing of work items
    Map,
    /// Reduce phase for aggregating map results
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
///
/// This is the core abstraction for phase execution. Each phase type
/// implements this trait to define its execution behavior.
///
/// # Example Implementation
///
/// ```rust
/// struct CustomPhaseExecutor {
///     config: CustomConfig,
/// }
///
/// #[async_trait]
/// impl PhaseExecutor for CustomPhaseExecutor {
///     async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError> {
///         // Validate inputs
///         self.validate_context(context)?;
///
///         // Execute phase logic
///         let start = std::time::Instant::now();
///         // ... perform work ...
///
///         // Return result
///         Ok(PhaseResult {
///             phase_type: self.phase_type(),
///             success: true,
///             data: Some(json!({"processed": 10})),
///             error_message: None,
///             metrics: PhaseMetrics {
///                 duration_secs: start.elapsed().as_secs_f64(),
///                 items_processed: 10,
///                 items_successful: 10,
///                 items_failed: 0,
///             },
///         })
///     }
///
///     fn phase_type(&self) -> PhaseType {
///         PhaseType::Custom
///     }
/// }
/// ```
#[async_trait]
pub trait PhaseExecutor: Send + Sync {
    /// Execute the phase
    async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError>;

    /// Get the phase type
    fn phase_type(&self) -> PhaseType;

    /// Check if the phase can be skipped
    ///
    /// Override this to define skip conditions for your phase
    fn can_skip(&self, _context: &PhaseContext) -> bool {
        // By default, phases cannot be skipped
        false
    }

    /// Validate the context before execution
    ///
    /// Override this to add phase-specific validation
    fn validate_context(&self, _context: &PhaseContext) -> Result<(), PhaseError> {
        // Default validation passes
        Ok(())
    }
}

/// Trait for handling phase transitions
///
/// Implement this trait to customize how the workflow transitions between phases.
/// This allows for dynamic workflow behavior based on runtime conditions.
///
/// # State Machine Transitions
///
/// The handler controls transitions through these states:
/// - `Continue(PhaseType)`: Move to the specified phase
/// - `Skip(PhaseType)`: Skip to a later phase
/// - `Complete`: Mark workflow as complete
/// - `Error(String)`: Abort workflow with error
pub trait PhaseTransitionHandler: Send + Sync {
    /// Determine if a phase should be executed
    ///
    /// Return false to skip the phase
    fn should_execute(&self, phase: PhaseType, context: &PhaseContext) -> bool;

    /// Handle phase completion
    ///
    /// Called after a phase completes successfully
    fn on_phase_complete(&self, phase: PhaseType, result: &PhaseResult);

    /// Handle phase error
    ///
    /// Determines how to proceed when a phase fails
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

#[cfg(test)]
mod coordinator_test;
#[cfg(test)]
mod map_test;
#[cfg(test)]
mod reduce_test;
#[cfg(test)]
mod setup_test;
