//! MapReduce executor for parallel workflow execution
//!
//! This module orchestrates parallel execution of workflow steps across multiple
//! agents using isolated git worktrees for fault isolation and parallelism.
//!
//! The module has been decomposed into focused sub-modules following functional
//! programming principles, with each module under 500 lines for maintainability.

// Sub-modules for organized functionality
pub mod agent;
pub mod agent_command_executor;
pub mod aggregation;
pub mod cleanup;
pub mod command;
pub mod coordination;
pub mod dry_run;
pub mod event;
pub mod map_phase;
pub mod noop_writer;
pub mod phases;
pub mod progress;
pub mod reduce_phase;
pub mod resources;
pub mod state;
pub mod types;
pub mod utils;

// Re-export commonly used types for convenience
pub use agent::{AgentLifecycleManager, AgentResult, AgentResultAggregator, AgentStatus};
pub use aggregation::{AggregationSummary, ResultCollector, ResultReducer};
pub use coordination::{MapReduceCoordinator, PhaseOrchestrator, WorkScheduler};
pub use state::StateManager;
pub use types::{
    AgentContext, MapPhase, MapReduceConfig, ReducePhase, ResumeOptions, ResumeResult, SetupPhase,
};
pub use utils::{calculate_map_result_summary, MapResultSummary};

// Standard library imports
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// External crate imports
use chrono::Utc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

// Internal imports
use crate::commands::CommandRegistry;
use crate::cook::execution::dlq::DeadLetterQueue;
use crate::cook::execution::errors::{ErrorContext, MapReduceError, MapReduceResult, SpanInfo};
use crate::cook::execution::events::{EventLogger, EventWriter, JsonlEventWriter};
use crate::cook::execution::interpolation::InterpolationEngine;
use crate::cook::execution::progress::EnhancedProgressTracker;
use crate::cook::execution::progress_tracker::ProgressTracker as NewProgressTracker;
use crate::cook::execution::state::{DefaultJobStateManager, JobStateManager, MapReduceJobState};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{ErrorPolicyExecutor, WorkflowErrorPolicy};
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreePool, WorktreePoolConfig};

// Import from sub-modules
use agent::{DefaultLifecycleManager, DefaultResultAggregator};
use state::persistence::DefaultStateStore;

/// Main MapReduce executor that coordinates all operations
#[allow(dead_code)]
pub struct MapReduceExecutor {
    // Core executors and managers
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    worktree_manager: Arc<WorktreeManager>,
    worktree_pool: Option<Arc<WorktreePool>>,

    // Configuration
    project_root: PathBuf,
    correlation_id: String,
    enable_web_dashboard: bool,
    setup_variables: HashMap<String, String>,

    // Command execution
    interpolation_engine: Arc<Mutex<InterpolationEngine>>,
    command_registry: Arc<CommandRegistry>,
    command_router: Arc<command::CommandRouter>,
    step_executor: Arc<command::StepExecutor>,
    subprocess: Arc<SubprocessManager>,

    // State management
    state_manager: Arc<dyn JobStateManager>,
    enhanced_state_manager: Arc<StateManager>,
    retry_state_manager: Arc<crate::cook::retry_state::RetryStateManager>,

    // Event and error handling
    event_logger: Arc<EventLogger>,
    dlq: Option<Arc<DeadLetterQueue>>,
    error_policy_executor: Option<ErrorPolicyExecutor>,

    // Progress tracking
    enhanced_progress_tracker: Option<Arc<EnhancedProgressTracker>>,
    new_progress_tracker: Option<Arc<NewProgressTracker>>,

    // Agent management
    agent_lifecycle_manager: Arc<dyn AgentLifecycleManager>,
    agent_result_aggregator: Arc<dyn AgentResultAggregator>,

    // Resource management
    resource_manager: Arc<resources::ResourceManager>,

    // Coordination components (new)
    coordinator: Option<Arc<MapReduceCoordinator>>,
    orchestrator: Option<Arc<PhaseOrchestrator>>,
}

