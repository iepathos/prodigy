//! MapReduce executor for parallel workflow execution
//!
//! Implements parallel execution of workflow steps across multiple agents
//! using isolated git worktrees for fault isolation and parallelism.

use crate::commands::{AttributeValue, CommandRegistry, ExecutionContext};
use crate::cook::execution::data_pipeline::DataPipeline;
use crate::cook::execution::dlq::{DeadLetterQueue, DeadLetteredItem, ErrorType, FailureDetail};
use crate::cook::execution::errors::{ErrorContext, MapReduceError, MapReduceResult, SpanInfo};
use crate::cook::execution::events::{EventLogger, EventWriter, JsonlEventWriter, MapReduceEvent};
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::state::{DefaultJobStateManager, JobStateManager, MapReduceJobState};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{CommandType, StepResult, WorkflowStep};
use crate::subprocess::SubprocessManager;
use crate::worktree::WorktreeManager;
// Keep anyhow imports for backwards compatibility with state.rs which still uses anyhow::Result
use chrono::Utc;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::timeout as tokio_timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceConfig {
    /// Path to input JSON file
    pub input: PathBuf,
    /// JSON path expression to extract work items
    #[serde(default)]
    pub json_path: String,
    /// Maximum number of parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Timeout per agent in seconds
    #[serde(default = "default_timeout")]
    pub timeout_per_agent: u64,
    /// Number of retry attempts on failure
    #[serde(default = "default_retry")]
    pub retry_on_failure: u32,
    /// Maximum number of items to process
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Number of items to skip
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

fn default_max_parallel() -> usize {
    10
}

fn default_timeout() -> u64 {
    600 // 10 minutes
}

fn default_retry() -> u32 {
    2
}

/// Map phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhase {
    /// Input configuration
    #[serde(flatten)]
    pub config: MapReduceConfig,
    /// Agent template commands
    pub agent_template: Vec<WorkflowStep>,
    /// Optional filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Optional sort field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
}

/// Reduce phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhase {
    /// Commands to execute in reduce phase
    pub commands: Vec<WorkflowStep>,
}

/// Status of an agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Pending,
    Running,
    Success,
    Failed(String),
    Timeout,
    Retrying(u32),
}

/// Result from a single agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Unique identifier for the work item
    pub item_id: String,
    /// Status of the agent execution
    pub status: AgentStatus,
    /// Output from the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Git commits created by the agent
    #[serde(default)]
    pub commits: Vec<String>,
    /// Duration of execution
    pub duration: Duration,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Worktree path used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
    /// Branch name created for this agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    /// Worktree session ID for cleanup tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_session_id: Option<String>,
    /// Files modified by this agent
    #[serde(default)]
    pub files_modified: Vec<PathBuf>,
}

/// Options for resuming a MapReduce job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeOptions {
    /// Force resume even if job appears complete
    pub force: bool,
    /// Maximum additional retries for failed items
    pub max_additional_retries: u32,
    /// Skip validation of checkpoint integrity
    pub skip_validation: bool,
}

impl Default for ResumeOptions {
    fn default() -> Self {
        Self {
            force: false,
            max_additional_retries: 2,
            skip_validation: false,
        }
    }
}

/// Result of resuming a MapReduce job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    /// Job ID that was resumed
    pub job_id: String,
    /// Checkpoint version resumed from
    pub resumed_from_version: u32,
    /// Total number of work items
    pub total_items: usize,
    /// Number of already completed items
    pub already_completed: usize,
    /// Number of remaining items to process
    pub remaining_items: usize,
    /// Final results after resumption
    pub final_results: Vec<AgentResult>,
}

/// Agent operation types for detailed status display
#[derive(Debug, Clone)]
enum AgentOperation {
    Idle,
    Setup(String),
    Claude(String),
    Shell(String),
    Test(String),
    Handler(String),
    Retrying(String, u32),
    Complete,
}

/// Progress tracking for parallel execution
struct ProgressTracker {
    #[allow(dead_code)]
    multi_progress: MultiProgress,
    overall_bar: ProgressBar,
    agent_bars: Vec<ProgressBar>,
    tick_handle: Option<JoinHandle<()>>,
    is_finished: Arc<AtomicBool>,
    #[allow(dead_code)]
    start_time: Instant,
    agent_operations: Arc<RwLock<Vec<AgentOperation>>>,
}

impl ProgressTracker {
    fn new(total_items: usize, max_parallel: usize) -> Self {
        let multi_progress = MultiProgress::new();

        // Overall progress bar
        let overall_bar = multi_progress.add(ProgressBar::new(total_items as u64));
        overall_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("##-"),
        );
        overall_bar.set_message("Processing items...");

        // Enable steady tick for timer updates
        overall_bar.enable_steady_tick(Duration::from_millis(100));

        // Individual agent progress bars
        let mut agent_bars = Vec::new();
        let mut agent_operations = Vec::new();
        for i in 0..max_parallel.min(total_items) {
            let bar = multi_progress.add(ProgressBar::new(100));
            bar.set_style(
                ProgressStyle::default_bar()
                    .template(&format!("  Agent {:2}: {{msg}}", i + 1))
                    .unwrap(),
            );
            bar.set_message("Idle");
            agent_bars.push(bar);
            agent_operations.push(AgentOperation::Idle);
        }

        Self {
            multi_progress,
            overall_bar,
            agent_bars,
            tick_handle: None,
            is_finished: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            agent_operations: Arc::new(RwLock::new(agent_operations)),
        }
    }

    fn update_agent(&self, agent_index: usize, message: &str) {
        if agent_index < self.agent_bars.len() {
            self.agent_bars[agent_index].set_message(message.to_string());
        }
    }

    async fn update_agent_operation(&self, agent_index: usize, operation: AgentOperation) {
        let mut ops = self.agent_operations.write().await;
        if agent_index < ops.len() {
            ops[agent_index] = operation.clone();

            // Format the operation for display
            let message = match operation {
                AgentOperation::Idle => "Idle".to_string(),
                AgentOperation::Setup(cmd) => {
                    format!("[setup] {}", Self::truncate_command(&cmd, 40))
                }
                AgentOperation::Claude(cmd) => {
                    format!("[claude] {}", Self::truncate_command(&cmd, 40))
                }
                AgentOperation::Shell(cmd) => {
                    format!("[shell] {}", Self::truncate_command(&cmd, 40))
                }
                AgentOperation::Test(cmd) => format!("[test] {}", Self::truncate_command(&cmd, 40)),
                AgentOperation::Handler(name) => format!("[handler] {}", name),
                AgentOperation::Retrying(item, attempt) => {
                    format!("Retrying {} (attempt {})", item, attempt)
                }
                AgentOperation::Complete => "Complete".to_string(),
            };

            self.update_agent(agent_index, &message);
        }
    }

    fn truncate_command(cmd: &str, max_len: usize) -> String {
        if cmd.len() <= max_len {
            cmd.to_string()
        } else {
            format!("{}...", &cmd[..max_len - 3])
        }
    }

    fn complete_item(&self) {
        self.overall_bar.inc(1);
    }

    fn finish(&self, message: &str) {
        self.is_finished.store(true, Ordering::Relaxed);
        self.overall_bar.finish_with_message(message.to_string());
        for bar in &self.agent_bars {
            bar.finish_and_clear();
        }
    }

    fn start_timer(&mut self) {
        let is_finished = self.is_finished.clone();
        let overall_bar = self.overall_bar.clone();

        // Spawn timer update task
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                if is_finished.load(Ordering::Relaxed) {
                    break;
                }
                overall_bar.tick();
            }
        });

        self.tick_handle = Some(handle);
    }
}

/// Context for agent-specific command execution
#[derive(Clone)]
pub struct AgentContext {
    /// Unique identifier for this agent
    pub item_id: String,
    /// Path to the agent's isolated worktree
    pub worktree_path: PathBuf,
    /// Name of the agent's worktree
    pub worktree_name: String,
    /// Variables available for interpolation
    pub variables: HashMap<String, String>,
    /// Last shell command output
    pub shell_output: Option<String>,
    /// Environment for command execution
    pub environment: ExecutionEnvironment,
    /// Current retry count for failed commands
    pub retry_count: u32,
    /// Captured outputs from previous steps
    pub captured_outputs: HashMap<String, String>,
    /// Iteration-specific variables
    pub iteration_vars: HashMap<String, String>,
}

impl AgentContext {
    /// Create a new agent context
    pub fn new(
        item_id: String,
        worktree_path: PathBuf,
        worktree_name: String,
        environment: ExecutionEnvironment,
    ) -> Self {
        Self {
            item_id,
            worktree_path,
            worktree_name,
            variables: HashMap::new(),
            shell_output: None,
            environment,
            retry_count: 0,
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
        }
    }

    /// Update context with command output
    pub fn update_with_output(&mut self, output: Option<String>) {
        if let Some(out) = output {
            self.shell_output = Some(out.clone());
            self.variables
                .insert("shell.output".to_string(), out.clone());
            self.variables.insert("shell.last_output".to_string(), out);
        }
    }

    /// Convert to InterpolationContext
    pub fn to_interpolation_context(&self) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add all variables
        for (key, value) in &self.variables {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add shell output
        if let Some(ref output) = self.shell_output {
            context.set(
                "shell",
                json!({
                    "output": output,
                    "last_output": output
                }),
            );
        }

        // Add captured outputs
        for (key, value) in &self.captured_outputs {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add iteration variables
        for (key, value) in &self.iteration_vars {
            context.set(key.clone(), Value::String(value.clone()));
        }

        context
    }
}

/// MapReduce executor for parallel workflow execution
pub struct MapReduceExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    worktree_manager: Arc<WorktreeManager>,
    project_root: PathBuf,
    interpolation_engine: Arc<Mutex<InterpolationEngine>>,
    command_registry: Arc<CommandRegistry>,
    subprocess: Arc<SubprocessManager>,
    state_manager: Arc<dyn JobStateManager>,
    event_logger: Arc<EventLogger>,
    dlq: Option<Arc<DeadLetterQueue>>,
    correlation_id: String,
}

