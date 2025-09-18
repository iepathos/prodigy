//! Phase coordinator for orchestrating MapReduce workflow execution
//!
//! This module handles phase transitions, state management, and overall
//! workflow coordination.
//!
//! # Phase Transition State Machine
//!
//! The coordinator manages a state machine with the following transitions:
//!
//! ```text
//! ┌─────────┐
//! │  Start  │
//! └────┬────┘
//!      │
//!      v
//! ┌─────────┐  success   ┌─────────┐  success   ┌─────────┐
//! │  Setup  │ ─────────> │   Map   │ ─────────> │ Reduce  │
//! └────┬────┘            └────┬────┘            └────┬────┘
//!      │ skip                 │                       │ skip/success
//!      │                      │ error                 v
//!      │                      │                  ┌──────────┐
//!      │                      └────────────────> │ Complete │
//!      │                                         └──────────┘
//!      │ error                                        ^
//!      v                                              │
//! ┌─────────┐                                         │
//! │  Error  │ <───────────────────────────────────────┘
//! └─────────┘             error
//! ```
//!
//! ## Transition Rules:
//!
//! 1. **Setup Phase**:
//!    - Executes first if defined
//!    - Can be skipped if no setup commands exist
//!    - On success: transitions to Map
//!    - On error: workflow fails (unless custom handler overrides)
//!
//! 2. **Map Phase**:
//!    - Always executes (cannot be skipped)
//!    - Processes work items in parallel
//!    - On success: transitions to Reduce
//!    - On error: behavior depends on error policy
//!
//! 3. **Reduce Phase**:
//!    - Executes after Map if defined
//!    - Can be skipped if no reduce commands or no map results
//!    - On success: workflow completes
//!    - On error: workflow fails
//!
//! ## Custom Transition Handlers
//!
//! The coordinator uses a `PhaseTransitionHandler` to make transition decisions.
//! You can provide a custom handler to override default behavior:
//!
//! ```rust
//! let coordinator = PhaseCoordinator::new(setup, map, reduce, subprocess_mgr)
//!     .with_transition_handler(Box::new(MyCustomHandler));
//! ```

use super::{
    DefaultTransitionHandler, PhaseContext, PhaseError, PhaseExecutor, PhaseResult,
    PhaseTransitionHandler, PhaseType,
};
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::mapreduce::{MapPhase, ReducePhase};
use crate::cook::execution::SetupPhase;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::subprocess::SubprocessManager;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Result of a phase transition decision
#[derive(Debug, Clone)]
pub enum PhaseTransition {
    /// Continue to the specified phase
    Continue(PhaseType),
    /// Skip the specified phase
    Skip(PhaseType),
    /// Workflow is complete
    Complete,
    /// An error occurred
    Error(String),
}

/// Coordinates the execution of phases in a MapReduce workflow
pub struct PhaseCoordinator {
    /// Setup phase executor (if setup phase exists)
    setup_executor: Option<Box<dyn PhaseExecutor>>,
    /// Map phase executor
    map_executor: Box<dyn PhaseExecutor>,
    /// Reduce phase executor (if reduce phase exists)
    reduce_executor: Option<Box<dyn PhaseExecutor>>,
    /// Handler for phase transitions
    transition_handler: Box<dyn PhaseTransitionHandler>,
}

impl PhaseCoordinator {
    /// Create a new phase coordinator
    pub fn new(
        setup_phase: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce_phase: Option<ReducePhase>,
        _subprocess_manager: Arc<SubprocessManager>,
    ) -> Self {
        // Create executors for each phase
        let setup_executor = setup_phase.map(|phase| {
            Box::new(super::setup::SetupPhaseExecutor::new(phase)) as Box<dyn PhaseExecutor>
        });

        let map_executor =
            Box::new(super::map::MapPhaseExecutor::new(map_phase)) as Box<dyn PhaseExecutor>;

        let reduce_executor = reduce_phase.map(|phase| {
            Box::new(super::reduce::ReducePhaseExecutor::new(phase)) as Box<dyn PhaseExecutor>
        });

        Self {
            setup_executor,
            map_executor,
            reduce_executor,
            transition_handler: Box::new(DefaultTransitionHandler),
        }
    }

    /// Set a custom transition handler
    pub fn with_transition_handler(mut self, handler: Box<dyn PhaseTransitionHandler>) -> Self {
        self.transition_handler = handler;
        self
    }