#[allow(dead_code)]
impl MapReduceExecutor {
    /// Create a new MapReduce executor
    pub async fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        worktree_manager: Arc<WorktreeManager>,
        project_root: PathBuf,
    ) -> Self {
        // Initialize state management
        let state_manager = Self::initialize_state_manager(&project_root).await;

        // Initialize event logging
        let (event_logger, _job_id) = Self::initialize_event_logger(&project_root).await;

        // Initialize agent managers
        let agent_lifecycle_manager =
            Arc::new(DefaultLifecycleManager::new(worktree_manager.clone()));
        let agent_result_aggregator = Arc::new(DefaultResultAggregator::new());

        // Initialize command system
        let command_registry = Arc::new(CommandRegistry::with_defaults().await);
        let command_router =
            Self::initialize_command_router(claude_executor.clone(), command_registry.clone());

        let interpolation_engine = Arc::new(Mutex::new(InterpolationEngine::new(false)));
        let step_executor = Self::initialize_step_executor(command_router.clone());

        // Initialize resource manager
        let resource_manager = Arc::new(resources::ResourceManager::with_worktree_manager(
            None,
            worktree_manager.clone(),
        ));

        // Initialize enhanced state manager
        let enhanced_state_manager = Arc::new(StateManager::new(Arc::new(
            DefaultStateStore::from_manager(state_manager.clone()),
        )));

        Self {
            claude_executor,
            session_manager,
            user_interaction,
            worktree_manager,
            worktree_pool: None,
            project_root,
            correlation_id: Uuid::new_v4().to_string(),
            enable_web_dashboard: Self::check_web_dashboard_env(),
            setup_variables: HashMap::new(),
            interpolation_engine,
            command_registry,
            command_router,
            step_executor,
            subprocess: Arc::new(SubprocessManager::production()),
            state_manager,
            enhanced_state_manager,
            retry_state_manager: Arc::new(crate::cook::retry_state::RetryStateManager::new()),
            event_logger,
            dlq: None,
            error_policy_executor: None,
            enhanced_progress_tracker: None,
            new_progress_tracker: None,
            agent_lifecycle_manager,
            agent_result_aggregator,
            resource_manager,
            coordinator: None,
            orchestrator: None,
        }
    }

    // === Initialization Helpers (Pure Functions) ===

    /// Initialize state manager with global storage support
    async fn initialize_state_manager(project_root: &Path) -> Arc<dyn JobStateManager> {
        match DefaultJobStateManager::new_with_global(project_root.to_path_buf()).await {
            Ok(manager) => Arc::new(manager),
            Err(e) => {
                warn!(
                    "Failed to create global state manager: {}, falling back to local",
                    e
                );
                let state_dir = project_root.join(".prodigy").join("mapreduce");
                Arc::new(DefaultJobStateManager::new(state_dir))
            }
        }
    }

    /// Initialize event logger with global storage
    async fn initialize_event_logger(project_root: &Path) -> (Arc<EventLogger>, String) {
        let job_id = format!("mapreduce-{}", Utc::now().format("%Y%m%d_%H%M%S"));

        let event_logger =
            match crate::storage::create_global_event_logger(project_root, &job_id).await {
                Ok(logger) => {
                    info!("Using global event storage for job: {}", job_id);
                    Arc::new(logger)
                }
                Err(e) => {
                    warn!(
                        "Failed to create global event logger: {}, using fallback",
                        e
                    );
                    Self::create_fallback_event_logger().await
                }
            };

        (event_logger, job_id)
    }

    /// Create fallback event logger
    async fn create_fallback_event_logger() -> Arc<EventLogger> {
        let temp_path = std::env::temp_dir().join("prodigy_events.jsonl");
        let writer: Box<dyn EventWriter> = match JsonlEventWriter::new(temp_path.clone()).await {
            Ok(w) => Box::new(w),
            Err(e) => {
                error!("Failed to create fallback event logger: {}", e);
                Box::new(noop_writer::NoOpEventWriter::new())
            }
        };
        Arc::new(EventLogger::new(vec![writer]))
    }

    /// Initialize command router
    fn initialize_command_router(
        claude_executor: Arc<dyn ClaudeExecutor>,
        command_registry: Arc<CommandRegistry>,
    ) -> Arc<command::CommandRouter> {
        let mut router = command::CommandRouter::new();

        router.register(
            "claude".to_string(),
            Arc::new(command::ClaudeCommandExecutor::new(claude_executor)),
        );
        router.register(
            "shell".to_string(),
            Arc::new(command::ShellCommandExecutor::new()),
        );
        router.register(
            "handler".to_string(),
            Arc::new(command::HandlerCommandExecutor::new(command_registry)),
        );

        Arc::new(router)
    }

    /// Initialize step executor
    fn initialize_step_executor(
        command_router: Arc<command::CommandRouter>,
    ) -> Arc<command::StepExecutor> {
        let step_interpolator = Arc::new(command::StepInterpolator::new(Arc::new(Mutex::new(
            command::InterpolationEngine::new(false),
        ))));

        Arc::new(command::StepExecutor::new(
            command_router,
            step_interpolator,
        ))
    }

    /// Check if web dashboard is enabled
    fn check_web_dashboard_env() -> bool {
        std::env::var("PRODIGY_WEB_DASHBOARD")
            .unwrap_or_else(|_| "false".to_string())
            .eq_ignore_ascii_case("true")
    }

    // === Public API ===

    /// Set error handling policy
    pub fn set_error_policy(&mut self, policy: WorkflowErrorPolicy) {
        self.error_policy_executor = Some(ErrorPolicyExecutor::new(policy));
    }

    /// Initialize worktree pool if not already initialized
    fn ensure_pool_initialized(&mut self) {
        if self.worktree_pool.is_none() {
            let config = WorktreePoolConfig::default();
            self.initialize_pool(config);
        }
    }

    /// Initialize worktree pool with configuration
    fn initialize_pool(&mut self, config: WorktreePoolConfig) {
        if self.worktree_pool.is_none() {
            let pool = Arc::new(WorktreePool::new(config, self.worktree_manager.clone()));
            self.worktree_pool = Some(pool.clone());

            // Update resource manager with the new pool
            self.resource_manager = Arc::new(resources::ResourceManager::with_worktree_manager(
                Some(pool),
                self.worktree_manager.clone(),
            ));
        }
    }

    /// Execute a MapReduce workflow
    pub async fn execute(
        &mut self,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
    ) -> MapReduceResult<Vec<AgentResult>> {
        // Create default environment
        let env = ExecutionEnvironment {
            working_dir: Arc::new(self.project_root.clone()),
            project_dir: Arc::new(self.project_root.clone()),
            worktree_name: None,
            session_id: Arc::from(self.correlation_id.as_str()),
        };
        self.execute_with_context(setup, map_phase, reduce, env)
            .await
    }

    /// Execute with custom environment context
    pub async fn execute_with_context(
        &mut self,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
        env: ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        // Ensure coordinator is initialized
        if self.coordinator.is_none() {
            self.coordinator = Some(Arc::new(MapReduceCoordinator::new(
                self.agent_lifecycle_manager.clone(),
                self.enhanced_state_manager.clone(),
                self.user_interaction.clone(),
                self.subprocess.clone(),
                self.project_root.clone(),
            )));
        }

        // Delegate to coordinator
        let coordinator = self.coordinator.as_ref().unwrap();
        coordinator
            .execute_job(setup, map_phase, reduce, &env)
            .await
    }

    /// Resume a MapReduce job from checkpoint
    pub async fn resume_job(&self, job_id: &str) -> MapReduceResult<ResumeResult> {
        self.resume_job_with_options(job_id, ResumeOptions::default())
            .await
    }

    /// Resume with custom options
    pub async fn resume_job_with_options(
        &self,
        job_id: &str,
        options: ResumeOptions,
    ) -> MapReduceResult<ResumeResult> {
        info!("Resuming job {} with options: {:?}", job_id, options);

        // Load checkpoint from state manager
        let checkpoint = self
            .state_manager
            .get_job_state(job_id)
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to load checkpoint: {}", e),
                source: None,
            })?;

        // Validate checkpoint if required
        if !options.skip_validation {
            self.validate_checkpoint(&checkpoint)?;
        }

        // Calculate remaining work
        let (pending_items, _) = self.calculate_pending_items(&checkpoint);

        // Create resume result - use default checkpoint version
        Ok(ResumeResult {
            job_id: job_id.to_string(),
            resumed_from_version: 1, // Default version
            total_items: checkpoint.total_items,
            already_completed: checkpoint.completed_agents.len(),
            remaining_items: pending_items.len(),
            final_results: vec![],
        })
    }

    /// Check if a job can be resumed
    pub async fn can_resume_job(&self, job_id: &str) -> bool {
        match self.state_manager.get_job_state(job_id).await {
            Ok(checkpoint) => !checkpoint.is_complete,
            Err(_) => false,
        }
    }

    /// List all resumable jobs
    pub async fn list_resumable_jobs(&self) -> MapReduceResult<Vec<String>> {
        self.state_manager
            .list_resumable_jobs()
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to list resumable jobs: {}", e),
                source: None,
            })
            .map(|jobs| jobs.into_iter().map(|j| j.job_id).collect())
    }

    // === Helper Methods (Pure Functions) ===

    /// Create error context
    fn create_error_context(&self, span_name: &str) -> ErrorContext {
        ErrorContext {
            correlation_id: self.correlation_id.clone(),
            timestamp: Utc::now(),
            hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string()),
            thread_id: format!("{:?}", std::thread::current().id()),
            span_trace: vec![SpanInfo {
                name: span_name.to_string(),
                start: Utc::now(),
                attributes: HashMap::new(),
            }],
        }
    }

    /// Validate checkpoint integrity
    fn validate_checkpoint(&self, state: &MapReduceJobState) -> MapReduceResult<()> {
        if state.total_items == 0 {
            return Err(MapReduceError::ValidationFailed {
                details: "Checkpoint has no work items (total_items = 0)".to_string(),
                source: None,
            });
        }

        if state.completed_agents.len() > state.total_items {
            return Err(MapReduceError::ValidationFailed {
                details: format!(
                    "More completed agents ({}) than total items ({})",
                    state.completed_agents.len(),
                    state.total_items
                ),
                source: None,
            });
        }

        Ok(())
    }

    /// Calculate pending items from checkpoint
    fn calculate_pending_items(&self, state: &MapReduceJobState) -> (Vec<usize>, Vec<usize>) {
        // Use pending_items directly from state
        let pending: Vec<usize> = (0..state.pending_items.len()).collect();

        // Extract indices from failed agents
        let failed: Vec<usize> = (0..state.failed_agents.len()).collect();

        (pending, failed)
    }
}

