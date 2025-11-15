//! Core MapReduce execution coordinator
//!
//! This module coordinates the execution of MapReduce jobs,
//! managing phases and resource allocation.

use super::command_executor::CommandExecutor;
use crate::cook::execution::claude::ClaudeExecutorImpl;
use crate::cook::execution::data_pipeline::DataPipeline;
use crate::cook::execution::dlq::DeadLetterQueue;
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::input_source::InputSource;
use crate::cook::execution::mapreduce::{
    agent::{AgentConfig, AgentLifecycleManager, AgentResult, AgentStatus},
    aggregation::{AggregationSummary, CollectionStrategy, ResultCollector},
    dlq_integration,
    event::{EventLogger, MapReduceEvent},
    merge_queue::MergeQueue,
    resources::git::GitOperations,
    retry_tracking,
    state::StateManager,
    timeout::{TimeoutConfig, TimeoutEnforcer},
    types::{MapPhase, ReducePhase, SetupPhase},
};
use crate::cook::execution::runner::RealCommandRunner;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{OnFailureConfig, WorkflowStep};
use crate::subprocess::SubprocessManager;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info, warn};

/// Information about an orphaned worktree from cleanup failure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrphanedWorktree {
    /// Path to the worktree directory
    pub path: PathBuf,
    /// Agent ID that created the worktree
    pub agent_id: String,
    /// Work item ID that was being processed
    pub item_id: String,
    /// Timestamp when the cleanup failure occurred
    pub failed_at: DateTime<Utc>,
    /// Error message from the cleanup failure
    pub error: String,
}

/// Main coordinator for MapReduce execution
pub struct MapReduceCoordinator {
    /// Agent lifecycle manager
    agent_manager: Arc<dyn AgentLifecycleManager>,
    /// State manager for job state
    _state_manager: Arc<StateManager>,
    /// User interaction handler
    user_interaction: Arc<dyn UserInteraction>,
    /// Result collector
    result_collector: Arc<ResultCollector>,
    /// Subprocess manager for command execution
    subprocess: Arc<SubprocessManager>,
    /// Project root directory
    project_root: PathBuf,
    /// Event logger for tracking execution
    event_logger: Arc<EventLogger>,
    /// Job ID for tracking
    job_id: String,
    /// Claude executor for Claude commands (used via command_executor)
    #[allow(dead_code)]
    pub(crate) claude_executor: Arc<dyn ClaudeExecutor>,
    /// Session manager
    _session_manager: Arc<dyn SessionManager>,
    /// Execution mode (normal or dry-run)
    execution_mode: crate::cook::execution::mapreduce::dry_run::ExecutionMode,
    /// Timeout enforcer for agent execution
    timeout_enforcer: Arc<Mutex<Option<Arc<TimeoutEnforcer>>>>,
    /// Merge queue for serializing agent merges
    merge_queue: Arc<MergeQueue>,
    /// Registry of orphaned worktrees from cleanup failures
    orphaned_worktrees: Arc<Mutex<Vec<OrphanedWorktree>>>,
    /// Dead Letter Queue for failed items
    dlq: Arc<DeadLetterQueue>,
    /// Retry counts for work items (for accurate DLQ attempt tracking)
    retry_counts: Arc<tokio::sync::RwLock<HashMap<String, u32>>>,
    /// Command executor for running workflow steps
    command_executor: CommandExecutor,
}

impl MapReduceCoordinator {
    /// Create a new coordinator
    pub fn new(
        agent_manager: Arc<dyn AgentLifecycleManager>,
        state_manager: Arc<StateManager>,
        user_interaction: Arc<dyn UserInteraction>,
        subprocess: Arc<SubprocessManager>,
        project_root: PathBuf,
    ) -> Self {
        Self::with_mode(
            agent_manager,
            state_manager,
            user_interaction,
            subprocess,
            project_root,
            crate::cook::execution::mapreduce::dry_run::ExecutionMode::Normal,
            0, // Default verbosity
        )
    }