    /// Execute the workflow, coordinating all phases
    ///
    /// This method orchestrates the entire MapReduce workflow by:
    /// 1. Executing phases in order (Setup -> Map -> Reduce)
    /// 2. Managing transitions based on phase results
    /// 3. Handling errors according to the transition handler
    ///
    /// # State Machine Flow
    ///
    /// The execution follows this flow:
    /// - Setup phase runs first (if defined)
    /// - Map phase always runs (unless Setup fails)
    /// - Reduce phase runs last (if defined and Map succeeds)
    ///
    /// # Returns
    ///
    /// Returns the final `PhaseResult` from the last executed phase,
    /// or an error if the workflow fails.
    pub async fn execute_workflow(
        &self,
        environment: ExecutionEnvironment,
        subprocess_manager: Arc<SubprocessManager>,
    ) -> MapReduceResult<PhaseResult> {
        let mut context = PhaseContext::new(environment, subprocess_manager);
        let mut workflow_result;

        // Execute setup phase if present
        if let Some(setup) = &self.setup_executor {
            match self.execute_phase(setup.as_ref(), &mut context).await {
                Ok(result) => {
                    info!("Setup phase completed successfully");
                    self.transition_handler
                        .on_phase_complete(PhaseType::Setup, &result);
                }
                Err(error) => {
                    warn!("Setup phase failed: {}", error);
                    let transition = self
                        .transition_handler
                        .on_phase_error(PhaseType::Setup, &error);
                    if matches!(transition, PhaseTransition::Error(_)) {
                        return Err(error.into());
                    }
                }
            }
        }

        // Execute map phase
        match self
            .execute_phase(self.map_executor.as_ref(), &mut context)
            .await
        {
            Ok(result) => {
                info!(
                    "Map phase completed with {} items processed",
                    result.metrics.items_processed
                );
                self.transition_handler
                    .on_phase_complete(PhaseType::Map, &result);
                workflow_result = Some(result);
            }
            Err(error) => {
                warn!("Map phase failed: {}", error);
                let transition = self
                    .transition_handler
                    .on_phase_error(PhaseType::Map, &error);
                return match transition {
                    PhaseTransition::Error(msg) => Err(MapReduceError::General {
                        message: msg,
                        source: None,
                    }),
                    _ => Err(error.into()),
                };
            }
        }

        // Execute reduce phase if present and if map phase succeeded
        if let Some(reduce) = &self.reduce_executor {
            if context.map_results.is_some() {
                match self.execute_phase(reduce.as_ref(), &mut context).await {
                    Ok(result) => {
                        info!("Reduce phase completed successfully");
                        self.transition_handler
                            .on_phase_complete(PhaseType::Reduce, &result);
                        workflow_result = Some(result);
                    }
                    Err(error) => {
                        warn!("Reduce phase failed: {}", error);
                        let transition = self
                            .transition_handler
                            .on_phase_error(PhaseType::Reduce, &error);
                        if matches!(transition, PhaseTransition::Error(_)) {
                            return Err(error.into());
                        }
                    }
                }
            } else {
                debug!("Skipping reduce phase - no map results available");
            }
        }

        workflow_result.ok_or_else(|| MapReduceError::General {
            message: "No phases were executed successfully".to_string(),
            source: None,
        })
    }

    /// Execute a single phase
    async fn execute_phase(
        &self,
        executor: &dyn PhaseExecutor,
        context: &mut PhaseContext,
    ) -> Result<PhaseResult, PhaseError> {
        let phase_type = executor.phase_type();

        // Check if phase should be executed
        if !self.transition_handler.should_execute(phase_type, context) {
            debug!("Skipping phase {} based on transition handler", phase_type);
            return Ok(PhaseResult {
                phase_type,
                success: true,
                data: None,
                error_message: Some(format!("Phase {} was skipped", phase_type)),
                metrics: Default::default(),
            });
        }

        // Check if phase can be skipped
        if executor.can_skip(context) {
            debug!("Skipping phase {} - can_skip returned true", phase_type);
            return Ok(PhaseResult {
                phase_type,
                success: true,
                data: None,
                error_message: Some(format!("Phase {} was skipped", phase_type)),
                metrics: Default::default(),
            });
        }

        // Validate context
        executor.validate_context(context)?;

        // Execute the phase
        info!("Starting execution of {} phase", phase_type);
        let start_time = std::time::Instant::now();

        let result = executor.execute(context).await?;

        let duration = start_time.elapsed();
        info!(
            "{} phase completed in {:.2}s",
            phase_type,
            duration.as_secs_f64()
        );

        Ok(result)
    }

    /// Resume execution from a checkpoint
    pub async fn resume_from_checkpoint(
        &self,
        checkpoint: super::PhaseCheckpoint,
        environment: ExecutionEnvironment,
        subprocess_manager: Arc<SubprocessManager>,
    ) -> MapReduceResult<PhaseResult> {
        let mut context = PhaseContext::new(environment, subprocess_manager);
        context.checkpoint = Some(checkpoint.clone());

        // Determine which phase to resume from
        let starting_phase = match checkpoint.phase_type {
            PhaseType::Setup => {
                if let Some(setup) = &self.setup_executor {
                    return self
                        .execute_phase(setup.as_ref(), &mut context)
                        .await
                        .map_err(|e| e.into());
                }
                PhaseType::Map
            }
            PhaseType::Map => PhaseType::Map,
            PhaseType::Reduce => {
                if let Some(reduce) = &self.reduce_executor {
                    return self
                        .execute_phase(reduce.as_ref(), &mut context)
                        .await
                        .map_err(|e| e.into());
                }
                return Ok(PhaseResult {
                    phase_type: PhaseType::Reduce,
                    success: true,
                    data: None,
                    error_message: Some("Reduce phase already completed".to_string()),
                    metrics: Default::default(),
                });
            }
        };

        // Resume from the appropriate phase
        match starting_phase {
            PhaseType::Map => {
                self.execute_workflow(
                    context.environment.clone(),
                    context.subprocess_manager.clone(),
                )
                .await
            }
            _ => Err(MapReduceError::General {
                message: format!("Cannot resume from {} phase", starting_phase),
                source: None,
            }),
        }
    }
}
