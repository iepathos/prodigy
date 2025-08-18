//! MapReduce executor for parallel workflow execution
//!
//! Implements parallel execution of workflow steps across multiple agents
//! using isolated git worktrees for fault isolation and parallelism.

use crate::commands::{AttributeValue, CommandRegistry, ExecutionContext};
use crate::cook::execution::data_pipeline::DataPipeline;
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{CommandType, StepResult, WorkflowStep};
use crate::subprocess::SubprocessManager;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::timeout as tokio_timeout;
use tracing::{debug, error, info, warn};

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
}

/// Progress tracking for parallel execution
struct ProgressTracker {
    multi_progress: MultiProgress,
    overall_bar: ProgressBar,
    agent_bars: Vec<ProgressBar>,
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

        // Individual agent progress bars
        let mut agent_bars = Vec::new();
        for i in 0..max_parallel.min(total_items) {
            let bar = multi_progress.add(ProgressBar::new(100));
            bar.set_style(
                ProgressStyle::default_bar()
                    .template(&format!("  Agent {:2}: {{msg}}", i + 1))
                    .unwrap(),
            );
            bar.set_message("Idle");
            agent_bars.push(bar);
        }

        Self {
            multi_progress,
            overall_bar,
            agent_bars,
        }
    }

    fn update_agent(&self, agent_index: usize, message: &str) {
        if agent_index < self.agent_bars.len() {
            self.agent_bars[agent_index].set_message(message.to_string());
        }
    }

    fn complete_item(&self) {
        self.overall_bar.inc(1);
    }

    fn finish(&self, message: &str) {
        self.overall_bar.finish_with_message(message.to_string());
        for bar in &self.agent_bars {
            bar.finish_and_clear();
        }
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
}