    /// Create a new coordinator with execution mode
    pub fn with_mode(
        agent_manager: Arc<dyn AgentLifecycleManager>,
        state_manager: Arc<StateManager>,
        user_interaction: Arc<dyn UserInteraction>,
        subprocess: Arc<SubprocessManager>,
        project_root: PathBuf,
        execution_mode: crate::cook::execution::mapreduce::dry_run::ExecutionMode,
        verbosity: u8,
    ) -> Self {
        let result_collector = Arc::new(ResultCollector::new(CollectionStrategy::InMemory));
        let job_id = format!("mapreduce-{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        let event_logger = Arc::new(EventLogger::new(project_root.clone(), job_id.clone(), None));

        // Create claude executor using the real implementation
        let command_runner = RealCommandRunner::new();
        let claude_executor: Arc<dyn ClaudeExecutor> =
            Arc::new(ClaudeExecutorImpl::new(command_runner));

        // Create session manager - not used but required for struct
        let session_manager = Arc::new(DummySessionManager);

        // Create merge queue for serializing agent merges with Claude support
        let git_ops = Arc::new(GitOperations::new());
        let merge_queue = Arc::new(MergeQueue::new_with_claude(
            git_ops,
            Some(claude_executor.clone()),
            verbosity,
        ));

        // Initialize DLQ for failed items tracking
        let dlq = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                crate::storage::create_global_dlq(&project_root, &job_id, None)
                    .await
                    .unwrap_or_else(|e| {
                        warn!("Failed to create global DLQ: {}, using fallback", e);
                        // Create fallback DLQ with temp path
                        let temp_path = std::env::temp_dir().join("prodigy_dlq");
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                DeadLetterQueue::new(job_id.clone(), temp_path, 1000, 30, None)
                                    .await
                                    .expect("Failed to create fallback DLQ")
                            })
                        })
                    })
            })
        });

        // Create command executor
        let command_executor = CommandExecutor::new(claude_executor.clone(), subprocess.clone());

        Self {
            agent_manager,
            _state_manager: state_manager,
            user_interaction,
            result_collector,
            subprocess,
            project_root,
            event_logger,
            job_id,
            claude_executor,
            _session_manager: session_manager,
            execution_mode,
            timeout_enforcer: Arc::new(Mutex::new(None)),
            merge_queue,
            orphaned_worktrees: Arc::new(Mutex::new(Vec::new())),
            dlq: Arc::new(dlq),
            retry_counts: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            command_executor,
        }
    }

    /// Get the list of orphaned worktrees
    pub async fn get_orphaned_worktrees(&self) -> Vec<OrphanedWorktree> {
        self.orphaned_worktrees.lock().await.clone()
    }

    /// Register an orphaned worktree when cleanup fails
    pub async fn register_orphaned_worktree(&self, orphaned: OrphanedWorktree) {
        warn!(
            "Registered orphaned worktree: {} (agent: {}, item: {})",
            orphaned.path.display(),
            orphaned.agent_id,
            orphaned.item_id
        );
        let mut registry = self.orphaned_worktrees.lock().await;
        registry.push(orphaned);
    }

    /// Execute a complete MapReduce job
    pub async fn execute_job(
        &self,
        setup: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce: Option<ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        info!("Starting MapReduce job execution");

        // Initialize timeout enforcer if configured
        if let Some(timeout_secs) = map_phase.config.agent_timeout_secs {
            let timeout_config =
                map_phase
                    .timeout_config
                    .clone()
                    .unwrap_or_else(|| TimeoutConfig {
                        agent_timeout_secs: Some(timeout_secs),
                        ..TimeoutConfig::default()
                    });
            let mut enforcer = self.timeout_enforcer.lock().await;
            *enforcer = Some(Arc::new(TimeoutEnforcer::new(timeout_config)));
            info!("Timeout enforcement enabled with {}s timeout", timeout_secs);
        } else if let Some(timeout_config) = map_phase.timeout_config.clone() {
            let mut enforcer = self.timeout_enforcer.lock().await;
            *enforcer = Some(Arc::new(TimeoutEnforcer::new(timeout_config)));
            info!("Timeout enforcement enabled with custom configuration");
        }

        // Check if we're in dry-run mode
        if let crate::cook::execution::mapreduce::dry_run::ExecutionMode::DryRun(ref config) =
            self.execution_mode
        {
            return self
                .execute_dry_run(setup.as_ref(), &map_phase, reduce.as_ref(), config)
                .await;
        }

        // Execute setup phase if present
        if let Some(setup_phase) = setup {
            self.execute_setup_phase(setup_phase, env).await?;
        }

        // Load work items
        let work_items = self.load_work_items(&map_phase).await?;

        if work_items.is_empty() {
            warn!("No work items to process");
            return Ok(Vec::new());
        }

        info!("Processing {} work items", work_items.len());

        // Execute map phase
        let map_results = self
            .execute_map_phase_internal(map_phase, work_items, env)
            .await?;

        // Execute reduce phase if present
        if let Some(reduce_phase) = reduce {
            self.execute_reduce_phase(reduce_phase, &map_results, env)
                .await?;
        }

        // SPEC 134: Merge to original branch is handled by orchestrator cleanup with user confirmation
        // No automatic merge happens here. The orchestrator will prompt the user to merge the parent
        // worktree back to the original branch after the workflow completes.
        tracing::info!("MapReduce reduce phase completed. Changes are in parent worktree.");

        Ok(map_results)
    }

    /// Execute in dry-run mode
    async fn execute_dry_run(
        &self,
        setup: Option<&SetupPhase>,
        map_phase: &MapPhase,
        reduce: Option<&ReducePhase>,
        _config: &crate::cook::execution::mapreduce::dry_run::DryRunConfig,
    ) -> MapReduceResult<Vec<AgentResult>> {
        use crate::cook::execution::mapreduce::dry_run::{DryRunValidator, OutputFormatter};

        info!("Executing MapReduce job in dry-run mode");

        // Create validator
        let validator = DryRunValidator::new();

        // Validate the workflow
        match validator
            .validate_workflow_phases(setup.cloned(), map_phase.clone(), reduce.cloned())
            .await
        {
            Ok(report) => {
                // Display the validation report
                let formatter = OutputFormatter::new();
                let output = formatter.format_human(&report);

                // Use user interaction to display the output
                self.user_interaction.display_info(&output);

                if report.errors.is_empty() {
                    self.user_interaction.display_success(
                        "Dry-run validation successful! Workflow is ready to execute.",
                    );
                    Ok(Vec::new()) // Return empty results for dry-run
                } else {
                    self.user_interaction.display_error(&format!(
                        "Dry-run validation failed with {} error(s)",
                        report.errors.len()
                    ));
                    Err(MapReduceError::General {
                        message: format!(
                            "Dry-run validation failed with {} errors",
                            report.errors.len()
                        ),
                        source: None,
                    })
                }
            }
            Err(e) => {
                self.user_interaction
                    .display_error(&format!("Dry-run validation failed: {}", e));
                Err(MapReduceError::General {
                    message: format!("Dry-run validation failed: {}", e),
                    source: None,
                })
            }
        }
    }

    /// Get a displayable name for a workflow step
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn get_step_display_name(step: &WorkflowStep) -> String {
        CommandExecutor::get_step_display_name(step)
    }

    /// Execute the setup phase
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) async fn execute_setup_phase(
        &self,
        setup_phase: SetupPhase,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        info!("Executing setup phase");
        info!(
            "Setup phase executing in directory: {}",
            env.working_dir.display()
        );

        for (index, step) in setup_phase.commands.iter().enumerate() {
            // Get step display name for logging
            let step_name = Self::get_step_display_name(step);

            // Display user-facing progress with the actual command
            self.user_interaction.display_progress(&format!(
                "Setup [{}/{}]: {}",
                index + 1,
                setup_phase.commands.len(),
                step_name
            ));

            info!(
                "Executing setup step {}/{}: {}",
                index + 1,
                setup_phase.commands.len(),
                step_name
            );

            // Log execution context at DEBUG level
            debug!("=== Step Execution Context ===");
            debug!("Step: {:?}", step);
            debug!("Working Directory: {}", env.working_dir.display());
            debug!("Project Directory: {}", self.project_root.display());
            debug!("Worktree: {:?}", env.worktree_name);
            debug!("Session ID: {}", env.session_id);

            // Set environment variables
            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

            debug!("Environment Variables:");
            for (key, value) in &env_vars {
                debug!("  {} = {}", key, value);
            }
            debug!("Actual execution directory: {}", env.working_dir.display());
            debug!("==============================");

            // Execute the step
            let result = self
                .command_executor
                .execute_setup_step(step, env, env_vars)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!(
                        "Setup step {} ({}) failed: {}",
                        index + 1,
                        step_name,
                        e
                    ))
                })?;

            // Display completion
            if result.success {
                self.user_interaction.display_success(&format!(
                    "✓ Setup [{}/{}]: {} completed",
                    index + 1,
                    setup_phase.commands.len(),
                    step_name
                ));
            }

            if !result.success {
                // Handle on_failure if configured
                if let Some(on_failure) = &step.on_failure {
                    self.user_interaction.display_warning(&format!(
                        "Setup step {} failed, executing on_failure handler",
                        index + 1
                    ));

                    // Create empty variables for on_failure (setup phase has no item context)
                    let variables = HashMap::new();

                    let handler_result = Self::handle_on_failure(
                        on_failure,
                        &env.working_dir,
                        &variables,
                        &self.command_executor,
                        &self.user_interaction,
                    )
                    .await?;

                    if !handler_result {
                        return Err(MapReduceError::ProcessingError(format!(
                            "Setup step {} failed and on_failure handler failed",
                            index + 1
                        )));
                    }

                    // on_failure handler succeeded, continue to next step
                    continue;
                }

                // No on_failure handler, build error message and fail
                let mut error_msg = format!("Setup step {} ({}) failed", index + 1, step_name);

                // Add exit code if available
                if let Some(code) = result.exit_code {
                    error_msg.push_str(&format!(" (exit code: {})", code));
                }

                // Add stderr if available
                if !result.stderr.trim().is_empty() {
                    error_msg.push_str(&format!("\nstderr: {}", result.stderr.trim()));
                }

                // Add stdout if available and stderr is empty
                if result.stderr.trim().is_empty() && !result.stdout.trim().is_empty() {
                    error_msg.push_str(&format!("\nstdout: {}", result.stdout.trim()));
                }

                // Add Claude JSON log location if available (most direct path to debugging)
                if let Some(json_log) = &result.json_log_location {
                    error_msg.push_str(&format!("\n\n📝 Claude log: {}", json_log));
                } else if step.claude.is_some() {
                    // Fallback: If Claude command but no direct log, show event logs location
                    if let Ok(repo_name) = crate::storage::extract_repo_name(&self.project_root) {
                        let log_hint = format!(
                            "\n\n💡 Check Claude logs at: ~/.prodigy/events/{}/{}/*.jsonl",
                            repo_name, self.job_id
                        );
                        error_msg.push_str(&log_hint);
                    }
                }

                return Err(MapReduceError::ProcessingError(error_msg));
            }
        }

        info!("Setup phase completed");
        Ok(())
    }

    /// Load work items from input source
    async fn load_work_items(&self, map_phase: &MapPhase) -> MapReduceResult<Vec<Value>> {
        info!("Loading work items from: {}", map_phase.config.input);

        // Create input source
        let input_source =
            InputSource::detect_with_base(&map_phase.config.input, &self.project_root);

        // Load data based on input source type
        let json_data = match input_source {
            InputSource::JsonFile(path) => {
                InputSource::load_json_file(&path, &self.project_root).await?
            }
            InputSource::Command(cmd) => {
                let items =
                    InputSource::execute_command(&cmd, Duration::from_secs(300), &self.subprocess)
                        .await?;
                serde_json::Value::Array(items)
            }
        };

        // Create data pipeline from configuration
        let pipeline = DataPipeline::from_config(
            map_phase.json_path.clone(),
            map_phase.filter.clone(),
            map_phase.sort_by.clone(),
            map_phase.max_items,
        )
        .map_err(|e| MapReduceError::InvalidConfiguration {
            reason: format!("Failed to build data pipeline: {}", e),
            field: "configuration".to_string(),
            value: "configuration".to_string(),
        })?;

        // Process the data through the pipeline
        let items =
            pipeline
                .process(&json_data)
                .map_err(|e| MapReduceError::InvalidConfiguration {
                    reason: format!("Failed to process work items: {}", e),
                    field: "input".to_string(),
                    value: map_phase.config.input.clone(),
                })?;

        debug!("Loaded {} work items", items.len());
        Ok(items)
    }

    /// Execute the map phase
    async fn execute_map_phase_internal(
        &self,
        map_phase: MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        info!("Executing map phase with {} items", work_items.len());

        let total_items = work_items.len();
        let max_parallel = map_phase.config.max_parallel.min(total_items);

        self.user_interaction.display_progress(&format!(
            "Processing {} items with {} parallel agents",
            total_items, max_parallel
        ));

        // Log map phase start
        self.event_logger
            .log_event(MapReduceEvent::map_phase_started(total_items))
            .await
            .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

        // Create semaphore for parallel control
        let semaphore = Arc::new(Semaphore::new(max_parallel));

        // Get the timeout enforcer if configured
        let timeout_enforcer = self.timeout_enforcer.lock().await.clone();

        // Process items in parallel with controlled concurrency
        let agent_futures: Vec<_> = work_items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let sem = Arc::clone(&semaphore);
                let agent_manager = Arc::clone(&self.agent_manager);
                let merge_queue = Arc::clone(&self.merge_queue);
                let event_logger = Arc::clone(&self.event_logger);
                let result_collector = Arc::clone(&self.result_collector);
                let user_interaction = Arc::clone(&self.user_interaction);
                let command_executor = self.command_executor.clone();
                let dlq = Arc::clone(&self.dlq);
                let retry_counts = Arc::clone(&self.retry_counts);
                let map_phase = map_phase.clone();
                let env = env.clone();
                let job_id = self.job_id.clone();
                let timeout_enforcer = timeout_enforcer.clone();

                tokio::spawn(async move {
                    // Acquire semaphore permit
                    let _permit = sem.acquire().await.map_err(|e| {
                        MapReduceError::ProcessingError(format!(
                            "Failed to acquire semaphore: {}",
                            e
                        ))
                    })?;

                    let item_id = format!("item_{}", index);
                    let agent_id = format!("{}_agent_{}", job_id, index);

                    // Log agent start
                    event_logger
                        .log_event(MapReduceEvent::agent_started(
                            agent_id.clone(),
                            item_id.clone(),
                        ))
                        .await
                        .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

                    let start_time = Instant::now();

                    // Clone item for DLQ tracking in case of failure
                    let item_for_dlq = item.clone();

                    // Execute agent with item
                    let result = Self::execute_agent_for_item(
                        &agent_manager,
                        &merge_queue,
                        &agent_id,
                        &item_id,
                        item,
                        &map_phase,
                        &env,
                        &user_interaction,
                        &command_executor,
                        timeout_enforcer.as_ref(),
                        index,
                        total_items,
                    )
                    .await;

                    let duration = start_time.elapsed();

                    // Convert result to AgentResult
                    let agent_result = match result {
                        Ok(agent_result) => {
                            event_logger
                                .log_event(MapReduceEvent::agent_completed(
                                    agent_id.clone(),
                                    item_id.clone(),
                                    chrono::Duration::from_std(duration)
                                        .unwrap_or(chrono::Duration::seconds(0)),
                                    None,
                                ))
                                .await
                                .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

                            agent_result
                        }
                        Err(e) => {
                            event_logger
                                .log_event(MapReduceEvent::agent_failed(
                                    agent_id.clone(),
                                    item_id.clone(),
                                    e.to_string(),
                                ))
                                .await
                                .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

                            AgentResult {
                                item_id: item_id.clone(),
                                status: AgentStatus::Failed(e.to_string()),
                                output: None,
                                commits: vec![],
                                duration: std::time::Duration::from_secs(0),
                                error: Some(e.to_string()),
                                worktree_path: None,
                                branch_name: None,
                                worktree_session_id: Some(agent_id),
                                files_modified: vec![],
                                json_log_location: None,
                                cleanup_status: None,
                            }
                        }
                    };

                    // Add result to collector
                    result_collector.add_result(agent_result.clone()).await;

                    // Add failed items to DLQ (graceful failure - don't break workflow)
                    // Get current attempt number for this item
                    let retry_counts_read = retry_counts.read().await;
                    let attempt_number = retry_tracking::get_item_attempt_number(
                        &item_id,
                        &retry_counts_read,
                    );
                    drop(retry_counts_read); // Release read lock

                    if let Some(dlq_item) =
                        dlq_integration::agent_result_to_dlq_item(&agent_result, &item_for_dlq, attempt_number)
                    {
                        if let Err(e) = dlq.add(dlq_item).await {
                            warn!(
                                "Failed to add item {} to DLQ: {}. Item tracking may be incomplete.",
                                agent_result.item_id, e
                            );
                        } else {
                            info!(
                                "Added failed item {} to DLQ (attempt {})",
                                agent_result.item_id, attempt_number
                            );

                            // Increment retry count in state
                            let mut retry_counts_write = retry_counts.write().await;
                            *retry_counts_write = retry_tracking::increment_retry_count(
                                &item_id,
                                retry_counts_write.clone(),
                            );
                        }
                    }

                    Ok::<AgentResult, MapReduceError>(agent_result)
                })
            })
            .collect();

        // Wait for all agents to complete
        let mut results = Vec::new();
        for future in agent_futures {
            match future.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    warn!("Agent execution failed: {}", e);
                    // Continue processing other agents
                }
                Err(e) => {
                    warn!("Agent task panicked: {}", e);
                    // Continue processing other agents
                }
            }
        }

        // Log map phase completion
        let summary = AggregationSummary::from_results(&results);
        self.event_logger
            .log_event(MapReduceEvent::map_phase_completed(
                summary.successful,
                summary.failed,
            ))
            .await
            .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

        self.display_map_summary(&summary);

        Ok(results)
    }

    /// Register agent timeout with the enforcer
    ///
    /// Attempts to register a timeout for the agent with the enforcer.
    /// Returns None if enforcer is not available or registration fails.
    ///
    /// # Arguments
    /// * `enforcer` - Optional timeout enforcer
    /// * `agent_id` - ID of the agent to register
    /// * `item_id` - ID of the work item being processed
    /// * `commands` - Commands that will be executed by the agent
    ///
    /// # Returns
    /// Optional timeout handle if registration succeeds
    async fn register_agent_timeout(
        enforcer: Option<&Arc<TimeoutEnforcer>>,
        agent_id: &str,
        item_id: &str,
        commands: &[WorkflowStep],
    ) -> Option<crate::cook::execution::mapreduce::timeout::TimeoutHandle> {
        if let Some(enforcer) = enforcer {
            match enforcer
                .register_agent_timeout(agent_id.to_string(), item_id.to_string(), commands)
                .await
            {
                Ok(handle) => Some(handle),
                Err(e) => {
                    warn!("Failed to register timeout for agent {}: {}", agent_id, e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Register command lifecycle events with the enforcer
    ///
    /// Notifies the timeout enforcer of command start or completion.
    /// Logs warnings if notification fails but doesn't propagate errors.
    ///
    /// # Arguments
    /// * `enforcer` - Optional timeout enforcer
    /// * `agent_id` - ID of the agent executing the command
    /// * `index` - Index of the command in the workflow
    /// * `elapsed` - Optional elapsed time (for completion events)
    async fn register_command_lifecycle(
        enforcer: Option<&Arc<TimeoutEnforcer>>,
        agent_id: &str,
        index: usize,
        elapsed: Option<Duration>,
    ) -> MapReduceResult<()> {
        if let Some(enforcer) = enforcer {
            if let Some(duration) = elapsed {
                // Command completion
                let _ = enforcer
                    .register_command_completion(&agent_id.to_string(), index, duration)
                    .await;
            } else {
                // Command start
                let _ = enforcer
                    .register_command_start(&agent_id.to_string(), index)
                    .await;
            }
        }
        Ok(())
    }

    /// Unregister agent timeout with the enforcer
    ///
    /// Removes the timeout registration for a completed agent.
    ///
    /// # Arguments
    /// * `enforcer` - Optional timeout enforcer
    /// * `agent_id` - ID of the agent to unregister
    async fn unregister_agent_timeout(
        enforcer: Option<&Arc<TimeoutEnforcer>>,
        agent_id: &str,
    ) -> MapReduceResult<()> {
        if let Some(enforcer) = enforcer {
            let _ = enforcer
                .unregister_agent_timeout(&agent_id.to_string())
                .await;
        }
        Ok(())
    }

    /// Merge agent changes and cleanup worktree
    ///
    /// Handles merging agent changes back to parent worktree and cleaning up
    /// the agent's worktree. Returns whether merge was successful.
    ///
    /// # Arguments
    /// * `agent_manager` - Agent lifecycle manager
    /// * `merge_queue` - Merge queue for serialized merges
    /// * `handle` - Agent handle with worktree information
    /// * `config` - Agent configuration
    /// * `agent_result` - Result from agent execution
    /// * `env` - Execution environment
    /// * `agent_id` - ID of the agent
    /// * `item_id` - ID of the work item
    ///
    /// # Returns
    /// True if merge was successful, false otherwise
    #[allow(clippy::too_many_arguments)]
    async fn merge_and_cleanup_agent(
        agent_manager: &Arc<dyn AgentLifecycleManager>,
        merge_queue: &Arc<MergeQueue>,
        handle: crate::cook::execution::mapreduce::agent::AgentHandle,
        config: &AgentConfig,
        agent_result: &AgentResult,
        env: &ExecutionEnvironment,
        agent_id: &str,
        item_id: &str,
    ) -> MapReduceResult<bool> {
        if !agent_result.commits.is_empty() {
            // Create branch for the agent
            agent_manager
                .create_agent_branch(handle.worktree_path(), &config.branch_name)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!("Failed to create agent branch: {}", e))
                })?;

            // Submit merge to queue (serialized processing)
            match merge_queue
                .submit_merge(
                    agent_id.to_string(),
                    config.branch_name.clone(),
                    item_id.to_string(),
                    env.clone(),
                )
                .await
            {
                Ok(()) => {
                    info!("Successfully merged agent {} (item {})", agent_id, item_id);
                    // Cleanup the worktree after successful merge
                    // Note: Cleanup failures are non-critical since the work was successfully merged
                    if let Err(e) = agent_manager.cleanup_agent(handle).await {
                        warn!(
                            "Failed to cleanup agent {} after successful merge: {}. \
                             Work was successfully merged, worktree may need manual cleanup.",
                            agent_id, e
                        );
                    }
                    Ok(true)
                }
                Err(e) => {
                    warn!(
                        "Failed to merge agent {} (item {}): {}",
                        agent_id, item_id, e
                    );
                    // Still cleanup the worktree even if merge failed
                    let _ = agent_manager.cleanup_agent(handle).await;
                    Ok(false)
                }
            }
        } else {
            // No commits, just cleanup the worktree
            // Note: Cleanup failures are non-critical, log but don't fail the agent
            if let Err(e) = agent_manager.cleanup_agent(handle).await {
                warn!(
                    "Failed to cleanup agent {} (no commits made): {}. \
                     Worktree may need manual cleanup.",
                    agent_id, e
                );
            }
            Ok(false)
        }
    }

    /// Execute agent commands in the worktree
    ///
    /// Executes all commands for an agent, handling failures and collecting results.
    ///
    /// # Arguments
    /// * `handle` - Agent handle with worktree information
    /// * `commands` - Commands to execute
    /// * `item` - Work item data
    /// * `item_id` - ID of the work item
    /// * `agent_id` - ID of the agent
    /// * `env` - Execution environment
    /// * `command_executor` - Command executor for running steps
    /// * `timeout_enforcer` - Optional timeout enforcer
    /// * `user_interaction` - User interaction handler
    ///
    /// # Returns
    /// Tuple of (output, commits, files_modified)
    #[allow(clippy::too_many_arguments)]
    async fn execute_agent_commands(
        handle: &crate::cook::execution::mapreduce::agent::AgentHandle,
        commands: &[WorkflowStep],
        item: &Value,
        item_id: &str,
        agent_id: &str,
        _env: &ExecutionEnvironment,
        command_executor: &CommandExecutor,
        timeout_enforcer: Option<&Arc<TimeoutEnforcer>>,
        user_interaction: &Arc<dyn UserInteraction>,
    ) -> MapReduceResult<(String, Vec<String>, Vec<String>)> {
        let mut output = String::new();
        let mut all_commits = Vec::new();
        let mut all_files_modified = Vec::new();

        for (index, step) in commands.iter().enumerate() {
            user_interaction.display_progress(&format!(
                "Agent {}: Executing step {}/{}",
                agent_id,
                index + 1,
                commands.len()
            ));

            // Notify timeout enforcer of command start
            Self::register_command_lifecycle(timeout_enforcer, agent_id, index, None).await?;

            let cmd_start = Instant::now();

            // Create variables for interpolation
            let variables = Self::build_item_variables(item, item_id);

            // Execute the step in the agent's worktree
            let step_result = command_executor
                .execute_step_in_worktree(
                    handle.worktree_path(),
                    step,
                    &variables,
                    None, // Map phase doesn't need full context
                )
                .await?;

            // Notify timeout enforcer of command completion
            Self::register_command_lifecycle(
                timeout_enforcer,
                agent_id,
                index,
                Some(cmd_start.elapsed()),
            )
            .await?;

            if !step_result.success {
                // Handle on_failure if configured
                if let Some(on_failure) = &step.on_failure {
                    user_interaction.display_warning(&format!(
                        "Agent {}: Step {} failed, executing on_failure handler",
                        agent_id,
                        index + 1
                    ));

                    // Execute on_failure handler
                    let handler_result = Self::handle_on_failure(
                        on_failure,
                        handle.worktree_path(),
                        &variables,
                        command_executor,
                        user_interaction,
                    )
                    .await?;

                    if !handler_result {
                        return Err(MapReduceError::ProcessingError(format!(
                            "Agent {} step {} failed and on_failure handler failed",
                            agent_id,
                            index + 1
                        )));
                    }
                }
            }

            // Capture output
            if !step_result.stdout.is_empty() {
                output.push_str(&step_result.stdout);
                output.push('\n');
            }

            // Track commits if required
            if step.commit_required {
                // Get actual commits from git in the worktree
                let commits = Self::get_worktree_commits(handle.worktree_path()).await;
                all_commits.extend(commits);

                // Get files modified
                let files = Self::get_worktree_modified_files(handle.worktree_path()).await;
                all_files_modified.extend(files);
            }
        }

        Ok((output, all_commits, all_files_modified))
    }

    /// Build variables for item interpolation
    ///
    /// Extracts fields from a JSON item and creates a HashMap of variables
    /// for template interpolation. Handles different JSON value types and
    /// provides both flattened field access (item.field) and full JSON (item_json).
    ///
    /// # Arguments
    /// * `item` - The JSON item to extract variables from
    /// * `item_id` - The ID of the item being processed
    ///
    /// # Returns
    /// HashMap of variable names to string values for interpolation
    fn build_item_variables(item: &Value, item_id: &str) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        // Store the item fields as flattened variables for interpolation
        if let Value::Object(map) = item {
            for (key, value) in map {
                let var_key = format!("item.{}", key);
                let var_value = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    _ => value.to_string(),
                };
                variables.insert(var_key, var_value);
            }
        }

        // Also store the entire item as JSON for complex cases
        variables.insert(
            "item_json".to_string(),
            serde_json::to_string(item).unwrap_or_default(),
        );
        variables.insert("item_id".to_string(), item_id.to_string());

        variables
    }

    /// Execute a single agent for a work item
    #[allow(clippy::too_many_arguments)]
    async fn execute_agent_for_item(
        agent_manager: &Arc<dyn AgentLifecycleManager>,
        merge_queue: &Arc<MergeQueue>,
        agent_id: &str,
        item_id: &str,
        item: Value,
        map_phase: &MapPhase,
        env: &ExecutionEnvironment,
        user_interaction: &Arc<dyn UserInteraction>,
        command_executor: &CommandExecutor,
        timeout_enforcer: Option<&Arc<TimeoutEnforcer>>,
        agent_index: usize,
        total_items: usize,
    ) -> MapReduceResult<AgentResult> {
        info!("Starting agent {} for item {}", agent_id, item_id);

        // Create agent config
        let config = AgentConfig {
            id: agent_id.to_string(),
            item_id: item_id.to_string(),
            branch_name: format!("agent-{}-{}", agent_id, item_id),
            max_retries: 3,
            timeout: Duration::from_secs(600),
            agent_index,
            total_items,
        };

        // Convert agent template to WorkflowSteps
        let commands = map_phase.agent_template.clone();

        // Create the agent with its worktree
        let handle = agent_manager
            .create_agent(config.clone(), commands.clone())
            .await
            .map_err(|e| {
                MapReduceError::ProcessingError(format!("Failed to create agent: {}", e))
            })?;

        // Register timeout if enforcer is available
        let _timeout_handle =
            Self::register_agent_timeout(timeout_enforcer, agent_id, item_id, &commands).await;

        // Execute commands with timeout monitoring
        let agent_result = {
            let start_time = Instant::now();

            // Execute all commands
            let (output, all_commits, all_files_modified) = Self::execute_agent_commands(
                &handle,
                &commands,
                &item,
                item_id,
                agent_id,
                env,
                command_executor,
                timeout_enforcer,
                user_interaction,
            )
            .await?;

            // Calculate total duration
            let total_duration = start_time.elapsed();

            // Build the result
            AgentResult {
                item_id: item_id.to_string(),
                status: AgentStatus::Success,
                output: Some(output),
                commits: all_commits.clone(),
                duration: total_duration,
                error: None,
                worktree_path: Some(handle.worktree_path().to_path_buf()),
                branch_name: Some(handle.worktree_session.branch.clone()),
                worktree_session_id: Some(agent_id.to_string()),
                files_modified: all_files_modified.clone(),
                json_log_location: None,
                cleanup_status: None,
            }
        };

        // Unregister timeout (agent completed)
        Self::unregister_agent_timeout(timeout_enforcer, agent_id).await?;

        // Merge and cleanup agent if successful
        let merge_successful = Self::merge_and_cleanup_agent(
            agent_manager,
            merge_queue,
            handle,
            &config,
            &agent_result,
            env,
            agent_id,
            item_id,
        )
        .await?;

        // Update agent status based on merge outcome
        if !merge_successful && !agent_result.commits.is_empty() {
            warn!("Agent {} completed successfully but merge failed", agent_id);
            // Mark agent as failed since merge is part of successful completion
            let mut final_result = agent_result;
            final_result.status = AgentStatus::Failed(
                "Agent execution succeeded but merge to parent worktree failed".to_string(),
            );
            final_result.error =
                Some("Merge to parent worktree failed - changes not integrated".to_string());
            return Ok(final_result);
        }

        Ok(agent_result)
    }

    /// Handle on_failure configuration
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) async fn handle_on_failure(
        on_failure: &OnFailureConfig,
        worktree_path: &Path,
        variables: &HashMap<String, String>,
        command_executor: &CommandExecutor,
        user_interaction: &Arc<dyn UserInteraction>,
    ) -> MapReduceResult<bool> {
        // Extract commands based on OnFailureConfig variant
        let (claude_cmd, shell_cmd) = match on_failure {
            OnFailureConfig::Advanced { claude, shell, .. } => {
                (claude.as_deref(), shell.as_deref())
            }
            OnFailureConfig::SingleCommand(cmd) => {
                if cmd.starts_with("/") {
                    (Some(cmd.as_str()), None)
                } else {
                    (None, Some(cmd.as_str()))
                }
            }
            _ => (None, None),
        };

        // Execute Claude command if present
        if let Some(cmd) = claude_cmd {
            user_interaction
                .display_progress(&format!("on_failure: Executing Claude command: {}", cmd));
            info!("Executing on_failure Claude command: {}", cmd);

            let step = WorkflowStep {
                claude: Some(cmd.to_string()),
                ..Default::default()
            };

            let result = command_executor
                .execute_step_in_worktree(worktree_path, &step, variables, None)
                .await?;

            return Ok(result.success);
        }

        // Execute shell command if present
        if let Some(cmd) = shell_cmd {
            user_interaction
                .display_progress(&format!("on_failure: Executing shell command: {}", cmd));
            info!("Executing on_failure shell command: {}", cmd);

            let step = WorkflowStep {
                shell: Some(cmd.to_string()),
                ..Default::default()
            };

            let result = command_executor
                .execute_step_in_worktree(worktree_path, &step, variables, None)
                .await?;

            return Ok(result.success);
        }

        // No handler to execute
        Ok(true)
    }

    /// Get commits from a worktree
    async fn get_worktree_commits(worktree_path: &Path) -> Vec<String> {
        use crate::cook::execution::mapreduce::resources::git_operations::{
            GitOperationsConfig, GitOperationsService, GitResultExt,
        };

        let mut service = GitOperationsService::new(GitOperationsConfig::default());
        match service
            .get_worktree_commits(worktree_path, None, None)
            .await
        {
            Ok(commits) => commits.to_string_list(),
            Err(e) => {
                warn!("Failed to get worktree commits: {}", e);
                vec![]
            }
        }
    }

    /// Get modified files from a worktree
    async fn get_worktree_modified_files(worktree_path: &Path) -> Vec<String> {
        use crate::cook::execution::mapreduce::resources::git_operations::{
            GitOperationsConfig, GitOperationsService, GitResultExt,
        };

        let mut service = GitOperationsService::new(GitOperationsConfig::default());
        match service
            .get_worktree_modified_files(worktree_path, None)
            .await
        {
            Ok(files) => files.to_string_list(),
            Err(e) => {
                warn!("Failed to get modified files: {}", e);
                vec![]
            }
        }
    }

    /// Build full interpolation context for reduce phase
    ///
    /// Creates an InterpolationContext with all reduce phase variables including:
    /// - Scalar summary values (map.successful, map.failed, map.total)
    /// - Full map.results array (for write_file commands)
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn build_reduce_interpolation_context(
        map_results: &[AgentResult],
        summary: &AggregationSummary,
    ) -> MapReduceResult<crate::cook::execution::interpolation::InterpolationContext> {
        use crate::cook::execution::interpolation::InterpolationContext;

        let mut context = InterpolationContext::new();

        // Add scalar summary values
        context.set("map.successful", serde_json::json!(summary.successful));
        context.set("map.failed", serde_json::json!(summary.failed));
        context.set("map.total", serde_json::json!(summary.total));

        // Add full results as JSON value (for write_file interpolation)
        // This can be >1MB with many agents, so it's excluded from env vars
        // but available for interpolation in write_file commands
        let results_value = serde_json::to_value(map_results).map_err(|e| {
            MapReduceError::ProcessingError(format!("Failed to serialize map results: {}", e))
        })?;
        context.set("map.results", results_value);

        Ok(context)
    }

    /// Execute the reduce phase
    async fn execute_reduce_phase(
        &self,
        reduce: ReducePhase,
        map_results: &[AgentResult],
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        info!("Executing reduce phase");

        self.user_interaction
            .display_progress("Starting reduce phase...");

        let summary = AggregationSummary::from_results(map_results);
        self.display_reduce_summary(&summary);

        // Log reduce phase start
        self.event_logger
            .log_event(MapReduceEvent::reduce_phase_started())
            .await
            .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

        // Create LIMITED variables for environment (prevent E2BIG errors)
        // map.results excluded because it can be >1MB with many agents
        let mut variables = HashMap::new();
        variables.insert("map.successful".to_string(), summary.successful.to_string());
        variables.insert("map.failed".to_string(), summary.failed.to_string());
        variables.insert("map.total".to_string(), summary.total.to_string());

        // Create FULL interpolation context for write_file commands
        // Includes map.results since interpolation doesn't use env vars
        let full_context = Self::build_reduce_interpolation_context(map_results, &summary)?;

        // Execute reduce commands
        for (index, step) in reduce.commands.iter().enumerate() {
            self.user_interaction.display_progress(&format!(
                "Reduce phase: Executing step {}/{}",
                index + 1,
                reduce.commands.len()
            ));

            let step_result = self
                .command_executor
                .execute_step_in_worktree(
                    &env.working_dir,
                    step,
                    &variables,
                    Some(&full_context), // Reduce phase provides full context
                )
                .await?;

            if !step_result.success {
                // Handle on_failure if configured
                if let Some(on_failure) = &step.on_failure {
                    self.user_interaction.display_warning(&format!(
                        "Reduce step {} failed, executing on_failure handler",
                        index + 1
                    ));

                    let handler_result = Self::handle_on_failure(
                        on_failure,
                        &env.working_dir,
                        &variables,
                        &self.command_executor,
                        &self.user_interaction,
                    )
                    .await?;

                    if !handler_result {
                        return Err(MapReduceError::ProcessingError(format!(
                            "Reduce step {} failed and on_failure handler failed",
                            index + 1
                        )));
                    }
                }
            }
        }

        // Log reduce phase completion
        self.event_logger
            .log_event(MapReduceEvent::reduce_phase_completed())
            .await
            .map_err(|e| MapReduceError::ProcessingError(e.to_string()))?;

        self.user_interaction
            .display_success("Reduce phase completed");
        Ok(())
    }

    /// Display map phase summary
    fn display_map_summary(&self, summary: &AggregationSummary) {
        let message = format!(
            "Map phase completed: {} successful, {} failed (total: {})",
            summary.successful, summary.failed, summary.total
        );

        if summary.failed > 0 {
            self.user_interaction.display_warning(&message);
        } else {
            self.user_interaction.display_success(&message);
        }
    }

    /// Display reduce phase summary
    fn display_reduce_summary(&self, summary: &AggregationSummary) {
        self.user_interaction.display_info(&format!(
            "Reduce phase input: {} items ({} successful, {} failed)",
            summary.total, summary.successful, summary.failed
        ));
    }

    /// Get collected results
    pub async fn get_results(&self) -> Vec<AgentResult> {
        self.result_collector.get_results().await
    }

    /// Clear collected results
    pub async fn clear_results(&self) {
        self.result_collector.clear().await;
    }
}

