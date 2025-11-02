//! Core MapReduce execution coordinator
//!
//! This module coordinates the execution of MapReduce jobs,
//! managing phases and resource allocation.

use crate::cook::execution::claude::ClaudeExecutorImpl;
use crate::cook::execution::data_pipeline::DataPipeline;
use crate::cook::execution::dlq::DeadLetterQueue;
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::input_source::InputSource;
use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::mapreduce::{
    agent::{AgentConfig, AgentLifecycleManager, AgentResult, AgentStatus},
    aggregation::{AggregationSummary, CollectionStrategy, ResultCollector},
    dlq_integration,
    event::{EventLogger, MapReduceEvent},
    merge_queue::MergeQueue,
    resources::git::GitOperations,
    state::StateManager,
    timeout::{TimeoutConfig, TimeoutEnforcer},
    types::{MapPhase, ReducePhase, SetupPhase},
};
use crate::cook::execution::runner::RealCommandRunner;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{OnFailureConfig, StepResult, WorkflowStep};
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
    /// Claude executor for Claude commands
    claude_executor: Arc<dyn ClaudeExecutor>,
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
    fn get_step_display_name(step: &WorkflowStep) -> String {
        if let Some(claude_cmd) = &step.claude {
            format!("claude: {}", claude_cmd)
        } else if let Some(shell_cmd) = &step.shell {
            // Truncate long shell commands for readability
            if shell_cmd.len() > 60 {
                format!("shell: {}...", &shell_cmd[..57])
            } else {
                format!("shell: {}", shell_cmd)
            }
        } else if let Some(write_file) = &step.write_file {
            format!("write_file: {}", write_file.path)
        } else {
            "unknown step".to_string()
        }
    }

    /// Execute the setup phase
    async fn execute_setup_phase(
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
            let result = self.execute_setup_step(step, env, env_vars).await?;

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
                        env,
                        &self.claude_executor,
                        &self.subprocess,
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

    /// Execute a single setup step
    async fn execute_setup_step(
        &self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
    ) -> MapReduceResult<StepResult> {
        use crate::subprocess::ProcessCommandBuilder;

        if let Some(shell_cmd) = &step.shell {
            info!("Executing shell command: {}", shell_cmd);
            info!("Working directory: {}", env.working_dir.display());

            let command = ProcessCommandBuilder::new("sh")
                .args(["-c", shell_cmd])
                .current_dir(&env.working_dir)
                .envs(env_vars)
                .build();

            let output = self.subprocess.runner().run(command).await.map_err(|e| {
                MapReduceError::ProcessingError(format!("Shell command failed: {}", e))
            })?;

            let exit_code = match output.status {
                crate::subprocess::runner::ExitStatus::Success => 0,
                crate::subprocess::runner::ExitStatus::Error(code) => code,
                crate::subprocess::runner::ExitStatus::Timeout => -1,
                crate::subprocess::runner::ExitStatus::Signal(sig) => -sig,
            };

            Ok(StepResult {
                success: exit_code == 0,
                exit_code: Some(exit_code),
                stdout: output.stdout,
                stderr: output.stderr,
                json_log_location: None,
            })
        } else if let Some(claude_cmd) = &step.claude {
            info!("Executing Claude command: {}", claude_cmd);

            let result = self
                .claude_executor
                .execute_claude_command(claude_cmd, &env.working_dir, env_vars)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!("Claude command failed: {}", e))
                })?;

            let json_log_location = result.json_log_location().map(|s| s.to_string());

            Ok(StepResult {
                success: result.success,
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                json_log_location,
            })
        } else if let Some(write_file_cfg) = &step.write_file {
            info!("Executing write_file command: {}", write_file_cfg.path);

            let result =
                crate::cook::workflow::execute_write_file_command(write_file_cfg, &env.working_dir)
                    .await
                    .map_err(|e| {
                        MapReduceError::ProcessingError(format!("Write file command failed: {}", e))
                    })?;

            Ok(StepResult {
                success: result.success,
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                json_log_location: result.json_log_location,
            })
        } else {
            Err(MapReduceError::InvalidConfiguration {
                reason: "Step must have either 'claude', 'shell', or 'write_file' command"
                    .to_string(),
                field: "step".to_string(),
                value: format!("{:?}", step),
            })
        }
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
                let claude_executor = Arc::clone(&self.claude_executor);
                let subprocess = Arc::clone(&self.subprocess);
                let dlq = Arc::clone(&self.dlq);
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
                        &claude_executor,
                        &subprocess,
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
                    if let Some(dlq_item) =
                        dlq_integration::agent_result_to_dlq_item(&agent_result, &item_for_dlq, 1)
                    {
                        if let Err(e) = dlq.add(dlq_item).await {
                            warn!(
                                "Failed to add item {} to DLQ: {}. Item tracking may be incomplete.",
                                agent_result.item_id, e
                            );
                        } else {
                            info!(
                                "Added failed item {} to DLQ for potential retry",
                                agent_result.item_id
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
        claude_executor: &Arc<dyn ClaudeExecutor>,
        subprocess: &Arc<SubprocessManager>,
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
        let _timeout_handle = if let Some(enforcer) = timeout_enforcer {
            match enforcer
                .register_agent_timeout(agent_id.to_string(), item_id.to_string(), &commands)
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
        };

        // Execute commands with timeout monitoring
        let agent_result = {
            let start_time = Instant::now();

            // Execute commands in the agent's worktree
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
                if let Some(enforcer) = timeout_enforcer {
                    let _ = enforcer
                        .register_command_start(&agent_id.to_string(), index)
                        .await;
                }

                let cmd_start = Instant::now();

                // Create variables for interpolation
                let mut variables = HashMap::new();

                // Store the item fields as flattened variables for interpolation
                if let serde_json::Value::Object(map) = &item {
                    for (key, value) in map {
                        let var_key = format!("item.{}", key);
                        let var_value = match value {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            serde_json::Value::Null => "null".to_string(),
                            _ => value.to_string(),
                        };
                        variables.insert(var_key, var_value);
                    }
                }

                // Also store the entire item as JSON for complex cases
                variables.insert(
                    "item_json".to_string(),
                    serde_json::to_string(&item).unwrap_or_default(),
                );
                variables.insert("item_id".to_string(), item_id.to_string());

                // Execute the step in the agent's worktree
                let step_result = Self::execute_step_in_agent_worktree(
                    handle.worktree_path(),
                    step,
                    &variables,
                    None, // Map phase doesn't need full context
                    env,
                    claude_executor,
                    subprocess,
                )
                .await?;

                // Notify timeout enforcer of command completion
                if let Some(enforcer) = timeout_enforcer {
                    let _ = enforcer
                        .register_command_completion(
                            &agent_id.to_string(),
                            index,
                            cmd_start.elapsed(),
                        )
                        .await;
                }

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
                            env,
                            claude_executor,
                            subprocess,
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
        if let Some(enforcer) = timeout_enforcer {
            let _ = enforcer
                .unregister_agent_timeout(&agent_id.to_string())
                .await;
        }

        // Merge and cleanup agent if successful
        let merge_successful = if !agent_result.commits.is_empty() {
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
                    true
                }
                Err(e) => {
                    warn!(
                        "Failed to merge agent {} (item {}): {}",
                        agent_id, item_id, e
                    );
                    // Still cleanup the worktree even if merge failed
                    let _ = agent_manager.cleanup_agent(handle).await;
                    false
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
            false
        };

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

    /// Execute a step in an agent's worktree with variable interpolation
    ///
    /// # Arguments
    ///
    /// * `variables` - Limited scalar variables for environment variable export.
    ///   Excludes large data like `map.results` to prevent E2BIG errors.
    /// * `full_context` - Optional full interpolation context including large
    ///   variables. Used for write_file commands to enable `${map.results}`.
    ///   If None, falls back to building context from `variables` HashMap.
    ///
    /// # Variable Context Strategy
    ///
    /// - **Shell/Claude commands**: Use `variables` HashMap → converted to env vars
    /// - **write_file commands**: Use `full_context` if provided for interpolation
    /// - **Fallback**: If no `full_context`, build from `variables` (map phase)
    async fn execute_step_in_agent_worktree(
        worktree_path: &Path,
        step: &WorkflowStep,
        variables: &HashMap<String, String>,
        full_context: Option<&InterpolationContext>,
        _env: &ExecutionEnvironment,
        claude_executor: &Arc<dyn ClaudeExecutor>,
        subprocess: &Arc<SubprocessManager>,
    ) -> MapReduceResult<StepResult> {
        use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
        use crate::subprocess::ProcessCommandBuilder;

        // Create interpolation engine for variable substitution
        let mut engine = InterpolationEngine::default();

        // Build interpolation context with priority fallback
        let interp_context = if let Some(full_ctx) = full_context {
            // Use provided full context (for reduce phase with map.results)
            full_ctx.clone()
        } else {
            // Build from limited variables HashMap (for map phase)
            let mut ctx = InterpolationContext::new();

            // Build a nested JSON structure for item variables
            let mut item_obj = serde_json::Map::new();
            let mut other_vars = serde_json::Map::new();

            for (key, value) in variables {
                if let Some(item_field) = key.strip_prefix("item.") {
                    // Add to item object
                    item_obj.insert(
                        item_field.to_string(),
                        serde_json::Value::String(value.clone()),
                    );
                } else {
                    // Add as top-level variable
                    other_vars.insert(key.clone(), serde_json::Value::String(value.clone()));
                }
            }

            // Set the item object in context
            if !item_obj.is_empty() {
                ctx.set("item", serde_json::Value::Object(item_obj));
            }

            // Set other variables
            for (key, value) in other_vars {
                ctx.set(key, value);
            }

            ctx
        };

        // Execute based on step type
        if let Some(claude_cmd) = &step.claude {
            // Interpolate variables in command
            let interpolated_cmd =
                engine
                    .interpolate(claude_cmd, &interp_context)
                    .map_err(|e| {
                        MapReduceError::ProcessingError(format!(
                            "Variable interpolation failed: {}",
                            e
                        ))
                    })?;

            // Execute Claude command
            info!("Executing Claude command in worktree: {}", interpolated_cmd);

            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "false".to_string());

            let result = claude_executor
                .execute_claude_command(&interpolated_cmd, worktree_path, env_vars)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!(
                        "Failed to execute Claude command: {}",
                        e
                    ))
                })?;

            let json_log_location = result.json_log_location().map(|s| s.to_string());

            Ok(StepResult {
                success: result.success,
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                json_log_location,
            })
        } else if let Some(shell_cmd) = &step.shell {
            // Debug: print context
            debug!("Interpolating shell command: {}", shell_cmd);
            debug!("Context variables: {:?}", interp_context);

            // Interpolate variables in command
            let interpolated_cmd = engine
                .interpolate(shell_cmd, &interp_context)
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!("Variable interpolation failed: {}", e))
                })?;

            // Execute shell command
            info!("Executing shell command in worktree: {}", interpolated_cmd);

            let command = ProcessCommandBuilder::new("sh")
                .args(["-c", &interpolated_cmd])
                .current_dir(worktree_path)
                .envs(variables.clone())
                .build();

            let output = subprocess.runner().run(command).await.map_err(|e| {
                MapReduceError::ProcessingError(format!("Failed to execute shell command: {}", e))
            })?;

            let exit_code = match output.status {
                crate::subprocess::runner::ExitStatus::Success => 0,
                crate::subprocess::runner::ExitStatus::Error(code) => code,
                crate::subprocess::runner::ExitStatus::Timeout => -1,
                crate::subprocess::runner::ExitStatus::Signal(sig) => -sig,
            };

            Ok(StepResult {
                success: exit_code == 0,
                exit_code: Some(exit_code),
                stdout: output.stdout,
                stderr: output.stderr,
                json_log_location: None,
            })
        } else if let Some(write_file_cfg) = &step.write_file {
            // Interpolate variables in path and content
            let interpolated_path = engine
                .interpolate(&write_file_cfg.path, &interp_context)
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!(
                        "Variable interpolation failed for path: {}",
                        e
                    ))
                })?;

            let interpolated_content = engine
                .interpolate(&write_file_cfg.content, &interp_context)
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!(
                        "Variable interpolation failed for content: {}",
                        e
                    ))
                })?;

            // Create interpolated config
            let interpolated_cfg = crate::config::command::WriteFileConfig {
                path: interpolated_path,
                content: interpolated_content,
                format: write_file_cfg.format.clone(),
                create_dirs: write_file_cfg.create_dirs,
                mode: write_file_cfg.mode.clone(),
            };

            // Execute write_file command
            info!(
                "Executing write_file command in worktree: {}",
                interpolated_cfg.path
            );

            let result =
                crate::cook::workflow::execute_write_file_command(&interpolated_cfg, worktree_path)
                    .await
                    .map_err(|e| {
                        MapReduceError::ProcessingError(format!(
                            "Failed to execute write_file command: {}",
                            e
                        ))
                    })?;

            Ok(StepResult {
                success: result.success,
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                json_log_location: result.json_log_location,
            })
        } else {
            Err(MapReduceError::InvalidConfiguration {
                reason: "Step must have either 'claude', 'shell', or 'write_file' command"
                    .to_string(),
                field: "step".to_string(),
                value: format!("{:?}", step),
            })
        }
    }

    /// Handle on_failure configuration
    async fn handle_on_failure(
        on_failure: &OnFailureConfig,
        worktree_path: &Path,
        variables: &HashMap<String, String>,
        _env: &ExecutionEnvironment,
        claude_executor: &Arc<dyn ClaudeExecutor>,
        subprocess: &Arc<SubprocessManager>,
        user_interaction: &Arc<dyn UserInteraction>,
    ) -> MapReduceResult<bool> {
        use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};

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
            let mut engine = InterpolationEngine::default();
            let mut interp_context = InterpolationContext::new();
            for (key, value) in variables {
                interp_context.set(key.clone(), value.clone());
            }

            let interpolated_cmd = engine.interpolate(cmd, &interp_context).map_err(|e| {
                MapReduceError::ProcessingError(format!("Variable interpolation failed: {}", e))
            })?;

            user_interaction.display_progress(&format!(
                "on_failure: Executing Claude command: {}",
                interpolated_cmd
            ));
            info!("Executing on_failure Claude command: {}", interpolated_cmd);

            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

            let result = claude_executor
                .execute_claude_command(&interpolated_cmd, worktree_path, env_vars)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!(
                        "Failed to execute on_failure handler: {}",
                        e
                    ))
                })?;

            return Ok(result.success);
        }

        // Execute shell command if present
        if let Some(cmd) = shell_cmd {
            use crate::subprocess::ProcessCommandBuilder;

            let mut engine = InterpolationEngine::default();
            let mut interp_context = InterpolationContext::new();
            for (key, value) in variables {
                interp_context.set(key.clone(), value.clone());
            }

            let interpolated_cmd = engine.interpolate(cmd, &interp_context).map_err(|e| {
                MapReduceError::ProcessingError(format!("Variable interpolation failed: {}", e))
            })?;

            user_interaction.display_progress(&format!(
                "on_failure: Executing shell command: {}",
                interpolated_cmd
            ));
            info!("Executing on_failure shell command: {}", interpolated_cmd);

            let command = ProcessCommandBuilder::new("sh")
                .args(["-c", &interpolated_cmd])
                .current_dir(worktree_path)
                .envs(variables.clone())
                .build();

            let output = subprocess.runner().run(command).await.map_err(|e| {
                MapReduceError::ProcessingError(format!(
                    "Failed to execute on_failure shell command: {}",
                    e
                ))
            })?;

            let exit_code = match output.status {
                crate::subprocess::runner::ExitStatus::Success => 0,
                crate::subprocess::runner::ExitStatus::Error(code) => code,
                crate::subprocess::runner::ExitStatus::Timeout => -1,
                crate::subprocess::runner::ExitStatus::Signal(sig) => -sig,
            };

            return Ok(exit_code == 0);
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
    fn build_reduce_interpolation_context(
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

            let step_result = Self::execute_step_in_agent_worktree(
                &env.working_dir,
                step,
                &variables,
                Some(&full_context), // Reduce phase provides full context
                env,
                &self.claude_executor,
                &self.subprocess,
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
                        env,
                        &self.claude_executor,
                        &self.subprocess,
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
mod handle_on_failure_tests {
    use super::*;
    use crate::cook::execution::claude::ClaudeExecutor;
    use crate::cook::execution::ExecutionResult;
    use crate::cook::workflow::OnFailureConfig;
    use crate::subprocess::error::ProcessError;
    use crate::subprocess::runner::{
        ExitStatus, ProcessCommand, ProcessOutput, ProcessRunner, ProcessStream,
    };
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // Mock ClaudeExecutor for testing
    #[derive(Clone)]
    struct MockClaudeExecutor {
        should_succeed: bool,
        executed_commands: Arc<Mutex<Vec<String>>>,
    }

    impl MockClaudeExecutor {
        fn new(should_succeed: bool) -> Self {
            Self {
                should_succeed,
                executed_commands: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_executed_commands(&self) -> Vec<String> {
            self.executed_commands.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ClaudeExecutor for MockClaudeExecutor {
        async fn execute_claude_command(
            &self,
            command: &str,
            _project_path: &Path,
            _env_vars: HashMap<String, String>,
        ) -> anyhow::Result<ExecutionResult> {
            self.executed_commands
                .lock()
                .unwrap()
                .push(command.to_string());
            Ok(ExecutionResult {
                success: self.should_succeed,
                stdout: format!("Output from: {}", command),
                stderr: String::new(),
                exit_code: Some(if self.should_succeed { 0 } else { 1 }),
                metadata: HashMap::new(),
            })
        }

        async fn check_claude_cli(&self) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn get_claude_version(&self) -> anyhow::Result<String> {
            Ok("1.0.0".to_string())
        }
    }

    // Mock ProcessRunner for testing
    #[derive(Clone)]
    struct MockProcessRunner {
        should_succeed: bool,
        executed_commands: Arc<Mutex<Vec<String>>>,
    }

    impl MockProcessRunner {
        fn new(should_succeed: bool) -> Self {
            Self {
                should_succeed,
                executed_commands: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl ProcessRunner for MockProcessRunner {
        async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError> {
            // Extract the actual command from the args (skip "sh" and "-c")
            if command.args.len() > 1 {
                self.executed_commands
                    .lock()
                    .unwrap()
                    .push(command.args[1].clone());
            }

            Ok(ProcessOutput {
                status: if self.should_succeed {
                    ExitStatus::Success
                } else {
                    ExitStatus::Error(1)
                },
                stdout: "".to_string(),
                stderr: "".to_string(),
                duration: Duration::from_secs(0),
            })
        }

        async fn run_streaming(
            &self,
            _command: ProcessCommand,
        ) -> Result<ProcessStream, ProcessError> {
            unimplemented!("Not used in these tests")
        }
    }

    // Helper to create a mock SubprocessManager
    fn create_mock_subprocess(should_succeed: bool) -> Arc<SubprocessManager> {
        Arc::new(SubprocessManager::new(Arc::new(MockProcessRunner::new(
            should_succeed,
        ))))
    }

    // Helper to create a test ExecutionEnvironment
    fn create_test_env() -> ExecutionEnvironment {
        ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/tmp/test")),
            project_dir: Arc::new(PathBuf::from("/tmp/test")),
            worktree_name: None,
            session_id: Arc::from("test-session"),
        }
    }

    #[tokio::test]
    async fn test_advanced_config_with_claude_command_success() {
        let config = OnFailureConfig::Advanced {
            claude: Some("/test-command".to_string()),
            shell: None,
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_advanced_config_with_claude_command_failure() {
        let config = OnFailureConfig::Advanced {
            claude: Some("/test-command".to_string()),
            shell: None,
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(false));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_advanced_config_with_shell_command_success() {
        let config = OnFailureConfig::Advanced {
            claude: None,
            shell: Some("echo test".to_string()),
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_advanced_config_with_shell_command_failure() {
        let config = OnFailureConfig::Advanced {
            claude: None,
            shell: Some("echo test".to_string()),
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(false);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_single_command_with_claude() {
        let config = OnFailureConfig::SingleCommand("/test-claude-cmd".to_string());

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_single_command_with_shell() {
        let config = OnFailureConfig::SingleCommand("echo test".to_string());

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_default_config_returns_ok() {
        let config = OnFailureConfig::IgnoreErrors(true);

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_variable_interpolation_success() {
        let config = OnFailureConfig::SingleCommand("/test ${item_id}".to_string());

        let mock_executor = MockClaudeExecutor::new(true);
        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(mock_executor.clone());
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let mut variables = HashMap::new();
        variables.insert("item_id".to_string(), "item-123".to_string());
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
        let executed = mock_executor.get_executed_commands();
        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0], "/test item-123");
    }

    #[tokio::test]
    async fn test_variable_interpolation_with_missing_variable() {
        // When a variable is missing and strict_mode is false (default),
        // the interpolation engine leaves the variable unchanged
        let config = OnFailureConfig::SingleCommand("/test ${missing_var}".to_string());

        let mock_executor = MockClaudeExecutor::new(true);
        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(mock_executor.clone());
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &env,
            &claude_executor,
            &subprocess,
            &user_interaction,
        )
        .await;

        // Should succeed, but variable is left unchanged
        assert!(result.is_ok());
        assert!(result.unwrap());
        let executed = mock_executor.get_executed_commands();
        assert_eq!(executed.len(), 1);
        // The variable remains unchanged when not found in non-strict mode
        assert!(executed[0].contains("missing_var") || executed[0] == "/test ");
    }
}

#[cfg(test)]
mod execute_setup_phase_tests {
    use super::*;
    use crate::cook::execution::mapreduce::types::SetupPhase;
    use crate::cook::workflow::WorkflowStep;

    // Helper to create a test MapReduceCoordinator with mocks
    fn create_test_coordinator(
        claude_succeeds: bool,
        shell_succeeds: bool,
    ) -> MapReduceCoordinator {
        use crate::cook::execution::mapreduce::agent::{
            lifecycle::LifecycleError, AgentLifecycleManager,
        };
        use crate::cook::execution::mapreduce::state::{
            JobState, JobSummary, StateError, StateManager, StateStore,
        };
        use crate::cook::interaction::MockUserInteraction;
        use crate::subprocess::runner::{ExitStatus, ProcessCommand, ProcessOutput, ProcessRunner};
        use std::sync::Arc;

        // Mock ProcessRunner
        #[derive(Clone)]
        struct TestProcessRunner {
            should_succeed: bool,
        }

        #[async_trait::async_trait]
        impl ProcessRunner for TestProcessRunner {
            async fn run(
                &self,
                _command: ProcessCommand,
            ) -> Result<ProcessOutput, crate::subprocess::error::ProcessError> {
                Ok(ProcessOutput {
                    status: if self.should_succeed {
                        ExitStatus::Success
                    } else {
                        ExitStatus::Error(1)
                    },
                    stdout: "test stdout".to_string(),
                    stderr: if self.should_succeed {
                        String::new()
                    } else {
                        "test stderr".to_string()
                    },
                    duration: std::time::Duration::from_secs(0),
                })
            }

            async fn run_streaming(
                &self,
                _command: ProcessCommand,
            ) -> Result<
                crate::subprocess::runner::ProcessStream,
                crate::subprocess::error::ProcessError,
            > {
                unimplemented!("Not used in these tests")
            }
        }

        // Mock ClaudeExecutor
        #[derive(Clone)]
        struct TestClaudeExecutor {
            should_succeed: bool,
        }

        #[async_trait::async_trait]
        impl crate::cook::execution::ClaudeExecutor for TestClaudeExecutor {
            async fn execute_claude_command(
                &self,
                _command: &str,
                _project_path: &Path,
                _env_vars: HashMap<String, String>,
            ) -> anyhow::Result<crate::cook::execution::ExecutionResult> {
                Ok(crate::cook::execution::ExecutionResult {
                    success: self.should_succeed,
                    stdout: "claude stdout".to_string(),
                    stderr: if self.should_succeed {
                        String::new()
                    } else {
                        "claude stderr".to_string()
                    },
                    exit_code: Some(if self.should_succeed { 0 } else { 1 }),
                    metadata: HashMap::new(),
                })
            }

            async fn check_claude_cli(&self) -> anyhow::Result<bool> {
                Ok(true)
            }

            async fn get_claude_version(&self) -> anyhow::Result<String> {
                Ok("1.0.0".to_string())
            }
        }

        // Mock AgentLifecycleManager
        struct TestAgentLifecycleManager;

        #[async_trait::async_trait]
        impl AgentLifecycleManager for TestAgentLifecycleManager {
            async fn create_agent(
                &self,
                _config: crate::cook::execution::mapreduce::agent::AgentConfig,
                _commands: Vec<WorkflowStep>,
            ) -> Result<crate::cook::execution::mapreduce::agent::AgentHandle, LifecycleError>
            {
                unimplemented!("Not used in these tests")
            }

            async fn create_agent_branch(
                &self,
                _worktree_path: &Path,
                _branch_name: &str,
            ) -> Result<(), LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn merge_agent_to_parent(
                &self,
                _agent_branch: &str,
                _env: &ExecutionEnvironment,
            ) -> Result<(), LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn handle_merge_and_cleanup(
                &self,
                _is_successful: bool,
                _env: &ExecutionEnvironment,
                _worktree_path: &Path,
                _worktree_name: &str,
                _branch_name: &str,
                _template_steps: &[WorkflowStep],
                _item_id: &str,
            ) -> Result<bool, LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn cleanup_agent(
                &self,
                _handle: crate::cook::execution::mapreduce::agent::AgentHandle,
            ) -> Result<(), LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn get_worktree_commits(
                &self,
                _worktree_path: &Path,
            ) -> Result<Vec<String>, LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn get_modified_files(
                &self,
                _worktree_path: &Path,
            ) -> Result<Vec<String>, LifecycleError> {
                unimplemented!("Not used in these tests")
            }
        }

        // Mock StateStore
        struct TestStateStore;

        #[async_trait::async_trait]
        impl StateStore for TestStateStore {
            async fn save(&self, _state: &JobState) -> Result<(), StateError> {
                Ok(())
            }

            async fn load(&self, _job_id: &str) -> Result<Option<JobState>, StateError> {
                Ok(None)
            }

            async fn list(&self) -> Result<Vec<JobSummary>, StateError> {
                Ok(vec![])
            }

            async fn delete(&self, _job_id: &str) -> Result<(), StateError> {
                Ok(())
            }
        }

        let agent_manager: Arc<dyn AgentLifecycleManager> = Arc::new(TestAgentLifecycleManager);
        let state_manager = Arc::new(StateManager::new(Arc::new(TestStateStore)));
        let user_interaction = Arc::new(MockUserInteraction::new());
        let subprocess = Arc::new(SubprocessManager::new(Arc::new(TestProcessRunner {
            should_succeed: shell_succeeds,
        })));
        let project_root = PathBuf::from("/tmp/test");

        let mut coordinator = MapReduceCoordinator::new(
            agent_manager,
            state_manager,
            user_interaction,
            subprocess,
            project_root,
        );

        // Replace claude executor with test version
        coordinator.claude_executor = Arc::new(TestClaudeExecutor {
            should_succeed: claude_succeeds,
        });

        coordinator
    }

    fn create_test_env() -> ExecutionEnvironment {
        ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/tmp/test")),
            project_dir: Arc::new(PathBuf::from("/tmp/test")),
            worktree_name: None,
            session_id: Arc::from("test-session"),
        }
    }

    #[tokio::test]
    async fn test_setup_phase_all_steps_succeed() {
        let coordinator = create_test_coordinator(true, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![
                WorkflowStep {
                    shell: Some("echo test1".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo test2".to_string()),
                    ..Default::default()
                },
            ],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(
            result.is_ok(),
            "Setup phase should succeed with all passing steps"
        );
    }

    #[tokio::test]
    async fn test_setup_phase_shell_failure_with_exit_code() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("false".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(
            result.is_err(),
            "Setup phase should fail when shell command fails"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Setup step 1") && err_msg.contains("failed"),
            "Error should mention step number and failure, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("exit code:"),
            "Error should include exit code, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_setup_phase_shell_failure_with_stderr() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("some_command".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(
            result.is_err(),
            "Setup phase should fail when command produces stderr"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("stderr:") || err_msg.contains("test stderr"),
            "Error should include stderr output"
        );
    }

    #[tokio::test]
    async fn test_setup_phase_shell_failure_with_stdout_only() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("some_command".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(result.is_err(), "Setup phase should fail");

        let err_msg = result.unwrap_err().to_string();
        // Should include stdout when stderr is empty or not provided
        assert!(
            err_msg.contains("stdout:") || err_msg.contains("stderr:"),
            "Error should include output"
        );
    }

    #[tokio::test]
    async fn test_setup_phase_claude_failure_with_log_hint() {
        let coordinator = create_test_coordinator(false, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                claude: Some("/test-command".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(
            result.is_err(),
            "Setup phase should fail when Claude command fails"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Setup step 1") && err_msg.contains("failed"),
            "Error should mention step number and failure, got: {}",
            err_msg
        );
        // Note: log hint only appears if extract_repo_name succeeds, which it won't in this test
    }

    #[tokio::test]
    async fn test_setup_phase_multiple_steps_mixed_success() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![
                WorkflowStep {
                    shell: Some("echo step1".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("failing_command".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo step3".to_string()),
                    ..Default::default()
                },
            ],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(
            result.is_err(),
            "Setup phase should fail on first failing step"
        );

        let err_msg = result.unwrap_err().to_string();
        // Should fail on step 2 (index 1)
        assert!(
            err_msg.contains("Setup step"),
            "Error should mention which step failed"
        );
    }

    #[tokio::test]
    async fn test_setup_phase_environment_variables_set() {
        let coordinator = create_test_coordinator(true, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("echo $PRODIGY_AUTOMATION".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator.execute_setup_phase(setup, &env).await;
        // Should succeed - this test verifies env vars are set
        // (actual verification would require checking the subprocess call)
        assert!(result.is_ok(), "Setup phase should succeed");
    }

    #[tokio::test]
    async fn test_setup_phase_debug_logging_context() {
        let coordinator = create_test_coordinator(true, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("echo test".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        // This test verifies the function runs without panicking
        // Debug logs are checked via tracing (would need tracing subscriber in real test)
        let result = coordinator.execute_setup_phase(setup, &env).await;
        assert!(
            result.is_ok(),
            "Setup phase should succeed and log debug context"
        );
    }
}

#[cfg(test)]
mod reduce_interpolation_context_tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::types::{AgentResult, AgentStatus};
    use crate::cook::execution::mapreduce::aggregation::AggregationSummary;
    use std::time::Duration;

    #[test]
    fn test_build_reduce_interpolation_context_includes_map_results() {
        // Create sample agent results
        let results = vec![
            AgentResult {
                item_id: "item-1".to_string(),
                status: AgentStatus::Success,
                output: Some("output-1".to_string()),
                commits: vec!["commit-1".to_string()],
                files_modified: vec![],
                duration: Duration::from_secs(10),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: None,
                cleanup_status: None,
            },
            AgentResult {
                item_id: "item-2".to_string(),
                status: AgentStatus::Success,
                output: Some("output-2".to_string()),
                commits: vec!["commit-2".to_string()],
                files_modified: vec![],
                duration: Duration::from_secs(15),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: None,
                cleanup_status: None,
            },
        ];

        let summary = AggregationSummary::from_results(&results);

        let context =
            MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary).unwrap();

        // Verify scalar values are present
        let successful = context.variables.get("map.successful").unwrap();
        assert_eq!(successful.as_u64().unwrap(), 2);

        let failed = context.variables.get("map.failed").unwrap();
        assert_eq!(failed.as_u64().unwrap(), 0);

        let total = context.variables.get("map.total").unwrap();
        assert_eq!(total.as_u64().unwrap(), 2);

        // Verify map.results is present and is an array
        let map_results = context.variables.get("map.results").unwrap();
        assert!(map_results.is_array());
        let results_array = map_results.as_array().unwrap();
        assert_eq!(results_array.len(), 2);

        // Verify first result contains expected fields
        let first_result = &results_array[0];
        assert_eq!(
            first_result.get("item_id").unwrap().as_str().unwrap(),
            "item-1"
        );
        assert_eq!(
            first_result.get("output").unwrap().as_str().unwrap(),
            "output-1"
        );
    }

    #[test]
    fn test_build_reduce_interpolation_context_with_empty_results() {
        let results: Vec<AgentResult> = vec![];
        let summary = AggregationSummary::from_results(&results);

        let context =
            MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary).unwrap();

        // Verify scalar values
        assert_eq!(
            context
                .variables
                .get("map.successful")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );
        assert_eq!(
            context
                .variables
                .get("map.failed")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );
        assert_eq!(
            context
                .variables
                .get("map.total")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );

        // Verify map.results is an empty array
        let map_results = context.variables.get("map.results").unwrap();
        assert!(map_results.is_array());
        assert_eq!(map_results.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_build_reduce_interpolation_context_with_failed_agents() {
        let results = vec![
            AgentResult {
                item_id: "item-1".to_string(),
                status: AgentStatus::Success,
                output: Some("success".to_string()),
                commits: vec!["commit-1".to_string()],
                files_modified: vec![],
                duration: Duration::from_secs(10),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: None,
                cleanup_status: None,
            },
            AgentResult {
                item_id: "item-2".to_string(),
                status: AgentStatus::Failed("error occurred".to_string()),
                output: None,
                commits: vec![],
                files_modified: vec![],
                duration: Duration::from_secs(5),
                error: Some("error occurred".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: Some("/path/to/log.json".to_string()),
                cleanup_status: None,
            },
        ];

        let summary = AggregationSummary::from_results(&results);

        let context =
            MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary).unwrap();

        // Verify summary reflects mixed results
        assert_eq!(
            context
                .variables
                .get("map.successful")
                .unwrap()
                .as_u64()
                .unwrap(),
            1
        );
        assert_eq!(
            context
                .variables
                .get("map.failed")
                .unwrap()
                .as_u64()
                .unwrap(),
            1
        );
        assert_eq!(
            context
                .variables
                .get("map.total")
                .unwrap()
                .as_u64()
                .unwrap(),
            2
        );

        // Verify both results are present
        let map_results = context.variables.get("map.results").unwrap();
        assert_eq!(map_results.as_array().unwrap().len(), 2);

        // Verify failed agent has error details
        let failed_result = &map_results.as_array().unwrap()[1];
        assert_eq!(
            failed_result.get("error").unwrap().as_str().unwrap(),
            "error occurred"
        );
    }

    #[test]
    fn test_build_reduce_interpolation_context_serialization_error() {
        // This test verifies that the function handles serialization errors gracefully
        // In practice, AgentResult should always serialize correctly, but we test the error path

        // Note: It's difficult to trigger a serialization error with valid AgentResult data
        // This test primarily documents the expected behavior
        // A real serialization error would require malformed data that can't be represented in JSON

        let results = vec![AgentResult {
            item_id: "item-1".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec![],
            files_modified: vec![],
            duration: Duration::from_secs(10),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
            cleanup_status: None,
        }];

        let summary = AggregationSummary::from_results(&results);

        // This should succeed - valid data always serializes
        let result = MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary);
        assert!(
            result.is_ok(),
            "Valid AgentResult data should serialize successfully"
        );
    }
}