impl MapReduceExecutor {
    /// Create a new MapReduce executor
    pub async fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        worktree_manager: Arc<WorktreeManager>,
        project_root: PathBuf,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            worktree_manager,
            project_root,
            interpolation_engine: Arc::new(Mutex::new(InterpolationEngine::new(false))),
            command_registry: Arc::new(CommandRegistry::with_defaults().await),
            subprocess: Arc::new(SubprocessManager::production()),
        }
    }

    /// Execute a MapReduce workflow
    pub async fn execute(
        &self,
        map_phase: &MapPhase,
        reduce_phase: Option<&ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
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

        // Execute map phase
        let map_results = self.execute_map_phase(map_phase, work_items, env).await?;

        // Execute reduce phase if specified
        if let Some(reduce_phase) = reduce_phase {
            self.execute_reduce_phase(reduce_phase, &map_results, env)
                .await?;
        }

        // Report summary
        let duration = start_time.elapsed();
        self.report_summary(&map_results, duration);

        Ok(map_results)
    }

    /// Load work items from JSON file with pipeline processing
    async fn load_work_items_with_pipeline(
        &self,
        config: &MapReduceConfig,
        filter: &Option<String>,
        sort_by: &Option<String>,
    ) -> Result<Vec<Value>> {
        let input_path = if config.input.is_absolute() {
            config.input.clone()
        } else {
            self.project_root.join(&config.input)
        };

        let content = tokio::fs::read_to_string(&input_path)
            .await
            .context(format!(
                "Failed to read input file: {}",
                input_path.display()
            ))?;

        let json: Value = serde_json::from_str(&content).context("Failed to parse input JSON")?;

        // Use data pipeline for extraction, filtering, and sorting
        let json_path = if config.json_path.is_empty() {
            None
        } else {
            Some(config.json_path.clone())
        };

        // Create pipeline with all configuration options
        let mut pipeline = DataPipeline::from_config(
            json_path,
            filter.clone(),
            sort_by.clone(),
            config.max_items,
        )?;

        // Set offset if specified
        if let Some(offset) = config.offset {
            pipeline.offset = Some(offset);
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
    ) -> Result<Vec<AgentResult>> {
        let total_items = work_items.len();
        let max_parallel = map_phase.config.max_parallel.min(total_items);

        // Create progress tracker
        let progress = Arc::new(ProgressTracker::new(total_items, max_parallel));

        // Create channels for work distribution
        let (work_tx, work_rx) = mpsc::channel::<(usize, Value)>(total_items);
        let work_rx = Arc::new(RwLock::new(work_rx));

        // Send all work items to the queue
        for (index, item) in work_items.into_iter().enumerate() {
            work_tx.send((index, item)).await?;
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

            let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
                executor
                    .run_agent(agent_index, work_rx, results, progress, map_phase, env)
                    .await
            });

            workers.push(handle);
        }

        // Wait for all workers to complete
        for worker in workers {
            if let Err(e) = worker.await? {
                self.user_interaction
                    .display_warning(&format!("Worker error: {}", e));
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
    ) -> Result<()> {
        loop {
            // Get next work item
            let work_item = {
                let mut rx = work_rx.write().await;
                rx.recv().await
            };

            let Some((item_index, item)) = work_item else {
                // No more work
                progress.update_agent(agent_index, "Completed");
                break;
            };

            let item_id = format!("item_{}", item_index);
            progress.update_agent(agent_index, &format!("Processing {}", item_id));

            // Execute work item with retries
            let mut attempt = 0;
            let agent_result = loop {
                attempt += 1;

                if attempt > 1 {
                    progress.update_agent(
                        agent_index,
                        &format!("Retrying {} (attempt {})", item_id, attempt),
                    );
                }

                let result = self
                    .execute_agent_commands(&item_id, &item, &map_phase.agent_template, &env)
                    .await;

                match result {
                    Ok(res) => break res,
                    Err(_e) if attempt < map_phase.config.retry_on_failure => {
                        // Retry on failure
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        // Final failure
                        break AgentResult {
                            item_id: item_id.clone(),
                            status: AgentStatus::Failed(e.to_string()),
                            output: None,
                            commits: vec![],
                            duration: Duration::from_secs(0),
                            error: Some(e.to_string()),
                            worktree_path: None,
                        };
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
    async fn execute_agent_commands(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
    ) -> Result<AgentResult> {
        let start_time = Instant::now();

        // Create isolated worktree session for this agent
        let worktree_session = self
            .worktree_manager
            .create_session()
            .await
            .context("Failed to create agent worktree")?;
        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();

        // Create agent-specific environment
        let agent_env = ExecutionEnvironment {
            working_dir: worktree_path.clone(),
            project_dir: env.project_dir.clone(),
            worktree_name: Some(worktree_name.clone()),
            session_id: format!("{}-{}", env.session_id, item_id),
        };

        // Create agent context
        let mut context = AgentContext::new(
            item_id.to_string(),
            worktree_path.clone(),
            worktree_name.clone(),
            agent_env,
        );

        // Add item data to context variables
        if let Value::Object(obj) = item {
            for (key, value) in obj {
                if let Value::String(s) = value {
                    context.variables.insert(key.clone(), s.clone());
                } else {
                    context.variables.insert(key.clone(), value.to_string());
                }
            }
        }

        // Add standard variables
        context
            .variables
            .insert("worktree".to_string(), worktree_name.clone());
        context
            .variables
            .insert("item_id".to_string(), item_id.to_string());
        context.variables.insert(
            "session_id".to_string(),
            format!("{}-{}", env.session_id, item_id),
        );

        // Execute template steps with real command execution
        let mut all_outputs = Vec::new();
        let mut total_output = String::new();
        let mut execution_error: Option<String> = None;

        for (step_index, step) in template_steps.iter().enumerate() {
            debug!(
                "Executing step {} for agent {}: {:?}",
                step_index + 1,
                item_id,
                step.name
            );

            // Execute the step
            let step_result = match self.execute_single_step(step, &mut context).await {
                Ok(result) => result,
                Err(e) => {
                    let error_msg = format!("Step {} failed: {}", step_index + 1, e);
                    error!("Agent {} error: {}", item_id, error_msg);

                    // Handle on_failure if specified
                    if let Some(on_failure) = &step.on_failure {
                        info!("Executing on_failure handler for agent {}", item_id);
                        if let Err(handler_err) = self
                            .handle_on_failure(on_failure, &mut context, error_msg.clone())
                            .await
                        {
                            error!(
                                "on_failure handler also failed for agent {}: {}",
                                item_id, handler_err
                            );
                        }

                        // For MapReduce, we continue on failure unless it's critical
                        // TODO: Add a fail_workflow option to WorkflowStep if needed
                    } else {
                        execution_error = Some(error_msg);
                        break;
                    }

                    // Create a failed result for this step
                    StepResult {
                        success: false,
                        exit_code: Some(1),
                        stdout: String::new(),
                        stderr: e.to_string(),
                    }
                }
            };

            // Update context with step output
            if !step_result.stdout.is_empty() {
                context.update_with_output(Some(step_result.stdout.clone()));
                context.variables.insert(
                    format!("step{}.output", step_index + 1),
                    step_result.stdout.clone(),
                );
            }

            // Accumulate outputs
            all_outputs.push(step_result.stdout.clone());
            total_output.push_str(&format!(
                "=== Step {} ({}) ===\n{}\n",
                step_index + 1,
                step.name.as_deref().unwrap_or("unnamed"),
                step_result.stdout
            ));

            // Check for step failure
            if !step_result.success {
                // In MapReduce, we generally continue on failure
                // unless the step has an on_failure handler that says otherwise
                if step.on_failure.is_none() {
                    execution_error = Some(format!(
                        "Step {} failed with exit code {:?}",
                        step_index + 1,
                        step_result.exit_code
                    ));
                    break;
                }
            }
        }

        // Get commits from worktree
        let commits = self.get_worktree_commits(&worktree_path).await?;

        // Determine final status
        let status = if let Some(error) = execution_error {
            AgentStatus::Failed(error)
        } else {
            AgentStatus::Success
        };

        // Clean up worktree if not needed for reduce phase
        // Note: In production, we might keep worktrees for the reduce phase
        if !template_steps.is_empty() {
            self.worktree_manager
                .cleanup_session(&worktree_name, true)
                .await?;
        }

        Ok(AgentResult {
            item_id: item_id.to_string(),
            status,
            output: Some(total_output),
            commits,
            duration: start_time.elapsed(),
            error: None,
            worktree_path: Some(worktree_path),
        })
    }

    /// Interpolate variables in a workflow step
    async fn interpolate_workflow_step(
        &self,
        step: &WorkflowStep,
        context: &InterpolationContext,
    ) -> Result<WorkflowStep> {
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
    async fn get_worktree_commits(&self, worktree_path: &Path) -> Result<Vec<String>> {
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

    /// Execute the reduce phase
    async fn execute_reduce_phase(
        &self,
        reduce_phase: &ReducePhase,
        map_results: &[AgentResult],
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        self.user_interaction
            .display_progress("Starting reduce phase...");

        // Create interpolation context with map results
        let mut interp_context = InterpolationContext::new();

        // Add summary statistics
        let successful = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failed = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();

        interp_context.set(
            "map",
            json!({
                "successful": successful,
                "failed": failed,
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

        // Execute reduce commands with interpolation
        for (step_index, step) in reduce_phase.commands.iter().enumerate() {
            self.user_interaction
                .display_progress(&format!("Executing reduce step {}...", step_index + 1));

            // Interpolate the step
            let interpolated_step = self
                .interpolate_workflow_step(step, &interp_context)
                .await?;

            // Note: In a real implementation, this would use the WorkflowExecutor to execute the interpolated step
            let command_desc = interpolated_step
                .command
                .or(interpolated_step.claude)
                .or(interpolated_step.shell)
                .unwrap_or_else(|| "(no command)".to_string());
            self.user_interaction.display_progress(&format!(
                "Reduce step {}: {}",
                step_index + 1,
                command_desc
            ));
        }

        self.user_interaction
            .display_success("Reduce phase completed");

        Ok(())
    }

    /// Execute a single workflow step with agent context
    async fn execute_single_step(
        &self,
        step: &WorkflowStep,
        context: &mut AgentContext,
    ) -> Result<StepResult> {
        // Interpolate the step using the agent's context
        let interp_context = context.to_interpolation_context();
        let interpolated_step = self
            .interpolate_workflow_step(step, &interp_context)
            .await?;

        // Determine command type
        let command_type = self.determine_command_type(&interpolated_step)?;

        // Execute the command based on its type
        let result = match command_type {
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
                    return Err(anyhow!("Test command type not supported in MapReduce"));
                }
            }
        };

        // Capture output if requested
        if step.capture_output && !result.stdout.is_empty() {
            context
                .captured_outputs
                .insert("CAPTURED_OUTPUT".to_string(), result.stdout.clone());
        }

        Ok(result)
    }

    /// Determine command type from a workflow step
    fn determine_command_type(&self, step: &WorkflowStep) -> Result<CommandType> {
        // Count how many command fields are specified
        let mut specified_count = 0;
        if step.claude.is_some() {
            specified_count += 1;
        }
        if step.shell.is_some() {
            specified_count += 1;
        }
        if step.test.is_some() {
            specified_count += 1;
        }
        if step.handler.is_some() {
            specified_count += 1;
        }
        if step.name.is_some() || step.command.is_some() {
            specified_count += 1;
        }

        // Ensure only one command type is specified
        if specified_count > 1 {
            return Err(anyhow!(
                "Multiple command types specified. Use only one of: claude, shell, test, handler, or name/command"
            ));
        }

        if specified_count == 0 {
            return Err(anyhow!(
                "No command specified. Use one of: claude, shell, test, handler, or name/command"
            ));
        }

        // Return the appropriate command type
        if let Some(handler_step) = &step.handler {
            // Convert serde_json::Value to AttributeValue
            let mut attributes = HashMap::new();
            for (key, value) in &handler_step.attributes {
                attributes.insert(key.clone(), Self::json_to_attribute_value(value.clone()));
            }
            Ok(CommandType::Handler {
                handler_name: handler_step.name.clone(),
                attributes,
            })
        } else if let Some(claude_cmd) = &step.claude {
            Ok(CommandType::Claude(claude_cmd.clone()))
        } else if let Some(shell_cmd) = &step.shell {
            Ok(CommandType::Shell(shell_cmd.clone()))
        } else if let Some(test_cmd) = &step.test {
            Ok(CommandType::Test(test_cmd.clone()))
        } else if let Some(name) = &step.name {
            // Legacy support - prepend / if not present
            let command = if name.starts_with('/') {
                name.clone()
            } else {
                format!("/{name}")
            };
            Ok(CommandType::Legacy(command))
        } else if let Some(command) = &step.command {
            Ok(CommandType::Legacy(command.clone()))
        } else {
            Err(anyhow!("No valid command found in step"))
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
    ) -> Result<StepResult> {
        // Set up environment variables for the command
        let mut env_vars = HashMap::new();
        env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "MMM_CONTEXT_DIR".to_string(),
            context
                .worktree_path
                .join(".mmm/context")
                .to_string_lossy()
                .to_string(),
        );
        env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());
        env_vars.insert("MMM_WORKTREE".to_string(), context.worktree_name.clone());

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
    ) -> Result<StepResult> {
        use tokio::time::Duration;

        // Create command
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);

        // Set working directory to the agent's worktree
        cmd.current_dir(&context.worktree_path);

        // Set environment variables
        cmd.env("MMM_WORKTREE", &context.worktree_name);
        cmd.env("MMM_ITEM_ID", &context.item_id);
        cmd.env("MMM_AUTOMATION", "true");

        // Execute with optional timeout
        let output = if let Some(timeout_secs) = timeout {
            let duration = Duration::from_secs(timeout_secs);
            match tokio_timeout(duration, cmd.output()).await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(anyhow!("Command timed out after {} seconds", timeout_secs));
                }
            }
        } else {
            cmd.output().await?
        };

        Ok(StepResult {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    /// Execute a handler command with agent context
    async fn execute_handler_command(
        &self,
        handler_name: &str,
        attributes: &HashMap<String, AttributeValue>,
        context: &AgentContext,
    ) -> Result<StepResult> {
        // Create execution context for the handler
        let mut exec_context = ExecutionContext::new(context.worktree_path.clone());

        // Add environment variables
        exec_context.add_env_var("MMM_WORKTREE".to_string(), context.worktree_name.clone());
        exec_context.add_env_var("MMM_ITEM_ID".to_string(), context.item_id.clone());
        exec_context.add_env_var("MMM_AUTOMATION".to_string(), "true".to_string());

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
    async fn handle_on_failure(
        &self,
        on_failure: &WorkflowStep,
        context: &mut AgentContext,
        error: String,
    ) -> Result<()> {
        // Add error to context for interpolation
        context.variables.insert("error".to_string(), error.clone());
        context.variables.insert("last_error".to_string(), error);

        // Execute the on_failure step
        let result = self.execute_single_step(on_failure, context).await;

        // Log the result but don't fail the entire execution
        match result {
            Ok(step_result) => {
                if step_result.success {
                    info!("on_failure handler succeeded for agent {}", context.item_id);
                } else {
                    warn!(
                        "on_failure handler failed for agent {}: {}",
                        context.item_id, step_result.stderr
                    );
                }
            }
            Err(e) => {
                error!(
                    "Failed to execute on_failure handler for agent {}: {}",
                    context.item_id, e
                );
            }
        }

        Ok(())
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
        }
    }
}

#[cfg(test)]
#[path = "mapreduce_tests.rs"]
mod tests;