// === Tests Module ===

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // TODO: Re-enable tests once mock dependencies are available
    #[ignore]
    #[test]
    fn test_error_context_creation() {
        let executor = create_test_executor();
        let context = executor.create_error_context("test_span");

        assert_eq!(context.correlation_id, executor.correlation_id);
        assert!(!context.span_trace.is_empty());
        assert_eq!(context.span_trace[0].name, "test_span");
    }

    #[ignore]
    #[test]
    fn test_checkpoint_validation() {
        let executor = create_test_executor();

        // Valid checkpoint
        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: "$.items[*]".to_string(),
            max_parallel: 5,
            agent_timeout_secs: None,
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
            max_items: None,
            offset: None,
        };

        let valid_state = MapReduceJobState {
            job_id: "test".to_string(),
            config,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![],
            agent_results: HashMap::new(),
            completed_agents: HashSet::new(),
            failed_agents: HashMap::new(),
            pending_items: vec![],
            checkpoint_version: 1,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 10,
            successful_count: 3,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            setup_completed: false,
            setup_output: None,
            variables: HashMap::new(),
        };

        assert!(executor.validate_checkpoint(&valid_state).is_ok());

        // Invalid: no items
        let invalid_state = MapReduceJobState {
            total_items: 0,
            ..valid_state.clone()
        };

        assert!(executor.validate_checkpoint(&invalid_state).is_err());
    }

    fn create_test_executor() -> MapReduceExecutor {
        // Test helper stub - tests using this are marked as #[ignore]
        // Will be implemented once mock dependencies are available
        unimplemented!("Test helper requires mock dependencies that are not yet available")
    }
}