impl MapReduceExecutor {
    /// Create error context with correlation ID
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

    /// Create a new MapReduce executor
    pub async fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        worktree_manager: Arc<WorktreeManager>,
        project_root: PathBuf,
    ) -> Self {
        // Create state directory path
        let state_dir = project_root.join(".prodigy").join("mapreduce");
        let state_manager = Arc::new(DefaultJobStateManager::new(state_dir.clone()));

        // Create event logger with file-based writer
        let events_dir = state_dir.join("events");
        let event_writers: Vec<Box<dyn EventWriter>> = vec![
            // Primary JSONL writer for persistence
            Box::new(
                JsonlEventWriter::new(events_dir.join("global").join("events.jsonl"))
                    .await
                    .unwrap_or_else(|_| {
                        // Fallback to temp directory if creation fails
                        let temp_path = std::env::temp_dir().join("prodigy_events.jsonl");
                        warn!(
                            "Failed to create event logger in project dir, using temp: {:?}",
                            temp_path
                        );
                        futures::executor::block_on(JsonlEventWriter::new(temp_path))
                            .expect("Failed to create fallback event logger")
                    }),
            ),
        ];
        let event_logger = Arc::new(EventLogger::new(event_writers));

        Self {
            claude_executor,
            session_manager,
            user_interaction,
            worktree_manager,
            project_root,
            interpolation_engine: Arc::new(Mutex::new(InterpolationEngine::new(false))),
            command_registry: Arc::new(CommandRegistry::with_defaults().await),
            subprocess: Arc::new(SubprocessManager::production()),
            state_manager,
            event_logger,
            dlq: None, // Will be initialized per job
            correlation_id: Uuid::new_v4().to_string(),
        }
    }

    /// Execute a MapReduce workflow
    pub async fn execute(
        &mut self,
        map_phase: &MapPhase,
        reduce_phase: Option<&ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        let start_time = Instant::now();

        // Load and parse work items with filtering and sorting
        let work_items = self
            .load_work_items_with_pipeline(&map_phase.config, &map_phase.filter, &map_phase.sort_by)
            .await?;

        self.user_interaction.display_info(&format!(
            "Starting MapReduce execution with {} items, max {} parallel agents",
            work_items.len(),
            map_phase.config.max_parallel
        ));

        // Create a new job with persistent state
        let job_id = self
            .state_manager
            .create_job(map_phase.config.clone(), work_items.clone())
            .await?;

        debug!("Created MapReduce job with ID: {}", job_id);

        // Initialize Dead Letter Queue for this job
        let dlq_path = self.project_root.join(".prodigy");
        self.dlq = Some(Arc::new(
            DeadLetterQueue::new(
                job_id.clone(),
                dlq_path,
                1000, // Max 1000 items in DLQ
                30,   // 30 days retention
                Some(self.event_logger.clone()),
            )
            .await?,
        ));

        // Log job started event
        self.event_logger
            .log(MapReduceEvent::JobStarted {
                job_id: job_id.clone(),
                config: map_phase.config.clone(),
                total_items: work_items.len(),
                timestamp: Utc::now(),
            })
            .await
            .unwrap_or_else(|e| warn!("Failed to log job started event: {}", e));

        // Execute map phase with state tracking
        let map_results = self
            .execute_map_phase_with_state(&job_id, map_phase, work_items, env)
            .await?;

        // Execute reduce phase if specified AND there were items to process
        // Skip reduce if no items were processed or all failed
        if let Some(reduce_phase) = reduce_phase {
            if map_results.is_empty() {
                self.user_interaction.display_warning(
                    "⚠️ Skipping reduce phase: no items were processed in map phase",
                );
            } else {
                let successful_count = map_results
                    .iter()
                    .filter(|r| matches!(r.status, AgentStatus::Success))
                    .count();

                if successful_count == 0 {
                    self.user_interaction
                        .display_warning("⚠️ Skipping reduce phase: all map agents failed");
                } else {
                    // Mark reduce phase as started
                    self.state_manager.start_reduce_phase(&job_id).await?;

                    self.execute_reduce_phase(reduce_phase, &map_results, env)
                        .await?;

                    // Mark reduce phase as completed
                    self.state_manager
                        .complete_reduce_phase(&job_id, None)
                        .await?;
                }
            }
        }

        // Mark job as complete
        self.state_manager.mark_job_complete(&job_id).await?;

        // Report summary
        let duration = start_time.elapsed();
        self.report_summary(&map_results, duration);

        // Log job completion event
        let success_count = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failure_count = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();

        self.event_logger
            .log(MapReduceEvent::JobCompleted {
                job_id: job_id.clone(),
                duration: chrono::Duration::from_std(duration)
                    .unwrap_or(chrono::Duration::seconds(0)),
                success_count,
                failure_count,
            })
            .await
            .unwrap_or_else(|e| warn!("Failed to log job completed event: {}", e));

        // Report DLQ statistics if any items were added
        if let Some(dlq) = &self.dlq {
            if let Ok(stats) = dlq.get_stats().await {
                if stats.total_items > 0 {
                    self.user_interaction.display_warning(&format!(
                        "Dead Letter Queue: {} items failed permanently (run 'prodigy dlq list' to view)",
                        stats.total_items
                    ));
                }
            }
        }

        Ok(map_results)
    }

    /// Execute map phase with state tracking
    async fn execute_map_phase_with_state(
        &self,
        job_id: &str,
        map_phase: &MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        // Execute the normal map phase
        let results = self.execute_map_phase(map_phase, work_items, env).await?;

        // Update state for each result
        for result in &results {
            self.state_manager
                .update_agent_result(job_id, result.clone())
                .await?;
        }

        Ok(results)
    }

    /// Resume a MapReduce job from checkpoint with options
    pub async fn resume_job_with_options(
        &self,
        job_id: &str,
        options: ResumeOptions,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<ResumeResult> {
        // Load job state from checkpoint
        let state = self.state_manager.get_job_state(job_id).await?;

        // Validate checkpoint integrity unless skipped
        if !options.skip_validation {
            self.validate_checkpoint(&state)?;
        }

        // Check if job is already complete and not forcing
        if state.is_complete && !options.force {
            return Ok(ResumeResult {
                job_id: job_id.to_string(),
                resumed_from_version: state.checkpoint_version,
                total_items: state.total_items,
                already_completed: state.completed_agents.len(),
                remaining_items: 0,
                final_results: state.agent_results.into_values().collect(),
            });
        }

        self.user_interaction.display_info(&format!(
            "Resuming MapReduce job {} from checkpoint v{}",
            job_id, state.checkpoint_version
        ));

        // Display progress information
        self.user_interaction.display_info(&format!(
            "Progress: {} completed, {} failed, {} pending",
            state.successful_count,
            state.failed_count,
            state.pending_items.len()
        ));

        let already_completed = state.completed_agents.len();
        let mut final_results: Vec<AgentResult>;
        let mut remaining_count = 0;

        // Log job resumed event
        self.event_logger
            .log(MapReduceEvent::JobResumed {
                job_id: job_id.to_string(),
                checkpoint_version: state.checkpoint_version,
                pending_items: state.pending_items.len(),
            })
            .await
            .unwrap_or_else(|e| log::warn!("Failed to log job resumed event: {}", e));

        // Check if map phase is complete
        if !state.is_map_phase_complete() {
            // Calculate pending items
            let pending_items =
                self.calculate_pending_items(&state, options.max_additional_retries)?;
            remaining_count = pending_items.len();

            if !pending_items.is_empty() {
                self.user_interaction.display_info(&format!(
                    "Resuming map phase with {} remaining items",
                    pending_items.len()
                ));

                // Create a map phase config from the stored state
                let map_phase = MapPhase {
                    config: state.config.clone(),
                    agent_template: vec![], // This would need to be stored in state
                    filter: None,
                    sort_by: None,
                };

                // Execute remaining items
                let new_results = self
                    .execute_map_phase(&map_phase, pending_items, env)
                    .await?;

                // Update state with new results
                for result in &new_results {
                    self.state_manager
                        .update_agent_result(job_id, result.clone())
                        .await?;
                }

                // Combine with existing results
                final_results = state.agent_results.into_values().collect();
                final_results.extend(new_results);
            } else {
                final_results = state.agent_results.into_values().collect();
            }
        } else {
            // Map phase is complete
            final_results = state.agent_results.into_values().collect();

            if state.reduce_phase_state.is_none() {
                self.user_interaction
                    .display_info("Map phase complete, reduce phase pending");
            }
        }

        Ok(ResumeResult {
            job_id: job_id.to_string(),
            resumed_from_version: state.checkpoint_version,
            total_items: state.total_items,
            already_completed,
            remaining_items: remaining_count,
            final_results,
        })
    }

    /// Resume a MapReduce job from checkpoint (backward compatibility)
    pub async fn resume_job(
        &self,
        job_id: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        // Use the new method with default options for backward compatibility
        let result = self
            .resume_job_with_options(job_id, ResumeOptions::default(), env)
            .await?;
        Ok(result.final_results)
    }

    /// Validate checkpoint integrity
    fn validate_checkpoint(&self, state: &MapReduceJobState) -> MapReduceResult<()> {
        let context = self.create_error_context("checkpoint_validation");
        // Basic validation checks
        if state.job_id.is_empty() {
            return Err(MapReduceError::CheckpointCorrupted {
                job_id: "<empty>".to_string(),
                version: state.checkpoint_version,
                details: "Empty job ID in checkpoint".to_string(),
            }
            .with_context(context)
            .error);
        }

        if state.work_items.is_empty() {
            let context = self.create_error_context("checkpoint_validation");
            return Err(MapReduceError::CheckpointCorrupted {
                job_id: state.job_id.clone(),
                version: state.checkpoint_version,
                details: "No work items in checkpoint".to_string(),
            }
            .with_context(context)
            .error);
        }

        // Verify counts are consistent
        let total_processed = state.completed_agents.len();
        if total_processed > state.total_items {
            let context = self.create_error_context("checkpoint_validation");
            return Err(MapReduceError::CheckpointCorrupted {
                job_id: state.job_id.clone(),
                version: state.checkpoint_version,
                details: format!(
                    "Processed count ({}) exceeds total items ({})",
                    total_processed, state.total_items
                ),
            }
            .with_context(context)
            .error);
        }

        // Verify all completed agents have results
        for agent_id in &state.completed_agents {
            if !state.agent_results.contains_key(agent_id) {
                let context = self.create_error_context("checkpoint_validation");
                return Err(MapReduceError::CheckpointCorrupted {
                    job_id: state.job_id.clone(),
                    version: state.checkpoint_version,
                    details: format!("Completed agent {} has no result", agent_id),
                }
                .with_context(context)
                .error);
            }
        }

        Ok(())
    }

    /// Calculate pending items for resumption
    fn calculate_pending_items(
        &self,
        state: &MapReduceJobState,
        max_additional_retries: u32,
    ) -> MapReduceResult<Vec<Value>> {
        let mut pending_items = Vec::new();

        // Add never-attempted items
        for (i, item) in state.work_items.iter().enumerate() {
            let item_id = format!("item_{}", i);
            if !state.completed_agents.contains(&item_id)
                && !state.failed_agents.contains_key(&item_id)
            {
                pending_items.push(item.clone());
            }
        }

        // Add retriable failed items
        let max_retries = state.config.retry_on_failure + max_additional_retries;
        for (item_id, failure) in &state.failed_agents {
            if failure.attempts < max_retries {
                if let Some(idx) = item_id
                    .strip_prefix("item_")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    if idx < state.work_items.len() {
                        pending_items.push(state.work_items[idx].clone());
                    }
                }
            }
        }

        Ok(pending_items)
    }

    /// Check if a job can be resumed
    pub async fn can_resume_job(&self, job_id: &str) -> bool {
        match self.state_manager.get_job_state(job_id).await {
            Ok(state) => !state.is_complete,
            Err(_) => false,
        }
    }

    /// List resumable jobs
    pub async fn list_resumable_jobs(&self) -> MapReduceResult<Vec<String>> {
        // This would need implementation in the state manager
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Load work items from JSON file with pipeline processing
    async fn load_work_items_with_pipeline(
        &self,
        config: &MapReduceConfig,
        filter: &Option<String>,
        sort_by: &Option<String>,
    ) -> MapReduceResult<Vec<Value>> {
        let input_path = if config.input.is_absolute() {
            config.input.clone()
        } else {
            self.project_root.join(&config.input)
        };

        debug!("Attempting to read input file: {}", input_path.display());

        // Check if file exists first
        if !input_path.exists() {
            let context = self.create_error_context("load_work_items");
            return Err(MapReduceError::WorkItemLoadFailed {
                path: input_path.clone(),
                reason: "File does not exist".to_string(),
                source: None,
            }
            .with_context(context)
            .error);
        }

        let file_size = std::fs::metadata(&input_path)?.len();
        debug!("Input file size: {} bytes", file_size);

        let content = tokio::fs::read_to_string(&input_path).await.map_err(|e| {
            let context = self.create_error_context("load_work_items");
            MapReduceError::WorkItemLoadFailed {
                path: input_path.clone(),
                reason: format!("Failed to read file: {}", e),
                source: Some(Box::new(e)),
            }
            .with_context(context)
            .error
        })?;

        debug!("Read {} bytes from input file", content.len());

        let json: Value = serde_json::from_str(&content).map_err(|e| {
            let context = self.create_error_context("load_work_items");
            MapReduceError::WorkItemLoadFailed {
                path: input_path.clone(),
                reason: "Failed to parse JSON".to_string(),
                source: Some(Box::new(e)),
            }
            .with_context(context)
            .error
        })?;

        // Debug: Show the top-level structure
        if let Value::Object(ref map) = json {
            let keys: Vec<_> = map.keys().cloned().collect();
            debug!("JSON top-level keys: {:?}", keys);
        }

        // Debug: Log the JSON path configuration
        debug!(
            "Loading work items with JSON path: '{}', filter: {:?}, sort: {:?}",
            config.json_path, filter, sort_by
        );

        // Use data pipeline for extraction, filtering, and sorting
        let json_path = if config.json_path.is_empty() {
            None
        } else {
            Some(config.json_path.clone())
        };

        // Create pipeline with all configuration options
        let mut pipeline = DataPipeline::from_config(
            json_path.clone(),
            filter.clone(),
            sort_by.clone(),
            config.max_items,
        )?;

        // Set offset if specified
        if let Some(offset) = config.offset {
            pipeline.offset = Some(offset);
        }

        // Debug: Show what JSON path will be used
        if let Some(ref path) = json_path {
            debug!("Using JSON path expression: {}", path);
        } else {
            debug!("No JSON path specified, treating input as array or single item");
        }

        let items = pipeline.process(&json)?;

        debug!(
            "Loaded {} work items after pipeline processing",
            items.len()
        );

        Ok(items)
    }

    /// Execute the map phase with parallel agents
    async fn execute_map_phase(
        &self,
        map_phase: &MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<Vec<AgentResult>> {
        let total_items = work_items.len();

        // If there are no items to process, return empty results
        if total_items == 0 {
            self.user_interaction
                .display_warning("No items to process in map phase");
            return Ok(Vec::new());
        }

        let max_parallel = map_phase.config.max_parallel.min(total_items);

        // Create progress tracker and start timer
        let mut progress_tracker = ProgressTracker::new(total_items, max_parallel);
        progress_tracker.start_timer();
        let progress = Arc::new(progress_tracker);

        // Create channels for work distribution (ensure buffer is at least 1)
        let (work_tx, work_rx) = mpsc::channel::<(usize, Value)>(total_items.max(1));
        let work_rx = Arc::new(RwLock::new(work_rx));

        // Send all work items to the queue
        for (index, item) in work_items.into_iter().enumerate() {
            work_tx.send((index, item)).await.map_err(|e| {
                let context = self.create_error_context("map_phase_execution");
                MapReduceError::General {
                    message: format!("Failed to send work item to queue: {}", e),
                    source: None,
                }
                .with_context(context)
                .error
            })?;
        }
        drop(work_tx); // Close the sender

        // Results collection
        let results = Arc::new(RwLock::new(Vec::new()));

        // Spawn worker tasks
        let mut workers = Vec::new();
        for agent_index in 0..max_parallel {
            let work_rx = work_rx.clone();
            let results = results.clone();
            let progress = progress.clone();
            let map_phase = map_phase.clone();
            let env = env.clone();
            let executor = self.clone_executor();

            let handle: JoinHandle<MapReduceResult<()>> = tokio::spawn(async move {
                executor
                    .run_agent(agent_index, work_rx, results, progress, map_phase, env)
                    .await
            });

            workers.push(handle);
        }

        // Wait for all workers to complete
        for worker in workers {
            match worker.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    self.user_interaction
                        .display_warning(&format!("Worker error: {}", e));
                }
                Err(join_err) => {
                    let context = self.create_error_context("map_phase_execution");
                    return Err(MapReduceError::General {
                        message: format!("Worker task panicked: {}", join_err),
                        source: None,
                    }
                    .with_context(context)
                    .error);
                }
            }
        }

        // Finish progress tracking
        progress.finish("Map phase completed");

        // Return collected results
        let results = results.read().await;
        Ok(results.clone())
    }

    /// Run a single agent worker
    async fn run_agent(
        &self,
        agent_index: usize,
        work_rx: Arc<RwLock<mpsc::Receiver<(usize, Value)>>>,
        results: Arc<RwLock<Vec<AgentResult>>>,
        progress: Arc<ProgressTracker>,
        map_phase: MapPhase,
        env: ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        loop {
            // Get next work item
            let work_item = {
                let mut rx = work_rx.write().await;
                rx.recv().await
            };

            let Some((item_index, item)) = work_item else {
                // No more work
                progress
                    .update_agent_operation(agent_index, AgentOperation::Complete)
                    .await;
                break;
            };

            let item_id = Self::extract_item_identifier(&item, item_index);
            progress.update_agent(agent_index, &format!("Processing {}", &item_id));

            // Execute work item with retries
            let mut attempt = 0;
            let agent_result = loop {
                attempt += 1;

                if attempt > 1 {
                    progress
                        .update_agent_operation(
                            agent_index,
                            AgentOperation::Retrying(item_id.clone(), attempt),
                        )
                        .await;
                }

                let result = self
                    .execute_agent_commands(
                        &item_id,
                        &item,
                        &map_phase.agent_template,
                        &env,
                        agent_index,
                        progress.clone(),
                    )
                    .await;

                match result {
                    Ok(res) => break res,
                    Err(_e) if attempt <= map_phase.config.retry_on_failure => {
                        // Retry on failure
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        // Final failure - add to DLQ
                        let agent_result = AgentResult {
                            item_id: item_id.clone(),
                            status: AgentStatus::Failed(e.to_string()),
                            output: None,
                            commits: vec![],
                            duration: Duration::from_secs(0),
                            error: Some(e.to_string()),
                            worktree_path: None,
                            branch_name: None,
                            worktree_session_id: None,
                            files_modified: vec![],
                        };

                        // Log error event with correlation ID
                        self.event_logger
                            .log(MapReduceEvent::AgentFailed {
                                job_id: env.session_id.clone(),
                                agent_id: format!("agent_{}", agent_index),
                                error: e.to_string(),
                                retry_eligible: false,
                            })
                            .await
                            .unwrap_or_else(|log_err| {
                                log::warn!("Failed to log agent error event: {}", log_err);
                            });

                        // Add to Dead Letter Queue
                        if let Some(dlq) = &self.dlq {
                            let failure_detail = FailureDetail {
                                attempt_number: attempt,
                                timestamp: Utc::now(),
                                error_type: ErrorType::Unknown,
                                error_message: e.to_string(),
                                stack_trace: None,
                                agent_id: format!("agent_{}", agent_index),
                                step_failed: "execute_agent_commands".to_string(),
                                duration_ms: 0,
                            };

                            let dlq_item = DeadLetteredItem {
                                item_id: item_id.clone(),
                                item_data: item.clone(),
                                first_attempt: Utc::now(),
                                last_attempt: Utc::now(),
                                failure_count: attempt,
                                failure_history: vec![failure_detail],
                                error_signature: DeadLetterQueue::create_error_signature(
                                    &ErrorType::Unknown,
                                    &e.to_string(),
                                ),
                                worktree_artifacts: None,
                                reprocess_eligible: true,
                                manual_review_required: false,
                            };

                            if let Err(dlq_err) = dlq.add(dlq_item).await {
                                error!("Failed to add item to DLQ: {}", dlq_err);
                            } else {
                                info!("Added failed item {} to Dead Letter Queue", item_id);
                            }
                        }

                        break agent_result;
                    }
                }
            };

            // Store result
            {
                let mut res = results.write().await;
                res.push(agent_result);
            }

            // Update progress
            progress.complete_item();
        }

        Ok(())
    }

    /// Execute commands for a single agent
    /// Extract variables from item data for context
    fn extract_item_variables(item: &Value) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        if let Value::Object(obj) = item {
            for (key, value) in obj {
                let str_value = match value {
                    Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                variables.insert(key.clone(), str_value);
            }
        }
        variables
    }

    /// Create standard variables for agent context
    fn create_standard_variables(
        worktree_name: &str,
        item_id: &str,
        session_id: &str,
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        variables.insert("worktree".to_string(), worktree_name.to_string());
        variables.insert("item_id".to_string(), item_id.to_string());
        variables.insert(
            "session_id".to_string(),
            format!("{}-{}", session_id, item_id),
        );
        variables
    }

    /// Initialize agent context with all necessary variables
    fn initialize_agent_context(
        item_id: &str,
        item: &Value,
        worktree_path: PathBuf,
        worktree_name: String,
        env: &ExecutionEnvironment,
    ) -> AgentContext {
        let agent_env = ExecutionEnvironment {
            working_dir: worktree_path.clone(),
            project_dir: env.project_dir.clone(),
            worktree_name: Some(worktree_name.clone()),
            session_id: format!("{}-{}", env.session_id, item_id),
        };

        let mut context = AgentContext::new(
            item_id.to_string(),
            worktree_path,
            worktree_name.clone(),
            agent_env,
        );

        // Add item variables
        let item_vars = Self::extract_item_variables(item);
        context.variables.extend(item_vars);

        // Add standard variables
        let std_vars = Self::create_standard_variables(&worktree_name, item_id, &env.session_id);
        context.variables.extend(std_vars);

        context
    }

    async fn execute_agent_commands(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
        agent_index: usize,
        progress: Arc<ProgressTracker>,
    ) -> MapReduceResult<AgentResult> {
        let start_time = Instant::now();
        let agent_id = format!("agent-{}-{}", agent_index, item_id);

        // Create isolated worktree session for this agent
        let worktree_session = self.worktree_manager.create_session().await.map_err(|e| {
            let context = self.create_error_context("worktree_creation");
            let error = MapReduceError::WorktreeCreationFailed {
                agent_id: agent_id.clone(),
                reason: e.to_string(),
                source: std::io::Error::other(e.to_string()),
            }
            .with_context(context);

            // Log error event with correlation ID
            let event_logger = self.event_logger.clone();
            let job_id = env.session_id.clone();
            let agent_id_clone = agent_id.clone();
            let error_msg = error.to_string();
            tokio::spawn(async move {
                event_logger
                    .log(MapReduceEvent::AgentFailed {
                        job_id,
                        agent_id: agent_id_clone,
                        error: error_msg,
                        retry_eligible: true,
                    })
                    .await
                    .unwrap_or_else(|e| log::warn!("Failed to log error event: {}", e));
            });

            error.error
        })?;
        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();
        let worktree_session_id = worktree_name.clone();

        // Log agent started event
        self.event_logger
            .log(MapReduceEvent::AgentStarted {
                job_id: env.session_id.clone(),
                agent_id: agent_id.clone(),
                item_id: item_id.to_string(),
                worktree: worktree_name.clone(),
                attempt: 1,
            })
            .await
            .unwrap_or_else(|e| log::warn!("Failed to log agent started event: {}", e));

        // Create branch name for this agent
        let branch_name = format!("prodigy-agent-{}-{}", env.session_id, item_id);

        // Initialize agent context with all variables
        let mut context = Self::initialize_agent_context(
            item_id,
            item,
            worktree_path.clone(),
            worktree_name.clone(),
            env,
        );

        // Execute template steps with real command execution
        let execution_result = self
            .execute_all_steps(
                template_steps,
                &mut context,
                item_id,
                agent_index,
                progress.clone(),
                &agent_id,
                env,
            )
            .await;

        let (total_output, execution_error) = execution_result;

        // Finalize and create result
        let result = self
            .finalize_agent_result(
                item_id,
                &worktree_path,
                &worktree_name,
                &branch_name,
                worktree_session_id,
                env,
                template_steps,
                execution_error,
                total_output,
                start_time,
            )
            .await?;

        // Log agent completed or failed event
        match &result.status {
            AgentStatus::Success => {
                self.event_logger
                    .log(MapReduceEvent::AgentCompleted {
                        job_id: env.session_id.clone(),
                        agent_id: agent_id.clone(),
                        duration: chrono::Duration::from_std(start_time.elapsed())
                            .unwrap_or(chrono::Duration::seconds(0)),
                        commits: vec![],
                    })
                    .await
                    .unwrap_or_else(|e| log::warn!("Failed to log agent completed event: {}", e));
            }
            AgentStatus::Failed(error) => {
                self.event_logger
                    .log(MapReduceEvent::AgentFailed {
                        job_id: env.session_id.clone(),
                        agent_id: agent_id.clone(),
                        error: error.clone(),
                        retry_eligible: true,
                    })
                    .await
                    .unwrap_or_else(|e| log::warn!("Failed to log agent failed event: {}", e));
            }
            _ => {
                // For other statuses (Pending, Running, Timeout, Retrying), no specific event needed
                log::debug!("Agent {} status: {:?}", agent_id, result.status);
            }
        }

        Ok(result)
    }

    /// Classify the operation type of a step for progress tracking
    fn classify_step_operation(step: &WorkflowStep) -> AgentOperation {
        match () {
            _ if step.claude.is_some() => AgentOperation::Claude(step.claude.clone().unwrap()),
            _ if step.shell.is_some() => AgentOperation::Shell(step.shell.clone().unwrap()),
            _ if step.test.is_some() => {
                AgentOperation::Test(step.test.as_ref().unwrap().command.clone())
            }
            _ if step.handler.is_some() => {
                AgentOperation::Handler(step.handler.as_ref().unwrap().name.clone())
            }
            _ => AgentOperation::Setup(step.name.clone().unwrap_or_else(|| "step".to_string())),
        }
    }

    /// Execute all steps for an agent
    #[allow(clippy::too_many_arguments)]
    async fn execute_all_steps(
        &self,
        template_steps: &[WorkflowStep],
        context: &mut AgentContext,
        item_id: &str,
        agent_index: usize,
        progress: Arc<ProgressTracker>,
        agent_id: &str,
        env: &ExecutionEnvironment,
    ) -> (String, Option<String>) {
        let mut total_output = String::new();
        let mut execution_error: Option<String> = None;

        for (step_index, step) in template_steps.iter().enumerate() {
            debug!(
                "Executing step {} for agent {}: {:?}",
                step_index + 1,
                item_id,
                step.name
            );

            // Update agent operation
            let operation = Self::classify_step_operation(step);
            progress
                .update_agent_operation(agent_index, operation)
                .await;

            // Log agent progress event
            let step_name = step
                .name
                .clone()
                .unwrap_or_else(|| format!("step_{}", step_index + 1));
            let progress_pct = ((step_index as f32 + 0.5) / template_steps.len() as f32) * 100.0;
            self.event_logger
                .log(MapReduceEvent::AgentProgress {
                    job_id: env.session_id.clone(),
                    agent_id: agent_id.to_string(),
                    step: step_name.clone(),
                    progress_pct,
                })
                .await
                .unwrap_or_else(|e| log::warn!("Failed to log agent progress event: {}", e));

            // Execute the step and handle result
            let step_result = self
                .execute_step_with_handlers(step, context, item_id, step_index)
                .await;

            match step_result {
                Ok((result, should_continue)) => {
                    // Update context and accumulate output
                    self.update_context_from_step(context, &result, step_index);
                    total_output.push_str(&self.format_step_output(&result, step, step_index));

                    // Handle success case
                    if result.success {
                        if let Some(on_success) = &step.on_success {
                            self.execute_success_handler(on_success, context, item_id, step_index)
                                .await;
                        }
                    }

                    if !should_continue {
                        execution_error = Some(format!(
                            "Step {} failed and workflow should stop",
                            step_index + 1
                        ));
                        break;
                    }
                }
                Err(error) => {
                    execution_error = Some(error.to_string());
                    break;
                }
            }
        }

        (total_output, execution_error)
    }

    /// Execute a single step with error handlers
    async fn execute_step_with_handlers(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
        item_id: &str,
        step_index: usize,
    ) -> MapReduceResult<(StepResult, bool)> {
        match self.execute_single_step(step, context).await {
            Ok(result) => Ok((result, true)),
            Err(e) => {
                let error_msg = format!("Step {} failed: {}", step_index + 1, e);
                error!("Agent {} error: {}", item_id, error_msg);

                if let Some(on_failure) = &step.on_failure {
                    info!("Executing on_failure handler for agent {}", item_id);
                    let handled = self
                        .handle_on_failure(on_failure, step, context, error_msg.clone())
                        .await?;

                    let failed_result = StepResult {
                        success: false,
                        exit_code: Some(1),
                        stdout: String::new(),
                        stderr: e.to_string(),
                    };

                    Ok((failed_result, handled))
                } else {
                    let context = self.create_error_context("execute_all_steps");
                    Err(MapReduceError::General {
                        message: error_msg,
                        source: None,
                    }
                    .with_context(context)
                    .error)
                }
            }
        }
    }

    /// Update context from step result
    fn update_context_from_step(
        &self,
        context: &mut AgentContext,
        result: &StepResult,
        step_index: usize,
    ) {
        if !result.stdout.is_empty() {
            context.update_with_output(Some(result.stdout.clone()));
            context.variables.insert(
                format!("step{}.output", step_index + 1),
                result.stdout.clone(),
            );
        }
    }

    /// Format step output for display
    fn format_step_output(
        &self,
        result: &StepResult,
        step: &WorkflowStep,
        step_index: usize,
    ) -> String {
        format!(
            "=== Step {} ({}) ===\n{}\n",
            step_index + 1,
            step.name.as_deref().unwrap_or("unnamed"),
            result.stdout
        )
    }

    /// Execute success handler for a step
    async fn execute_success_handler(
        &self,
        on_success: &WorkflowStep,
        context: &mut AgentContext,
        item_id: &str,
        step_index: usize,
    ) {
        debug!(
            "Executing on_success handler for agent {} step {}",
            item_id,
            step_index + 1
        );

        // Store output for handler
        if let Some(output) = context.shell_output.clone() {
            context
                .captured_outputs
                .insert("shell.output".to_string(), output.clone());
            context.variables.insert("shell.output".to_string(), output);
        }

        match self.execute_single_step(on_success, context).await {
            Ok(result) if !result.success => {
                warn!(
                    "on_success handler failed for agent {} step {}: {}",
                    item_id,
                    step_index + 1,
                    result.stderr
                );
            }
            Err(e) => {
                warn!(
                    "Failed to execute on_success handler for agent {} step {}: {}",
                    item_id,
                    step_index + 1,
                    e
                );
            }
            _ => {}
        }
    }

    /// Finalize agent result and handle merging/cleanup
    #[allow(clippy::too_many_arguments)]
    async fn finalize_agent_result(
        &self,
        item_id: &str,
        worktree_path: &Path,
        worktree_name: &str,
        branch_name: &str,
        worktree_session_id: String,
        env: &ExecutionEnvironment,
        template_steps: &[WorkflowStep],
        execution_error: Option<String>,
        total_output: String,
        start_time: Instant,
    ) -> MapReduceResult<AgentResult> {
        // Get commits and modified files
        let commits = self.get_worktree_commits(worktree_path).await?;
        let files_modified = self.get_modified_files(worktree_path).await?;

        // Determine status
        let status = execution_error
            .clone()
            .map(AgentStatus::Failed)
            .unwrap_or(AgentStatus::Success);

        // Handle merge and cleanup
        let merge_result = self
            .handle_merge_and_cleanup(
                execution_error.is_none(),
                env,
                worktree_path,
                worktree_name,
                branch_name,
                template_steps,
                item_id,
            )
            .await?;

        Ok(AgentResult {
            item_id: item_id.to_string(),
            status,
            output: Some(total_output),
            commits,
            duration: start_time.elapsed(),
            error: execution_error,
            worktree_path: if merge_result {
                None
            } else {
                Some(worktree_path.to_path_buf())
            },
            branch_name: Some(branch_name.to_string()),
            worktree_session_id: if merge_result {
                None
            } else {
                Some(worktree_session_id)
            },
            files_modified,
        })
    }

    /// Handle merging to parent and cleanup
    #[allow(clippy::too_many_arguments)]
    async fn handle_merge_and_cleanup(
        &self,
        is_successful: bool,
        env: &ExecutionEnvironment,
        worktree_path: &Path,
        worktree_name: &str,
        branch_name: &str,
        template_steps: &[WorkflowStep],
        item_id: &str,
    ) -> MapReduceResult<bool> {
        if is_successful && env.worktree_name.is_some() {
            // Create and checkout branch
            self.create_agent_branch(worktree_path, branch_name).await?;

            // Try to merge
            match self.merge_agent_to_parent(branch_name, env).await {
                Ok(()) => {
                    info!("Successfully merged agent {} to parent worktree", item_id);
                    self.worktree_manager
                        .cleanup_session(worktree_name, true)
                        .await?;
                    Ok(true)
                }
                Err(e) => {
                    warn!("Failed to merge agent {} to parent: {}", item_id, e);
                    Ok(false)
                }
            }
        } else {
            // Cleanup if no parent or failed
            if !template_steps.is_empty() {
                self.worktree_manager
                    .cleanup_session(worktree_name, true)
                    .await?;
            }
            Ok(false)
        }
    }

    /// Interpolate variables in a workflow step
    async fn interpolate_workflow_step(
        &self,
        step: &WorkflowStep,
        context: &InterpolationContext,
    ) -> MapReduceResult<WorkflowStep> {
        let mut engine = self.interpolation_engine.lock().await;

        // Clone the step to avoid modifying the original
        let mut interpolated = step.clone();

        // Interpolate all string fields that might contain variables
        if let Some(name) = &step.name {
            interpolated.name = Some(engine.interpolate(name, context)?);
        }

        if let Some(claude) = &step.claude {
            interpolated.claude = Some(engine.interpolate(claude, context)?);
        }

        if let Some(shell) = &step.shell {
            interpolated.shell = Some(engine.interpolate(shell, context)?);
        }

        if let Some(command) = &step.command {
            interpolated.command = Some(engine.interpolate(command, context)?);
        }

        // Interpolate environment variables
        let mut interpolated_env = HashMap::new();
        for (key, value) in &step.env {
            let interpolated_key = engine.interpolate(key, context)?;
            let interpolated_value = engine.interpolate(value, context)?;
            interpolated_env.insert(interpolated_key, interpolated_value);
        }
        interpolated.env = interpolated_env;

        // Note: Handler, on_failure, on_success would need recursive interpolation
        // which we're not implementing for this initial version

        Ok(interpolated)
    }

    /// Get commits from a worktree
    async fn get_worktree_commits(&self, worktree_path: &Path) -> MapReduceResult<Vec<String>> {
        use tokio::process::Command;

        let output = Command::new("git")
            .args(["log", "--format=%H", "HEAD~10..HEAD"])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let commits = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(commits)
    }

    /// Get modified files in a worktree
    async fn get_modified_files(&self, worktree_path: &Path) -> MapReduceResult<Vec<PathBuf>> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD~1..HEAD"])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(PathBuf::from)
            .collect();

        Ok(files)
    }

    /// Create a branch for an agent
    async fn create_agent_branch(
        &self,
        worktree_path: &Path,
        branch_name: &str,
    ) -> MapReduceResult<()> {
        // Create branch from current HEAD
        let output = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let context = self.create_error_context("create_agent_branch");
            return Err(MapReduceError::General {
                message: format!("Failed to create branch {}: {}", branch_name, stderr),
                source: None,
            }
            .with_context(context)
            .error);
        }

        Ok(())
    }

    /// Merge an agent's branch to the parent worktree
    async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        // Get parent worktree path (use working_dir if we're in a parent worktree)
        let parent_worktree_path = if env.worktree_name.is_some() {
            &env.working_dir
        } else {
            // If no parent worktree, use main repository
            &env.project_dir
        };

        // Use the /prodigy-merge-worktree command to handle the merge
        // This provides intelligent conflict resolution
        let merge_command = format!("/prodigy-merge-worktree {}", agent_branch);

        // Set up environment variables for the merge command
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Execute the merge command
        let result = self
            .claude_executor
            .execute_claude_command(&merge_command, parent_worktree_path, env_vars)
            .await?;

        if !result.success {
            let context = self.create_error_context("merge_to_parent");
            return Err(MapReduceError::WorktreeMergeConflict {
                agent_id: agent_branch.to_string(), // Use branch name as agent identifier
                branch: agent_branch.to_string(),
                conflicts: vec![result.stderr.clone()],
            }
            .with_context(context)
            .error);
        }

        // Validate parent state after merge (run basic checks)
        self.validate_parent_state(parent_worktree_path).await?;

        Ok(())
    }

    /// Validate the parent worktree state after a merge
    async fn validate_parent_state(&self, parent_path: &Path) -> MapReduceResult<()> {
        // Check that there are no merge conflicts
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(parent_path)
            .output()
            .await?;

        let status = String::from_utf8_lossy(&output.stdout);
        if status.contains("UU ") || status.contains("AA ") || status.contains("DD ") {
            let context = self.create_error_context("validate_parent_state");
            return Err(MapReduceError::General {
                message: "Unresolved merge conflicts detected in parent worktree".to_string(),
                source: None,
            }
            .with_context(context)
            .error);
        }

        // Run basic syntax check if it's a Rust project
        if parent_path.join("Cargo.toml").exists() {
            let check_output = Command::new("cargo")
                .args(["check", "--quiet"])
                .current_dir(parent_path)
                .output()
                .await?;

            if !check_output.status.success() {
                warn!("Parent worktree fails cargo check after merge, but continuing");
            }
        }

        Ok(())
    }

    /// Get display name for a step
    fn get_step_display_name(&self, step: &WorkflowStep) -> String {
        if let Some(claude_cmd) = &step.claude {
            format!("claude: {claude_cmd}")
        } else if let Some(shell_cmd) = &step.shell {
            format!("shell: {shell_cmd}")
        } else if let Some(test_cmd) = &step.test {
            format!("test: {}", test_cmd.command)
        } else if let Some(handler_step) = &step.handler {
            format!("handler: {}", handler_step.name)
        } else if let Some(name) = &step.name {
            name.clone()
        } else if let Some(command) = &step.command {
            command.clone()
        } else {
            "unnamed step".to_string()
        }
    }

    /// Execute the reduce phase
    async fn execute_reduce_phase(
        &self,
        reduce_phase: &ReducePhase,
        map_results: &[AgentResult],
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<()> {
        // All successful agents have already been merged to parent progressively
        let successful_count = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();

        let failed_count = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();

        self.user_interaction.display_info(&format!(
            "All {} successful agents merged to parent worktree",
            successful_count
        ));

        if failed_count > 0 {
            self.user_interaction.display_warning(&format!(
                "{} agents failed and were not merged",
                failed_count
            ));
        }

        self.user_interaction
            .display_progress("Starting reduce phase in parent worktree...");

        // Create interpolation context with map results
        let mut interp_context = InterpolationContext::new();

        // Add summary statistics
        interp_context.set(
            "map",
            json!({
                "successful": successful_count,
                "failed": failed_count,
                "total": map_results.len()
            }),
        );

        // Add complete results as JSON value
        let results_value = serde_json::to_value(map_results)?;
        interp_context.set("map.results", results_value);

        // Also add individual result access
        for (index, result) in map_results.iter().enumerate() {
            let result_value = serde_json::to_value(result)?;
            interp_context.set(format!("results[{}]", index), result_value);
        }

        // Create a context for reduce phase execution in parent worktree
        let mut reduce_context = AgentContext::new(
            "reduce".to_string(),
            env.working_dir.clone(),
            env.worktree_name
                .clone()
                .unwrap_or_else(|| "main".to_string()),
            env.clone(),
        );

        // Transfer map results to reduce context variables for shell command substitution
        reduce_context
            .variables
            .insert("map.successful".to_string(), successful_count.to_string());
        reduce_context
            .variables
            .insert("map.failed".to_string(), failed_count.to_string());
        reduce_context
            .variables
            .insert("map.total".to_string(), map_results.len().to_string());

        // Add complete results as JSON string for complex access patterns
        let results_json = serde_json::to_string(map_results).map_err(|e| {
            let context = self.create_error_context("reduce_phase_execution");
            MapReduceError::General {
                message: "Failed to serialize map results to JSON".to_string(),
                source: Some(Box::new(e)),
            }
            .with_context(context)
            .error
        })?;
        reduce_context
            .variables
            .insert("map.results_json".to_string(), results_json.clone());

        // Also add map.results for Claude command interpolation
        // This will be available when to_interpolation_context is called
        reduce_context
            .variables
            .insert("map.results".to_string(), results_json);

        // Add individual result summaries for easier access in shell commands
        for (index, result) in map_results.iter().enumerate() {
            // Add basic result info
            reduce_context
                .variables
                .insert(format!("result.{}.item_id", index), result.item_id.clone());
            reduce_context.variables.insert(
                format!("result.{}.status", index),
                match &result.status {
                    AgentStatus::Success => "success".to_string(),
                    AgentStatus::Failed(err) => format!("failed: {}", err),
                    AgentStatus::Timeout => "timeout".to_string(),
                    AgentStatus::Pending => "pending".to_string(),
                    AgentStatus::Running => "running".to_string(),
                    AgentStatus::Retrying(attempt) => format!("retrying: {}", attempt),
                },
            );

            // Add output if available (truncated for safety)
            if let Some(ref output) = result.output {
                let truncated_output = if output.len() > 500 {
                    format!("{}...[truncated]", &output[..500])
                } else {
                    output.clone()
                };
                reduce_context
                    .variables
                    .insert(format!("result.{}.output", index), truncated_output);
            }

            // Add commit count
            reduce_context.variables.insert(
                format!("result.{}.commits", index),
                result.commits.len().to_string(),
            );
        }

        // Validate that required variables are available for reduce phase
        self.validate_reduce_variables(&reduce_phase.commands, &reduce_context)?;

        // Execute reduce commands in parent worktree
        for (step_index, step) in reduce_phase.commands.iter().enumerate() {
            let step_display = self.get_step_display_name(step);
            self.user_interaction.display_progress(&format!(
                "Reduce step {}/{}: {}",
                step_index + 1,
                reduce_phase.commands.len(),
                step_display
            ));

            // Execute the step in parent worktree context
            let step_result = self.execute_single_step(step, &mut reduce_context).await?;

            if !step_result.success {
                // Check if there's an on_failure handler
                if let Some(on_failure) = &step.on_failure {
                    self.user_interaction.display_warning(&format!(
                        "Step {} failed, executing on_failure handler...",
                        step_index + 1
                    ));

                    // Handle the on_failure configuration
                    // Store the shell output in context for the handler to use
                    reduce_context.captured_outputs.insert(
                        "shell.output".to_string(),
                        format!("{}\n{}", step_result.stdout, step_result.stderr),
                    );
                    reduce_context.variables.insert(
                        "shell.output".to_string(),
                        format!("{}\n{}", step_result.stdout, step_result.stderr),
                    );

                    let error_msg = format!(
                        "Step failed with exit code {}: {}",
                        step_result.exit_code.unwrap_or(-1),
                        step_result.stderr
                    );

                    // Try to handle the failure
                    match self
                        .handle_on_failure(on_failure, step, &mut reduce_context, error_msg)
                        .await
                    {
                        Ok(handled) => {
                            if !handled {
                                // on_failure says we should fail
                                let context = self.create_error_context("reduce_phase_execution");
                                return Err(MapReduceError::General {
                                    message: format!(
                                        "Reduce step {} failed and fail_workflow is true",
                                        step_index + 1
                                    ),
                                    source: None,
                                }
                                .with_context(context)
                                .error);
                            }
                            // Otherwise continue to next step
                        }
                        Err(handler_err) => {
                            // Handler itself failed
                            if on_failure.should_fail_workflow() {
                                let context = self.create_error_context("reduce_phase_execution");
                                return Err(MapReduceError::General {
                                    message: format!(
                                        "Reduce step {} on_failure handler failed: {}",
                                        step_index + 1,
                                        handler_err
                                    ),
                                    source: None,
                                }
                                .with_context(context)
                                .error);
                            }
                            // Otherwise, log the error but continue
                            self.user_interaction.display_warning(&format!(
                                "on_failure handler failed but continuing: {}",
                                handler_err
                            ));
                        }
                    }
                } else {
                    // No on_failure handler, fail immediately
                    let context = self.create_error_context("reduce_phase_execution");
                    return Err(MapReduceError::General {
                        message: format!(
                            "Reduce step {} failed: {}",
                            step_index + 1,
                            step_result.stderr
                        ),
                        source: None,
                    }
                    .with_context(context)
                    .error);
                }
            } else {
                // Step succeeded - check if there's an on_success handler
                if let Some(on_success) = &step.on_success {
                    self.user_interaction.display_info(&format!(
                        "Step {} succeeded, executing on_success handler...",
                        step_index + 1
                    ));

                    // Store the successful output in context for the handler to use
                    reduce_context
                        .captured_outputs
                        .insert("shell.output".to_string(), step_result.stdout.clone());
                    reduce_context
                        .variables
                        .insert("shell.output".to_string(), step_result.stdout.clone());

                    // Execute the on_success handler
                    let success_result = self
                        .execute_single_step(on_success, &mut reduce_context)
                        .await?;

                    if !success_result.success {
                        self.user_interaction.display_warning(&format!(
                            "on_success handler failed for step {}: {}",
                            step_index + 1,
                            success_result.stderr
                        ));
                        // Note: We don't fail the workflow when on_success handler fails
                        // This is consistent with typical behavior - on_success is a bonus action
                    }
                }
            }

            // After successful execution, make captured outputs available as variables
            // for subsequent commands in the reduce phase
            for (key, value) in reduce_context.captured_outputs.clone() {
                reduce_context.variables.insert(key, value);
            }
        }

        self.user_interaction
            .display_success("Reduce phase completed successfully");

        // Don't merge here - let the orchestrator's cleanup handle it
        // This prevents double-merge attempts
        if env.worktree_name.is_some() && !self.should_auto_merge(env) {
            // Only show manual instructions if NOT auto-merging
            // (If auto-merging, orchestrator cleanup will handle it)
            self.user_interaction.display_info(&format!(
                "\nParent worktree ready for review: {}\n",
                env.worktree_name.as_ref().unwrap()
            ));
            self.user_interaction
                .display_info("To create a PR: git push origin <branch> && gh pr create");
        }

        Ok(())
    }

    /// Check if auto-merge is enabled
    fn should_auto_merge(&self, _env: &ExecutionEnvironment) -> bool {
        // Check for -y flag via environment variable
        std::env::var("PRODIGY_AUTO_MERGE").unwrap_or_default() == "true"
            || std::env::var("PRODIGY_AUTO_CONFIRM").unwrap_or_default() == "true"
    }

    /// Execute a single workflow step with agent context
    async fn execute_single_step(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
    ) -> MapReduceResult<StepResult> {
        // Interpolate the step using the agent's context
        let interp_context = context.to_interpolation_context();
        let interpolated_step = self
            .interpolate_workflow_step(step, &interp_context)
            .await?;

        // Determine command type
        let command_type = self.determine_command_type(&interpolated_step)?;

        // Execute the command based on its type
        let result = match command_type.clone() {
            CommandType::Claude(cmd) => self.execute_claude_command(&cmd, context).await?,
            CommandType::Shell(cmd) => {
                self.execute_shell_command(&cmd, context, step.timeout)
                    .await?
            }
            CommandType::Handler {
                handler_name,
                attributes,
            } => {
                self.execute_handler_command(&handler_name, &attributes, context)
                    .await?
            }
            CommandType::Legacy(cmd) => {
                // Legacy commands use Claude executor
                self.execute_claude_command(&cmd, context).await?
            }
            CommandType::Test(_) => {
                // Test commands are converted to shell commands
                if let Some(shell_cmd) = &interpolated_step.shell {
                    self.execute_shell_command(shell_cmd, context, step.timeout)
                        .await?
                } else {
                    let context = self.create_error_context("execute_single_step");
                    return Err(MapReduceError::InvalidConfiguration {
                        reason: "Test commands are not supported in MapReduce".to_string(),
                        field: "command_type".to_string(),
                        value: "test".to_string(),
                    }
                    .with_context(context)
                    .error);
                }
            }
        };

        // Capture output if requested
        if step.capture_output.is_enabled() && !result.stdout.is_empty() {
            // Get the variable name for this output (custom or default)
            if let Some(var_name) = step.capture_output.get_variable_name(&command_type) {
                // Store with the specified variable name
                context
                    .captured_outputs
                    .insert(var_name, result.stdout.clone());
            }

            // Also store as generic CAPTURED_OUTPUT for backward compatibility
            context
                .captured_outputs
                .insert("CAPTURED_OUTPUT".to_string(), result.stdout.clone());
        }

        Ok(result)
    }

    /// Determine command type from a workflow step
    fn determine_command_type(&self, step: &WorkflowStep) -> MapReduceResult<CommandType> {
        // Collect all specified command types
        let commands = Self::collect_command_types(step);

        // Validate exactly one command is specified
        Self::validate_command_count(&commands)?;

        // Extract and return the single command type
        commands.into_iter().next().ok_or_else(|| {
            let context = self.create_error_context("determine_command_type");
            MapReduceError::InvalidConfiguration {
                reason: "No valid command found in step".to_string(),
                field: "command".to_string(),
                value: "<none>".to_string(),
            }
            .with_context(context)
            .error
        })
    }

    /// Collect all command types from a workflow step
    pub(crate) fn collect_command_types(step: &WorkflowStep) -> Vec<CommandType> {
        let mut commands = Vec::new();

        if let Some(handler_step) = &step.handler {
            let attributes = handler_step
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), Self::json_to_attribute_value(v.clone())))
                .collect();
            commands.push(CommandType::Handler {
                handler_name: handler_step.name.clone(),
                attributes,
            });
        }

        if let Some(claude_cmd) = &step.claude {
            commands.push(CommandType::Claude(claude_cmd.clone()));
        }

        if let Some(shell_cmd) = &step.shell {
            commands.push(CommandType::Shell(shell_cmd.clone()));
        }

        if let Some(test_cmd) = &step.test {
            commands.push(CommandType::Test(test_cmd.clone()));
        }

        if let Some(name) = &step.name {
            let command = Self::format_legacy_command(name);
            commands.push(CommandType::Legacy(command));
        }

        if let Some(command) = &step.command {
            commands.push(CommandType::Legacy(command.clone()));
        }

        commands
    }

    /// Validate that exactly one command type is specified
    pub(crate) fn validate_command_count(commands: &[CommandType]) -> MapReduceResult<()> {
        match commands.len() {
            0 => Err(MapReduceError::InvalidConfiguration {
                reason: "No command specified".to_string(),
                field: "command".to_string(),
                value: "Use one of: claude, shell, test, handler, or name/command".to_string(),
            }),
            1 => Ok(()),
            _ => Err(MapReduceError::InvalidConfiguration {
                reason: "Multiple command types specified".to_string(),
                field: "command".to_string(),
                value: "Use only one of: claude, shell, test, handler, or name/command".to_string(),
            }),
        }
    }

    /// Format a legacy command name with leading slash if needed
    pub(crate) fn format_legacy_command(name: &str) -> String {
        if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/{name}")
        }
    }

    /// Convert serde_json::Value to AttributeValue
    fn json_to_attribute_value(value: serde_json::Value) -> AttributeValue {
        match value {
            serde_json::Value::String(s) => AttributeValue::String(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    AttributeValue::Number(i as f64)
                } else if let Some(f) = n.as_f64() {
                    AttributeValue::Number(f)
                } else {
                    AttributeValue::Number(0.0)
                }
            }
            serde_json::Value::Bool(b) => AttributeValue::Boolean(b),
            serde_json::Value::Array(arr) => {
                AttributeValue::Array(arr.into_iter().map(Self::json_to_attribute_value).collect())
            }
            serde_json::Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    map.insert(k, Self::json_to_attribute_value(v));
                }
                AttributeValue::Object(map)
            }
            serde_json::Value::Null => AttributeValue::Null,
        }
    }

    /// Execute a Claude command with agent context
    async fn execute_claude_command(
        &self,
        command: &str,
        context: &AgentContext,
    ) -> MapReduceResult<StepResult> {
        // Set up environment variables for the command
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "PRODIGY_CONTEXT_DIR".to_string(),
            context
                .worktree_path
                .join(".prodigy/context")
                .to_string_lossy()
                .to_string(),
        );
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());
        env_vars.insert(
            "PRODIGY_WORKTREE".to_string(),
            context.worktree_name.clone(),
        );

        // Execute the Claude command
        let result = self
            .claude_executor
            .execute_claude_command(command, &context.worktree_path, env_vars)
            .await?;

        Ok(StepResult {
            success: result.success,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
        })
    }

    /// Execute a shell command with agent context
    async fn execute_shell_command(
        &self,
        command: &str,
        context: &AgentContext,
        timeout: Option<u64>,
    ) -> MapReduceResult<StepResult> {
        use tokio::time::Duration;

        // Create command
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);

        // Set working directory to the agent's worktree
        cmd.current_dir(&context.worktree_path);

        // Set environment variables
        cmd.env("PRODIGY_WORKTREE", &context.worktree_name);
        cmd.env("PRODIGY_ITEM_ID", &context.item_id);
        cmd.env("PRODIGY_AUTOMATION", "true");

        // Execute with optional timeout
        let output = if let Some(timeout_secs) = timeout {
            let duration = Duration::from_secs(timeout_secs);
            match tokio_timeout(duration, cmd.output()).await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(MapReduceError::AgentTimeout {
                        job_id: "<unknown>".to_string(),
                        agent_id: "<unknown>".to_string(),
                        item_id: "<unknown>".to_string(),
                        duration_secs: timeout_secs,
                        last_operation: "shell command execution".to_string(),
                    });
                }
            }
        } else {
            cmd.output().await?
        };

        let result = StepResult {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        };

        // Enhanced error logging for variable substitution failures
        if !result.success && result.stderr.contains("bad substitution") {
            // Log detailed information about the substitution failure
            error!(
                "Shell command failed with variable substitution error:\n  \
                Original command: {}\n  \
                Available variables: {:?}\n  \
                Error: {}",
                command,
                context.variables.keys().collect::<Vec<_>>(),
                result.stderr
            );

            // Try to identify which variables were referenced but not available
            let missing_vars = self.find_missing_variables(command, &context.variables);
            if !missing_vars.is_empty() {
                error!("Potentially missing variables: {:?}", missing_vars);
            }
        }

        Ok(result)
    }

    /// Find variables referenced in a command that are not available in the context
    fn find_missing_variables(
        &self,
        command: &str,
        available_vars: &HashMap<String, String>,
    ) -> Vec<String> {
        use std::collections::HashSet;

        let mut missing = Vec::new();
        let mut found_vars = HashSet::new();

        // Simple regex-like pattern matching for ${variable} and $variable patterns
        // This is a basic implementation - for production use, consider using a proper regex library
        let chars: Vec<char> = command.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' {
                if i + 1 < chars.len() && chars[i + 1] == '{' {
                    // Handle ${variable} pattern
                    i += 2; // Skip ${
                    let start = i;
                    while i < chars.len() && chars[i] != '}' {
                        i += 1;
                    }
                    if i < chars.len() && start < i {
                        let var_name: String = chars[start..i].iter().collect();
                        found_vars.insert(var_name);
                    }
                } else if i + 1 < chars.len()
                    && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_')
                {
                    // Handle $variable pattern
                    i += 1;
                    let start = i;
                    while i < chars.len()
                        && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                    {
                        i += 1;
                    }
                    if start < i {
                        let var_name: String = chars[start..i].iter().collect();
                        found_vars.insert(var_name);
                    }
                    continue; // Don't increment i again
                }
            }
            i += 1;
        }

        // Check which variables are referenced but not available
        for var in found_vars {
            if !available_vars.contains_key(&var) {
                missing.push(var);
            }
        }

        missing
    }

    /// Execute a handler command with agent context
    async fn execute_handler_command(
        &self,
        handler_name: &str,
        attributes: &HashMap<String, AttributeValue>,
        context: &AgentContext,
    ) -> MapReduceResult<StepResult> {
        // Create execution context for the handler
        let mut exec_context = ExecutionContext::new(context.worktree_path.clone());

        // Add environment variables
        exec_context.add_env_var(
            "PRODIGY_WORKTREE".to_string(),
            context.worktree_name.clone(),
        );
        exec_context.add_env_var("PRODIGY_ITEM_ID".to_string(), context.item_id.clone());
        exec_context.add_env_var("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Execute the handler
        let result = self
            .command_registry
            .execute(handler_name, &exec_context, attributes.clone())
            .await;

        // Convert CommandResult to StepResult
        Ok(StepResult {
            success: result.is_success(),
            exit_code: result.exit_code,
            stdout: result.stdout.unwrap_or_else(|| {
                result
                    .data
                    .as_ref()
                    .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
                    .unwrap_or_default()
            }),
            stderr: result
                .stderr
                .unwrap_or_else(|| result.error.unwrap_or_default()),
        })
    }

    /// Handle on_failure logic for a failed step
    /// Returns Ok(true) if the failure was handled and execution should continue,
    /// Ok(false) if the workflow should fail, or an error if the handler itself failed
    async fn handle_on_failure(
        &self,
        on_failure: &crate::cook::workflow::OnFailureConfig,
        original_step: &WorkflowStep,
        context: &mut AgentContext,
        error: String,
    ) -> MapReduceResult<bool> {
        // Add error to context for interpolation
        context.variables.insert("error".to_string(), error.clone());
        context.variables.insert("last_error".to_string(), error);

        // Check if there's a handler to execute
        if let Some(handler_step) = on_failure.handler() {
            info!("Executing on_failure handler for agent {}", context.item_id);

            // Execute the on_failure handler step
            let handler_result = self.execute_single_step(&handler_step, context).await?;

            if !handler_result.success {
                warn!(
                    "on_failure handler failed for agent {}: {}",
                    context.item_id, handler_result.stderr
                );
                // If handler fails and fail_workflow is true, propagate failure
                if on_failure.should_fail_workflow() {
                    return Ok(false);
                }
            }

            // Check if we should retry the original command
            // Retry is determined by max_retries > 0 (consistent with regular workflows)
            if on_failure.should_retry() {
                let max_retries = on_failure.max_retries();
                info!(
                    "🔄 Will retry original command for agent {} (max_retries/max_attempts: {})",
                    context.item_id, max_retries
                );

                for retry in 1..=max_retries {
                    self.user_interaction.display_info(&format!(
                        "🔄 Retry attempt {}/{} for agent {}",
                        retry, max_retries, context.item_id
                    ));

                    // Create a copy of the step without on_failure to avoid recursion
                    let mut retry_step = original_step.clone();
                    retry_step.on_failure = None;

                    let retry_result = self.execute_single_step(&retry_step, context).await?;
                    if retry_result.success {
                        self.user_interaction.display_success(&format!(
                            "✅ Retry succeeded for agent {} on attempt {}/{}",
                            context.item_id, retry, max_retries
                        ));
                        return Ok(true); // Successfully handled
                    } else {
                        self.user_interaction.display_warning(&format!(
                            "❌ Retry attempt {}/{} failed for agent {}: {}",
                            retry,
                            max_retries,
                            context.item_id,
                            retry_result
                                .stderr
                                .lines()
                                .next()
                                .unwrap_or("unknown error")
                        ));
                    }
                }
                self.user_interaction.display_error(&format!(
                    "All {} retry attempts failed for agent {}",
                    max_retries, context.item_id
                ));
            } else {
                debug!(
                    "Not retrying original command (max_retries: {})",
                    on_failure.max_retries()
                );
            }
        }

        // Return whether we should continue based on fail_workflow setting
        Ok(!on_failure.should_fail_workflow())
    }

    /// Report execution summary
    fn report_summary(&self, results: &[AgentResult], duration: Duration) {
        let successful = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failed = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();
        let timeout = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Timeout))
            .count();

        let total_commits: usize = results.iter().map(|r| r.commits.len()).sum();

        self.user_interaction.display_info(&format!(
            "\n📊 MapReduce Execution Summary:
            Total items: {}
            Successful: {} ({:.1}%)
            Failed: {} ({:.1}%)
            Timeouts: {} ({:.1}%)
            Total commits: {}
            Total duration: {:.2}s
            Average time per item: {:.2}s",
            results.len(),
            successful,
            (successful as f64 / results.len() as f64) * 100.0,
            failed,
            (failed as f64 / results.len() as f64) * 100.0,
            timeout,
            (timeout as f64 / results.len() as f64) * 100.0,
            total_commits,
            duration.as_secs_f64(),
            duration.as_secs_f64() / results.len() as f64,
        ));
    }

    /// Clone the executor for use in spawned tasks
    fn clone_executor(&self) -> MapReduceExecutor {
        MapReduceExecutor {
            claude_executor: self.claude_executor.clone(),
            session_manager: self.session_manager.clone(),
            user_interaction: self.user_interaction.clone(),
            worktree_manager: self.worktree_manager.clone(),
            project_root: self.project_root.clone(),
            interpolation_engine: self.interpolation_engine.clone(),
            command_registry: self.command_registry.clone(),
            subprocess: self.subprocess.clone(),
            state_manager: self.state_manager.clone(),
            event_logger: self.event_logger.clone(),
            dlq: self.dlq.clone(),
            correlation_id: self.correlation_id.clone(),
        }
    }

    /// Extract a meaningful identifier from a JSON work item
    fn extract_item_identifier(item: &Value, index: usize) -> String {
        // Priority order for identifier fields
        let id_fields = [
            "id",
            "name",
            "title",
            "path",
            "file",
            "key",
            "label",
            "identifier",
        ];

        if let Value::Object(obj) = item {
            for field in &id_fields {
                if let Some(value) = obj.get(*field) {
                    match value {
                        Value::String(s) => {
                            return Self::truncate_identifier(s, 30);
                        }
                        Value::Number(n) => {
                            return n.to_string();
                        }
                        _ => continue,
                    }
                }
            }
        }

        // Fallback to index
        format!("item_{}", index)
    }

    fn truncate_identifier(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }

    /// Validate that all required variables are available for reduce commands
    fn validate_reduce_variables(
        &self,
        commands: &[WorkflowStep],
        context: &AgentContext,
    ) -> MapReduceResult<()> {
        let mut all_missing_vars = Vec::new();

        for (step_index, step) in commands.iter().enumerate() {
            let step_name = step.name.as_deref().unwrap_or("unnamed");

            // Check shell commands for variable references
            if let Some(shell_cmd) = &step.shell {
                let missing_vars = self.find_missing_variables(shell_cmd, &context.variables);
                if !missing_vars.is_empty() {
                    warn!(
                        "Reduce step {} ('{}') references missing variables: {:?}\n  Command: {}",
                        step_index + 1,
                        step_name,
                        missing_vars,
                        shell_cmd
                    );
                    all_missing_vars.extend(missing_vars);
                }
            }

            // Check Claude commands for variable references (these get interpolated)
            if let Some(claude_cmd) = &step.claude {
                let missing_vars = self.find_missing_variables(claude_cmd, &context.variables);
                if !missing_vars.is_empty() {
                    warn!(
                        "Reduce step {} ('{}') references missing variables: {:?}\n  Command: {}",
                        step_index + 1,
                        step_name,
                        missing_vars,
                        claude_cmd
                    );
                    all_missing_vars.extend(missing_vars);
                }
            }

            // Check legacy commands
            if let Some(command) = &step.command {
                let missing_vars = self.find_missing_variables(command, &context.variables);
                if !missing_vars.is_empty() {
                    warn!(
                        "Reduce step {} ('{}') references missing variables: {:?}\n  Command: {}",
                        step_index + 1,
                        step_name,
                        missing_vars,
                        command
                    );
                    all_missing_vars.extend(missing_vars);
                }
            }
        }

        // Log available variables for debugging
        debug!(
            "Available variables for reduce phase: {:?}",
            context.variables.keys().collect::<Vec<_>>()
        );

        // For now, just warn about missing variables rather than failing
        // This allows workflows to continue even if some variables might be missing
        // In the future, we could make this configurable via workflow settings
        if !all_missing_vars.is_empty() {
            // Remove duplicates
            all_missing_vars.sort();
            all_missing_vars.dedup();

            warn!(
                "⚠️  Reduce phase validation found potentially missing variables: {:?}\n  \
                Available variables: {:?}\n  \
                Commands will still execute but may fail at runtime.",
                all_missing_vars,
                context.variables.keys().collect::<Vec<_>>()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::TestCommand;
    use crate::cook::workflow::{CaptureOutput, HandlerStep};

    #[test]
    fn test_collect_command_types_claude() {
        let step = WorkflowStep {
            claude: Some("test command".to_string()),
            shell: None,
            test: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], CommandType::Claude(cmd) if cmd == "test command");
    }

    #[test]
    fn test_collect_command_types_shell() {
        let step = WorkflowStep {
            claude: None,
            shell: Some("echo test".to_string()),
            test: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], CommandType::Shell(cmd) if cmd == "echo test");
    }

    #[test]
    fn test_collect_command_types_test() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: Some(TestCommand {
                command: "cargo test".to_string(),
                on_failure: None,
            }),
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], CommandType::Test(cmd) if cmd.command == "cargo test");
    }

    #[test]
    fn test_collect_command_types_handler() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            handler: Some(HandlerStep {
                name: "test_handler".to_string(),
                attributes: HashMap::new(),
            }),
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], CommandType::Handler { handler_name, .. } if handler_name == "test_handler");
    }

    #[test]
    fn test_collect_command_types_legacy_name() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            handler: None,
            name: Some("test_command".to_string()),
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        matches!(&commands[0], CommandType::Legacy(cmd) if cmd == "/test_command");
    }

    #[test]
    fn test_collect_command_types_legacy_name_with_slash() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            handler: None,
            name: Some("/test_command".to_string()),
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 1);
        if let CommandType::Legacy(cmd) = &commands[0] {
            assert_eq!(cmd, "/test_command");
        } else {
            panic!("Expected Legacy command type");
        }
    }

    #[test]
    fn test_collect_command_types_empty() {
        let step = WorkflowStep {
            claude: None,
            shell: None,
            test: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 0);
    }

    #[test]
    fn test_collect_command_types_multiple() {
        // This tests that the collection function returns all specified commands
        // The validation happens in validate_command_count
        let step = WorkflowStep {
            claude: Some("claude cmd".to_string()),
            shell: Some("shell cmd".to_string()),
            test: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
        };

        let commands = MapReduceExecutor::collect_command_types(&step);

        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_format_legacy_command() {
        assert_eq!(MapReduceExecutor::format_legacy_command("test"), "/test");
        assert_eq!(MapReduceExecutor::format_legacy_command("/test"), "/test");
        assert_eq!(
            MapReduceExecutor::format_legacy_command("/already/slash"),
            "/already/slash"
        );
    }

    #[test]
    fn test_truncate_identifier() {
        assert_eq!(MapReduceExecutor::truncate_identifier("short", 10), "short");
        assert_eq!(
            MapReduceExecutor::truncate_identifier("this is a very long identifier", 10),
            "this is..."
        );
        assert_eq!(
            MapReduceExecutor::truncate_identifier("exactly_ten", 11),
            "exactly_ten"
        );
        assert_eq!(
            MapReduceExecutor::truncate_identifier("exactly_eleven_", 11),
            "exactly_..."
        );
    }
}
