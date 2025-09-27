//! Core MapReduce execution coordinator
//!
//! This module coordinates the execution of MapReduce jobs,
//! managing phases and resource allocation.

use crate::cook::execution::data_pipeline::DataPipeline;
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::input_source::InputSource;
use crate::cook::execution::mapreduce::{
    agent::{AgentConfig, AgentLifecycleManager, AgentResult, AgentStatus},
    aggregation::{AggregationSummary, CollectionStrategy, ResultCollector},
    event::{EventLogger, MapReduceEvent},
    state::StateManager,
    types::{MapPhase, ReducePhase, SetupPhase},
};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{OnFailureConfig, WorkflowStep};
use crate::subprocess::runner::ExitStatus;
use crate::subprocess::SubprocessManager;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

/// Result of executing a single step
#[derive(Debug, Clone)]
struct StepResult {
    pub success: bool,
    pub _exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub _stderr: Option<String>,
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
    ) -> Self {
        let result_collector = Arc::new(ResultCollector::new(CollectionStrategy::InMemory));
        let job_id = format!("mapreduce-{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        let event_logger = Arc::new(EventLogger::new(project_root.clone(), job_id.clone(), None));

        // Create claude executor
        // In production, this would be injected
        let claude_executor = Arc::new(SimpleClaudeExecutor {
            subprocess: subprocess.clone(),
        });

        // Create session manager - not used but required for struct
        let session_manager = Arc::new(DummySessionManager);

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
        }
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

        // Check if we're in dry-run mode
        if let crate::cook::execution::mapreduce::dry_run::ExecutionMode::DryRun(ref config) = self.execution_mode {
            return self.execute_dry_run(setup.as_ref(), &map_phase, reduce.as_ref(), config).await;
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
        match validator.validate_workflow_phases(
            setup.map(|s| s.clone()),
            map_phase.clone(),
            reduce.map(|r| r.clone()),
        ).await {
            Ok(report) => {
                // Display the validation report
                let formatter = OutputFormatter::new();
                let output = formatter.format_human(&report);

                // Use user interaction to display the output
                self.user_interaction.display_info(&output);

                if report.errors.is_empty() {
                    self.user_interaction.display_success("Dry-run validation successful! Workflow is ready to execute.");
                    Ok(Vec::new()) // Return empty results for dry-run
                } else {
                    self.user_interaction.display_error(&format!("Dry-run validation failed with {} error(s)", report.errors.len()));
                    Err(MapReduceError::General {
                        message: format!("Dry-run validation failed with {} errors", report.errors.len()),
                        source: None,
                    })
                }
            },
            Err(e) => {
                self.user_interaction.display_error(&format!("Dry-run validation failed: {}", e));
                Err(MapReduceError::General {
                    message: format!("Dry-run validation failed: {}", e),
                    source: None,
                })
            }
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
            debug!(
                "Executing setup step {}/{}",
                index + 1,
                setup_phase.commands.len()
            );

            // Log execution context
            info!("=== Step Execution Context ===");
            info!("Step: {:?}", step);
            info!("Working Directory: {}", env.working_dir.display());
            info!("Project Directory: {}", self.project_root.display());
            info!("Worktree: {:?}", env.worktree_name);
            info!("Session ID: {}", env.session_id);

            // Set environment variables
            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
            env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

            info!("Environment Variables:");
            for (key, value) in &env_vars {
                info!("  {} = {}", key, value);
            }
            info!("Actual execution directory: {}", env.working_dir.display());
            info!("==============================");

            // Execute the step
            let result = self.execute_setup_step(step, env, env_vars).await?;

            if !result.success {
                return Err(MapReduceError::ProcessingError(format!(
                    "Setup step {} failed",
                    index + 1
                )));
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
            info!("  Working directory: {}", env.working_dir.display());

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
                _exit_code: Some(exit_code),
                stdout: Some(output.stdout),
                _stderr: Some(output.stderr),
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

            Ok(StepResult {
                success: result.success,
                _exit_code: result.exit_code,
                stdout: Some(result.stdout),
                _stderr: Some(result.stderr),
            })
        } else {
            Err(MapReduceError::InvalidConfiguration {
                reason: "Step must have either 'claude' or 'shell' command".to_string(),
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

        // Process items in parallel with controlled concurrency
        let agent_futures: Vec<_> = work_items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let sem = Arc::clone(&semaphore);
                let agent_manager = Arc::clone(&self.agent_manager);
                let event_logger = Arc::clone(&self.event_logger);
                let result_collector = Arc::clone(&self.result_collector);
                let user_interaction = Arc::clone(&self.user_interaction);
                let claude_executor = Arc::clone(&self.claude_executor);
                let subprocess = Arc::clone(&self.subprocess);
                let map_phase = map_phase.clone();
                let env = env.clone();
                let job_id = self.job_id.clone();

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

                    // Execute agent with item
                    let result = Self::execute_agent_for_item(
                        &agent_manager,
                        &agent_id,
                        &item_id,
                        item,
                        &map_phase,
                        &env,
                        &user_interaction,
                        &claude_executor,
                        &subprocess,
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
                            }
                        }
                    };

                    // Add result to collector
                    result_collector.add_result(agent_result.clone()).await;

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
        agent_id: &str,
        item_id: &str,
        item: Value,
        map_phase: &MapPhase,
        env: &ExecutionEnvironment,
        user_interaction: &Arc<dyn UserInteraction>,
        claude_executor: &Arc<dyn ClaudeExecutor>,
        subprocess: &Arc<SubprocessManager>,
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
                env,
                claude_executor,
                subprocess,
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
                        env,
                        claude_executor,
                        subprocess,
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
            if let Some(ref stdout) = step_result.stdout {
                output.push_str(stdout);
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

        // Merge agent changes back to parent if successful
        if !all_commits.is_empty() {
            agent_manager
                .merge_agent_to_parent(&config.branch_name, env)
                .await
                .map_err(|e| {
                    MapReduceError::ProcessingError(format!("Failed to merge agent changes: {}", e))
                })?;
        }

        Ok(AgentResult {
            item_id: item_id.to_string(),
            status: AgentStatus::Success,
            output: Some(output),
            commits: all_commits,
            duration: std::time::Duration::from_secs(0), // Would be tracked properly
            error: None,
            worktree_path: Some(handle.worktree_path().to_path_buf()),
            branch_name: Some(handle.worktree_session.branch.clone()),
            worktree_session_id: Some(agent_id.to_string()),
            files_modified: all_files_modified,
        })
    }

    /// Execute a step in an agent's worktree
    async fn execute_step_in_agent_worktree(
        worktree_path: &Path,
        step: &WorkflowStep,
        variables: &HashMap<String, String>,
        _env: &ExecutionEnvironment,
        claude_executor: &Arc<dyn ClaudeExecutor>,
        subprocess: &Arc<SubprocessManager>,
    ) -> MapReduceResult<StepResult> {
        use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
        use crate::subprocess::ProcessCommandBuilder;

        // Create interpolation engine for variable substitution
        let mut engine = InterpolationEngine::default();
        let mut interp_context = InterpolationContext::new();

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
            interp_context.set("item", serde_json::Value::Object(item_obj));
        }

        // Set other variables
        for (key, value) in other_vars {
            interp_context.set(key, value);
        }

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

            Ok(StepResult {
                success: result.success,
                _exit_code: result.exit_code,
                stdout: Some(result.stdout),
                _stderr: Some(result.stderr),
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
                _exit_code: Some(exit_code),
                stdout: Some(output.stdout),
                _stderr: Some(output.stderr),
            })
        } else {
            Err(MapReduceError::InvalidConfiguration {
                reason: "Step must have either 'claude' or 'shell' command".to_string(),
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
    async fn get_worktree_commits(_worktree_path: &Path) -> Vec<String> {
        // Would use git commands to get actual commits
        // For now, return empty
        vec![]
    }

    /// Get modified files from a worktree
    async fn get_worktree_modified_files(_worktree_path: &Path) -> Vec<String> {
        // Would use git commands to get actual modified files
        // For now, return empty
        vec![]
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

        // Create variables with map results
        let mut variables = HashMap::new();
        variables.insert(
            "map.results".to_string(),
            serde_json::to_string(map_results).unwrap_or_default(),
        );
        variables.insert("map.successful".to_string(), summary.successful.to_string());
        variables.insert("map.failed".to_string(), summary.failed.to_string());
        variables.insert("map.total".to_string(), summary.total.to_string());

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

// Simple Claude executor implementation for MapReduce
struct SimpleClaudeExecutor {
    subprocess: Arc<SubprocessManager>,
}

#[async_trait::async_trait]
impl ClaudeExecutor for SimpleClaudeExecutor {
    async fn execute_claude_command(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: HashMap<String, String>,
    ) -> anyhow::Result<crate::cook::execution::ExecutionResult> {
        use crate::subprocess::ProcessCommandBuilder;

        // Execute claude command as a subprocess
        let cmd = ProcessCommandBuilder::new("claude")
            .arg("--no-interactive")
            .arg(command)
            .current_dir(working_dir)
            .envs(env_vars)
            .build();

        let output = self.subprocess.runner().run(cmd).await?;

        let exit_code = match output.status {
            ExitStatus::Success => 0,
            ExitStatus::Error(code) => code,
            ExitStatus::Timeout => 124,
            ExitStatus::Signal(_) => 1,
        };

        Ok(crate::cook::execution::ExecutionResult {
            success: exit_code == 0,
            exit_code: Some(exit_code),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    async fn check_claude_cli(&self) -> anyhow::Result<bool> {
        Ok(true) // Assume Claude is available
    }

    async fn get_claude_version(&self) -> anyhow::Result<String> {
        Ok("Claude CLI v1.0.0".to_string()) // Mock version
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