// Dummy session manager
struct DummySessionManager;

#[async_trait::async_trait]
impl SessionManager for DummySessionManager {
    async fn start_session(&self, _session_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn update_session(
        &self,
        _update: crate::cook::session::SessionUpdate,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn complete_session(&self) -> anyhow::Result<crate::cook::session::SessionSummary> {
        Ok(crate::cook::session::SessionSummary {
            iterations: 0,
            files_changed: 0,
        })
    }

    fn get_state(&self) -> anyhow::Result<crate::cook::session::SessionState> {
        Ok(crate::cook::session::SessionState::new(
            "dummy".to_string(),
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        ))
    }

    async fn save_state(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_state(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_session(
        &self,
        _session_id: &str,
    ) -> anyhow::Result<crate::cook::session::SessionState> {
        Ok(crate::cook::session::SessionState::new(
            "dummy".to_string(),
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        ))
    }

    async fn save_checkpoint(
        &self,
        _state: &crate::cook::session::SessionState,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn list_resumable(&self) -> anyhow::Result<Vec<crate::cook::session::SessionInfo>> {
        Ok(vec![])
    }

    async fn get_last_interrupted(&self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_item_variables_with_simple_types() {
        let item = json!({
            "name": "test-item",
            "count": 42,
            "enabled": true,
            "optional": null
        });

        let variables = MapReduceCoordinator::build_item_variables(&item, "item-123");

        assert_eq!(variables.get("item.name"), Some(&"test-item".to_string()));
        assert_eq!(variables.get("item.count"), Some(&"42".to_string()));
        assert_eq!(variables.get("item.enabled"), Some(&"true".to_string()));
        assert_eq!(variables.get("item.optional"), Some(&"null".to_string()));
        assert_eq!(variables.get("item_id"), Some(&"item-123".to_string()));
        assert!(variables.contains_key("item_json"));
    }

    #[test]
    fn test_build_item_variables_with_nested_objects() {
        let item = json!({
            "name": "test",
            "metadata": {
                "key": "value"
            }
        });

        let variables = MapReduceCoordinator::build_item_variables(&item, "item-456");

        assert_eq!(variables.get("item.name"), Some(&"test".to_string()));
        // Nested objects should be serialized to JSON
        assert!(variables.get("item.metadata").unwrap().contains("key"));
        assert_eq!(variables.get("item_id"), Some(&"item-456".to_string()));
    }

    #[test]
    fn test_build_item_variables_with_empty_object() {
        let item = json!({});

        let variables = MapReduceCoordinator::build_item_variables(&item, "item-789");

        // Should still have item_json and item_id
        assert_eq!(variables.get("item_id"), Some(&"item-789".to_string()));
        assert_eq!(variables.get("item_json"), Some(&"{}".to_string()));
        assert_eq!(variables.len(), 2); // Only item_id and item_json
    }

    #[test]
    fn test_build_item_variables_with_non_object() {
        let item = json!("just a string");

        let variables = MapReduceCoordinator::build_item_variables(&item, "item-999");

        // Should still have item_json and item_id
        assert_eq!(variables.get("item_id"), Some(&"item-999".to_string()));
        assert_eq!(
            variables.get("item_json"),
            Some(&"\"just a string\"".to_string())
        );
        assert_eq!(variables.len(), 2); // Only item_id and item_json
    }
}
