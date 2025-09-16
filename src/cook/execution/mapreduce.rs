//! MapReduce executor for parallel workflow execution
//!
//! Implements parallel execution of workflow steps across multiple agents
//! using isolated git worktrees for fault isolation and parallelism.

use crate::commands::CommandRegistry;
use crate::cook::execution::dlq::{DeadLetterQueue, DeadLetteredItem, ErrorType, FailureDetail};
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::execution::events::EventLogger;
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::progress::EnhancedProgressTracker;
use crate::cook::execution::progress_tracker::ProgressTracker as NewProgressTracker;
use crate::cook::execution::state::JobStateManager;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{CommandType, StepResult, WorkflowStep};
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreePool};
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
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::timeout as tokio_timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceConfig {
    /// Input source: either a file path or command to execute
    pub input: String,
    /// JSON path expression to extract work items (for JSON files)
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

/// Setup phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhase {
    /// Commands to execute during setup
    pub commands: Vec<WorkflowStep>,
    /// Timeout for the entire setup phase (in seconds)
    pub timeout: u64,
    /// Variables to capture from setup commands
    /// Key is variable name, value is the command index to capture from
    #[serde(default)]
    pub capture_outputs: HashMap<String, usize>,
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
    /// Optional distinct field for deduplication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct: Option<String>,
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
    /// Files modified by the agent
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_modified: Vec<String>,
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
    overall_bar: ProgressBar,
    agent_bars: Vec<ProgressBar>,
    tick_handle: Option<JoinHandle<()>>,
    is_finished: Arc<AtomicBool>,
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
            overall_bar,
            agent_bars,
            tick_handle: None,
            is_finished: Arc::new(AtomicBool::new(false)),
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
    /// Variable store for structured capture data
    pub variable_store: crate::cook::workflow::variables::VariableStore,
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
            variable_store: crate::cook::workflow::variables::VariableStore::new(),
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
    worktree_pool: Option<Arc<WorktreePool>>,
    project_root: PathBuf,
    interpolation_engine: Arc<Mutex<InterpolationEngine>>,
    command_registry: Arc<CommandRegistry>,
    subprocess: Arc<SubprocessManager>,
    state_manager: Arc<dyn JobStateManager>,
    event_logger: Arc<EventLogger>,
    dlq: Option<Arc<DeadLetterQueue>>,
    correlation_id: String,
    enhanced_progress_tracker: Option<Arc<EnhancedProgressTracker>>,
    new_progress_tracker: Option<Arc<NewProgressTracker>>,
    enable_web_dashboard: bool,
    setup_variables: HashMap<String, String>,
}

/// Summary statistics for map results
#[derive(Debug)]
struct MapResultSummary {
    successful: usize,
    failed: usize,
    total: usize,
}

/// Calculate summary statistics from map results (pure function)
fn calculate_map_result_summary(map_results: &[AgentResult]) -> MapResultSummary {
    let successful = map_results
        .iter()
        .filter(|r| matches!(r.status, AgentStatus::Success))
        .count();

    let failed = map_results
        .iter()
        .filter(|r| matches!(r.status, AgentStatus::Failed(_) | AgentStatus::Timeout))
        .count();

    MapResultSummary {
        successful,
        failed,
        total: map_results.len(),
    }
}

/// Build InterpolationContext with map results (pure function)
fn build_map_results_interpolation_context(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> Result<InterpolationContext, serde_json::Error> {
    let mut context = InterpolationContext::new();

    // Add summary statistics
    context.set(
        "map",
        json!({
            "successful": summary.successful,
            "failed": summary.failed,
            "total": summary.total
        }),
    );

    // Add complete results as JSON value
    let results_value = serde_json::to_value(map_results)?;
    context.set("map.results", results_value);

    // Add individual result access
    for (index, result) in map_results.iter().enumerate() {
        let result_value = serde_json::to_value(result)?;
        context.set(format!("results[{}]", index), result_value);
    }

    Ok(context)
}

/// Build AgentContext variables for shell commands (pure function)
fn build_agent_context_variables(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> Result<HashMap<String, String>, serde_json::Error> {
    let mut variables = HashMap::new();

    // Add summary statistics as strings for shell command substitution
    variables.insert("map.successful".to_string(), summary.successful.to_string());
    variables.insert("map.failed".to_string(), summary.failed.to_string());
    variables.insert("map.total".to_string(), summary.total.to_string());

    // Add complete results as JSON string for complex access patterns
    let results_json = serde_json::to_string(map_results)?;
    variables.insert("map.results_json".to_string(), results_json.clone());
    variables.insert("map.results".to_string(), results_json);

    // Add individual result summaries for easier access in shell commands
    for (index, result) in map_results.iter().enumerate() {
        add_individual_result_variables(&mut variables, index, result);
    }

    Ok(variables)
}

/// Generate agent ID from index and item ID (pure function)
fn generate_agent_id(agent_index: usize, item_id: &str) -> String {
    format!("agent-{}-{}", agent_index, item_id)
}

/// Generate branch name for an agent (pure function)
fn generate_agent_branch_name(session_id: &str, item_id: &str) -> String {
    format!("prodigy-agent-{}-{}", session_id, item_id)
}

/// Classify agent status for event logging (pure function)
#[cfg(test)]
fn classify_agent_status(status: &AgentStatus) -> AgentEventType {
    match status {
        AgentStatus::Success => AgentEventType::Completed,
        AgentStatus::Failed(_) => AgentEventType::Failed,
        AgentStatus::Timeout => AgentEventType::TimedOut,
        AgentStatus::Retrying(_) => AgentEventType::Retrying,
        _ => AgentEventType::InProgress,
    }
}

/// Enum for agent event types
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
enum AgentEventType {
    Completed,
    Failed,
    TimedOut,
    Retrying,
    InProgress,
}

// ============================================================================
// Error Handling Pipeline Functions
// ============================================================================

/// Convert worktree errors to MapReduceError with proper context
fn handle_worktree_error(
    agent_id: &str,
    operation: &str,
    error: impl std::fmt::Display,
) -> MapReduceError {
    MapReduceError::WorktreeCreationFailed {
        agent_id: agent_id.to_string(),
        reason: format!("{} failed: {}", operation, error),
        source: std::io::Error::other(error.to_string()),
    }
}

/// Convert command execution errors to MapReduceError
fn handle_command_error(
    _job_id: &str,
    command: &str,
    error: impl std::fmt::Display,
) -> MapReduceError {
    MapReduceError::CommandExecutionFailed {
        command: command.to_string(),
        reason: error.to_string(),
        source: None,
    }
}

/// Convert generic errors to MapReduceError with context
fn handle_generic_error(
    operation: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> MapReduceError {
    MapReduceError::General {
        message: format!("{} failed", operation),
        source: Some(Box::new(error)),
    }
}

/// Create a DLQ item from failed agent result
fn create_dlq_item(
    item_id: &str,
    item: &Value,
    error: &str,
    _agent_id: &str,
    attempt: u32,
) -> DeadLetteredItem {
    let now = Utc::now();
    DeadLetteredItem {
        item_id: item_id.to_string(),
        item_data: item.clone(),
        first_attempt: now,
        last_attempt: now,
        failure_count: attempt,
        failure_history: vec![FailureDetail {
            attempt_number: attempt,
            timestamp: now,
            error_type: ErrorType::CommandFailed { exit_code: 1 },
            error_message: error.to_string(),
            stack_trace: None,
            agent_id: _agent_id.to_string(),
            step_failed: "agent_execution".to_string(),
            duration_ms: 0,
        }],
        error_signature: format!("agent_failed_{}", item_id),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    }
}

// ============================================================================
// Validation Pipeline Functions
// ============================================================================

/// Validate reduce phase can proceed based on map results
fn validate_reduce_phase(map_results: &[AgentResult]) -> Result<(), String> {
    if map_results.is_empty() {
        return Err("No items were processed in map phase".to_string());
    }

    let successful_count = map_results
        .iter()
        .filter(|r| matches!(r.status, AgentStatus::Success))
        .count();

    if successful_count == 0 {
        return Err("All map agents failed".to_string());
    }

    Ok(())
}

/// Validate workflow step before execution
fn validate_workflow_step(step: &WorkflowStep) -> Result<(), String> {
    // Simplified validation - just check if step has some command configured
    if step.command.is_none()
        && step.claude.is_none()
        && step.shell.is_none()
        && step.test.is_none()
        && step.handler.is_none()
    {
        return Err("Step must have at least one command configured".to_string());
    }

    Ok(())
}

/// Validate map phase configuration
fn validate_map_phase(config: &MapReduceConfig) -> Result<(), String> {
    if config.max_parallel == 0 {
        return Err("max_parallel must be greater than 0".to_string());
    }

    if config.timeout_per_agent == 0 {
        return Err("timeout_per_agent must be greater than 0".to_string());
    }

    if let Some(max_items) = config.max_items {
        if max_items == 0 {
            return Err("max_items must be greater than 0 if specified".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod pure_function_tests {
    use super::*;
    use serde_json::Value;
    use std::time::Duration;

    /// Helper function to create test AgentResult
    fn create_test_agent_result(
        item_id: &str,
        status: AgentStatus,
        output: Option<String>,
        commits: Vec<String>,
    ) -> AgentResult {
        AgentResult {
            item_id: item_id.to_string(),
            status,
            output,
            commits,
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        }
    }

    /// Test calculate_map_result_summary with mixed results
    #[test]
    fn test_calculate_map_result_summary_mixed_results() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("success output".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Failed("error".to_string()),
                Some("error output".to_string()),
                vec![],
            ),
            create_test_agent_result(
                "item3",
                AgentStatus::Success,
                Some("success output 2".to_string()),
                vec!["commit2".to_string(), "commit3".to_string()],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.total, 3);
    }

    /// Test calculate_map_result_summary with all successful results
    #[test]
    fn test_calculate_map_result_summary_all_successful() {
        let map_results = vec![
            create_test_agent_result(
                "item1",
                AgentStatus::Success,
                Some("success".to_string()),
                vec!["commit1".to_string()],
            ),
            create_test_agent_result(
                "item2",
                AgentStatus::Success,
                Some("success".to_string()),
                vec!["commit2".to_string()],
            ),
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total, 2);
    }

    /// Test calculate_map_result_summary with all failed results
    #[test]
    fn test_calculate_map_result_summary_all_failed() {
        let map_results = vec![
            AgentResult {
                item_id: "item1".to_string(),
                status: AgentStatus::Failed("error1".to_string()),
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: Some("error1".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item2".to_string(),
                status: AgentStatus::Timeout,
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
        ];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 0);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.total, 2);
    }

    /// Test calculate_map_result_summary with empty results
    #[test]
    fn test_calculate_map_result_summary_empty_results() {
        let map_results = vec![];

        let summary = calculate_map_result_summary(&map_results);

        assert_eq!(summary.successful, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total, 0);
    }

    /// Test generate_agent_id pure function
    #[test]
    fn test_generate_agent_id() {
        assert_eq!(generate_agent_id(0, "item-1"), "agent-0-item-1");
        assert_eq!(generate_agent_id(5, "test-item"), "agent-5-test-item");
        assert_eq!(generate_agent_id(100, "special"), "agent-100-special");
    }

    /// Test generate_agent_branch_name pure function
    #[test]
    fn test_generate_agent_branch_name() {
        assert_eq!(
            generate_agent_branch_name("session-123", "item-1"),
            "prodigy-agent-session-123-item-1"
        );
        assert_eq!(
            generate_agent_branch_name("test-session", "special-item"),
            "prodigy-agent-test-session-special-item"
        );
    }

    /// Test classify_agent_status pure function
    #[test]
    fn test_classify_agent_status() {
        assert_eq!(
            classify_agent_status(&AgentStatus::Success),
            AgentEventType::Completed
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Failed("error".to_string())),
            AgentEventType::Failed
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Timeout),
            AgentEventType::TimedOut
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Retrying(1)),
            AgentEventType::Retrying
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Pending),
            AgentEventType::InProgress
        );
        assert_eq!(
            classify_agent_status(&AgentStatus::Running),
            AgentEventType::InProgress
        );
    }

    /// Test build_map_results_interpolation_context
    #[test]
    fn test_build_map_results_interpolation_context() {
        let map_results = vec![
            AgentResult {
                item_id: "item1".to_string(),
                status: AgentStatus::Success,
                output: Some("success".to_string()),
                commits: vec!["commit1".to_string()],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item2".to_string(),
                status: AgentStatus::Failed("error".to_string()),
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: Some("error".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
        ];

        let summary = MapResultSummary {
            successful: 1,
            failed: 1,
            total: 2,
        };

        let context = build_map_results_interpolation_context(&map_results, &summary).unwrap();

        // Test that map object is properly structured
        let map_value = context.resolve_path(&["map".to_string()]).unwrap();

        if let Value::Object(map_obj) = map_value {
            assert_eq!(map_obj.get("successful").unwrap().as_u64().unwrap(), 1);
            assert_eq!(map_obj.get("failed").unwrap().as_u64().unwrap(), 1);
            assert_eq!(map_obj.get("total").unwrap().as_u64().unwrap(), 2);
        } else {
            panic!("Expected map to be an object");
        }

        // Test that individual paths resolve correctly
        assert_eq!(
            context
                .resolve_path(&["map".to_string(), "successful".to_string()])
                .unwrap(),
            Value::Number(1.into())
        );
        assert_eq!(
            context
                .resolve_path(&["map".to_string(), "failed".to_string()])
                .unwrap(),
            Value::Number(1.into())
        );
        assert_eq!(
            context
                .resolve_path(&["map".to_string(), "total".to_string()])
                .unwrap(),
            Value::Number(2.into())
        );

        // Test that map.results contains the full results
        let results_value = context.resolve_path(&["map.results".to_string()]).unwrap();
        assert!(results_value.is_array());
    }

    /// Test build_agent_context_variables
    #[test]
    fn test_build_agent_context_variables() {
        let map_results = vec![AgentResult {
            item_id: "test_item".to_string(),
            status: AgentStatus::Success,
            output: Some("output data".to_string()),
            commits: vec!["abc123".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        }];

        let summary = MapResultSummary {
            successful: 1,
            failed: 0,
            total: 1,
        };

        let variables = build_agent_context_variables(&map_results, &summary).unwrap();

        // Test summary statistics
        assert_eq!(variables.get("map.successful").unwrap(), "1");
        assert_eq!(variables.get("map.failed").unwrap(), "0");
        assert_eq!(variables.get("map.total").unwrap(), "1");

        // Test that results are present as JSON
        assert!(variables.contains_key("map.results"));
        assert!(variables.contains_key("map.results_json"));

        // Test individual result variables
        assert_eq!(variables.get("result.0.item_id").unwrap(), "test_item");
        assert_eq!(variables.get("result.0.status").unwrap(), "success");
        assert_eq!(variables.get("result.0.output").unwrap(), "output data");
        assert_eq!(variables.get("result.0.commits").unwrap(), "1");
    }

    /// Test add_individual_result_variables with different statuses
    #[test]
    fn test_add_individual_result_variables_various_statuses() {
        let mut variables = HashMap::new();

        // Test success result
        let success_result = AgentResult {
            item_id: "success_item".to_string(),
            status: AgentStatus::Success,
            output: Some("success output".to_string()),
            commits: vec!["commit1".to_string(), "commit2".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        add_individual_result_variables(&mut variables, 0, &success_result);
        assert_eq!(variables.get("result.0.item_id").unwrap(), "success_item");
        assert_eq!(variables.get("result.0.status").unwrap(), "success");
        assert_eq!(variables.get("result.0.output").unwrap(), "success output");
        assert_eq!(variables.get("result.0.commits").unwrap(), "2");

        // Test failed result
        let failed_result = AgentResult {
            item_id: "failed_item".to_string(),
            status: AgentStatus::Failed("test error".to_string()),
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: Some("test error".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        add_individual_result_variables(&mut variables, 1, &failed_result);
        assert_eq!(variables.get("result.1.item_id").unwrap(), "failed_item");
        assert_eq!(
            variables.get("result.1.status").unwrap(),
            "failed: test error"
        );
        assert!(!variables.contains_key("result.1.output")); // No output for failed
        assert_eq!(variables.get("result.1.commits").unwrap(), "0");

        // Test timeout result
        let timeout_result = AgentResult {
            item_id: "timeout_item".to_string(),
            status: AgentStatus::Timeout,
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        add_individual_result_variables(&mut variables, 2, &timeout_result);
        assert_eq!(variables.get("result.2.status").unwrap(), "timeout");
    }

    /// Test truncate_output function
    #[test]
    fn test_truncate_output() {
        // Test short output - should not be truncated
        let short_output = "short output";
        assert_eq!(truncate_output(short_output, 100), "short output");

        // Test long output - should be truncated
        let long_output = "a".repeat(600);
        let truncated = truncate_output(&long_output, 500);
        assert!(truncated.len() <= 500 + "...[truncated]".len());
        assert!(truncated.ends_with("...[truncated]"));
        assert!(truncated.starts_with("aaa"));

        // Test exact length - should not be truncated
        let exact_output = "a".repeat(500);
        assert_eq!(truncate_output(&exact_output, 500), exact_output);
    }

    /// Test that the bug scenario we fixed is now properly handled
    #[test]
    fn test_mapreduce_variable_interpolation_bug_fix() {
        // This test simulates the exact scenario that was failing before our fix:
        // MapReduce variables (${map.successful}, ${map.failed}, ${map.total})
        // were showing as 0 instead of actual values in the reduce phase

        let map_results = vec![
            AgentResult {
                item_id: "item1".to_string(),
                status: AgentStatus::Success,
                output: Some("processed item 1".to_string()),
                commits: vec!["commit1".to_string()],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item2".to_string(),
                status: AgentStatus::Success,
                output: Some("processed item 2".to_string()),
                commits: vec!["commit2".to_string()],
                duration: Duration::from_secs(1),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
            AgentResult {
                item_id: "item3".to_string(),
                status: AgentStatus::Failed("processing error".to_string()),
                output: None,
                commits: vec![],
                duration: Duration::from_secs(1),
                error: Some("processing error".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                files_modified: vec![],
            },
        ];

        // Calculate summary - this is the core fix
        let summary = calculate_map_result_summary(&map_results);
        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.total, 3);

        // Build interpolation context - this ensures variables are available
        let interp_context =
            build_map_results_interpolation_context(&map_results, &summary).unwrap();

        // Test the exact variables that were failing
        let successful_value = interp_context
            .resolve_path(&["map".to_string(), "successful".to_string()])
            .unwrap();
        let failed_value = interp_context
            .resolve_path(&["map".to_string(), "failed".to_string()])
            .unwrap();
        let total_value = interp_context
            .resolve_path(&["map".to_string(), "total".to_string()])
            .unwrap();

        assert_eq!(successful_value, Value::Number(2.into()));
        assert_eq!(failed_value, Value::Number(1.into()));
        assert_eq!(total_value, Value::Number(3.into()));

        // Test shell command variables - this is what was causing substitution errors
        let shell_variables = build_agent_context_variables(&map_results, &summary).unwrap();

        assert_eq!(shell_variables.get("map.successful").unwrap(), "2");
        assert_eq!(shell_variables.get("map.failed").unwrap(), "1");
        assert_eq!(shell_variables.get("map.total").unwrap(), "3");

        // Before the fix, these would have been "0", "0", "0"
        // After the fix, they correctly show "2", "1", "3"
    }

/// Add variables for a single agent result (pure function)
fn add_individual_result_variables(
    variables: &mut HashMap<String, String>,
    index: usize,
    result: &AgentResult,
) {
    // Add basic result info
    variables.insert(format!("result.{}.item_id", index), result.item_id.clone());

    let status_string = match &result.status {
        AgentStatus::Success => "success".to_string(),
        AgentStatus::Failed(err) => format!("failed: {}", err),
        AgentStatus::Timeout => "timeout".to_string(),
        AgentStatus::Pending => "pending".to_string(),
        AgentStatus::Running => "running".to_string(),
        AgentStatus::Retrying(attempt) => format!("retrying: {}", attempt),
    };
    variables.insert(format!("result.{}.status", index), status_string);

    // Add output if available (truncated for safety)
    if let Some(ref output) = result.output {
        let truncated_output = truncate_output(output, 500);
        variables.insert(format!("result.{}.output", index), truncated_output);
    }

    // Add commit count
    variables.insert(
        format!("result.{}.commits", index),
        result.commits.len().to_string(),
    );
}

/// Truncate output to safe length (pure function)
fn truncate_output(output: &str, max_length: usize) -> String {
    if output.len() > max_length {
        format!("{}...[truncated]", &output[..max_length])
    } else {
        output.to_string()
    }
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
        // Create state manager with global storage support
        let state_manager: Arc<dyn JobStateManager> =
            match crate::cook::execution::state::DefaultJobStateManager::new_with_global(
                project_root.clone(),
            )
            .await
            {
                Ok(manager) => Arc::new(manager),
                Err(e) => {
                    warn!(
                        "Failed to create global state manager: {}, falling back to local",
                        e
                    );
                    Arc::new(crate::cook::execution::state::DefaultJobStateManager::new(
                        project_root.clone(),
                    ))
                }
            };

        // Create event logger
        let event_logger = Arc::new(EventLogger::new(vec![]));

        // Create subprocess manager
        let runner = Arc::new(crate::subprocess::runner::TokioProcessRunner::new());
        let subprocess = Arc::new(crate::subprocess::SubprocessManager::new(runner));

        // Create interpolation engine
        let interpolation_engine = Arc::new(Mutex::new(InterpolationEngine::new()));

        // Create command registry
        let command_registry = Arc::new(CommandRegistry::new());

        // Generate correlation ID for this instance
        let correlation_id = Uuid::new_v4().to_string();

        Self {
            claude_executor,
            session_manager,
            user_interaction,
            worktree_manager,
            worktree_pool: None,
            project_root,
            interpolation_engine,
            command_registry,
            subprocess,
            state_manager,
            event_logger,
            dlq: None,
            correlation_id,
            enhanced_progress_tracker: None,
            new_progress_tracker: None,
            enable_web_dashboard: false,
            setup_variables: HashMap::new(),
        }
    }

    /// Create error context for better error reporting
    fn create_error_context(
        &self,
        span_name: &str,
    ) -> crate::cook::execution::errors::ErrorContext {
        crate::cook::execution::errors::ErrorContext {
            correlation_id: self.correlation_id.clone(),
            timestamp: Utc::now(),
            hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string()),
            thread_id: format!("{:?}", std::thread::current().id()),
            span_trace: vec![crate::cook::execution::errors::SpanInfo {
                name: span_name.to_string(),
                start: Utc::now(),
                attributes: HashMap::new(),
            }],
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
        // Initialize CommitTracker for agent commit tracking
        let git_ops = Arc::new(crate::abstractions::RealGitOperations::new());
        let mut commit_tracker =
            crate::cook::commit_tracker::CommitTracker::new(git_ops, worktree_path.to_path_buf());
        commit_tracker.initialize().await.map_err(|e| {
            let context = self.create_error_context("commit_tracker_init");
            MapReduceError::General {
                message: format!("Failed to initialize commit tracker: {}", e),
                source: None,
            }
            .with_context(context)
            .error
        })?;

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
            files_modified,
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
    async fn get_modified_files(&self, worktree_path: &Path) -> MapReduceResult<Vec<String>> {
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
            .map(|s| s.to_string())
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
        // Validate reduce phase can proceed
        if let Err(validation_error) = validate_reduce_phase(map_results) {
            self.user_interaction
                .display_warning(&format!("⚠️ Skipping reduce phase: {}", validation_error));
            return Ok(());
        }

        self.user_interaction
            .display_progress("Starting reduce phase...");

        // Calculate summary statistics using pure functions
        let summary_stats = calculate_map_result_summary(map_results);

        self.user_interaction.display_info(&format!(
            "All {} successful agents merged to parent worktree",
            summary_stats.successful
        ));

        if summary_stats.failed > 0 {
            self.user_interaction.display_warning(&format!(
                "{} agents failed and were not merged",
                summary_stats.failed
            ));
        }

        // Build interpolation context using pure functions
        let _interp_context = build_map_results_interpolation_context(map_results, &summary_stats)
            .map_err(|e| handle_generic_error("build_interpolation_context", e))?;

        // Create a context for reduce phase execution in parent worktree
        let mut reduce_context = AgentContext::new(
            "reduce".to_string(),
            env.working_dir.clone(),
            env.worktree_name
                .clone()
                .unwrap_or_else(|| "main".to_string()),
            env.clone(),
        );

        // Build and add variables using pure functions
        let context_variables = build_agent_context_variables(map_results, &summary_stats)
            .map_err(|e| {
                let context = self.create_error_context("build_agent_context_variables");
                MapReduceError::General {
                    message: "Failed to build agent context variables from map results".to_string(),
                    source: Some(Box::new(e)),
                }
                .with_context(context)
                .error
            })?;

        // Transfer variables to reduce context
        for (key, value) in context_variables {
            reduce_context.variables.insert(key, value);
        }

        // Add map results to variable store as structured data for better access
        {
            use crate::cook::workflow::variables::CapturedValue;

            // Add summary statistics
            reduce_context
                .variable_store
                .set(
                    "map.successful",
                    CapturedValue::Number(summary_stats.successful as f64),
                )
                .await;
            reduce_context
                .variable_store
                .set(
                    "map.failed",
                    CapturedValue::Number(summary_stats.failed as f64),
                )
                .await;
            reduce_context
                .variable_store
                .set(
                    "map.total",
                    CapturedValue::Number(summary_stats.total as f64),
                )
                .await;

            // Add the full results as a structured JSON value
            if let Ok(results_value) = serde_json::to_value(map_results) {
                reduce_context
                    .variable_store
                    .set("map.results", CapturedValue::from(results_value))
                    .await;
            }

            // Also add individual results for easier access
            let results_array: Vec<CapturedValue> = map_results
                .iter()
                .map(|result| {
                    if let Ok(result_json) = serde_json::to_value(result) {
                        CapturedValue::from(result_json)
                    } else {
                        CapturedValue::String(format!("{:?}", result))
                    }
                })
                .collect();
            reduce_context
                .variable_store
                .set("map.results_array", CapturedValue::Array(results_array))
                .await;
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

            // Log available variables for debugging interpolation issues
            debug!(
                "Executing reduce step {} with variables: map.successful={}, map.failed={}, map.total={}",
                step_index + 1,
                reduce_context.variables.get("map.successful").unwrap_or(&"<missing>".to_string()),
                reduce_context.variables.get("map.failed").unwrap_or(&"<missing>".to_string()),
                reduce_context.variables.get("map.total").unwrap_or(&"<missing>".to_string())
            );

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
            CommandType::GoalSeek(_) => {
                let context = self.create_error_context("execute_single_step");
                return Err(MapReduceError::InvalidConfiguration {
                    reason: "Goal-seeking commands are not supported in MapReduce".to_string(),
                    field: "command_type".to_string(),
                    value: "goal_seek".to_string(),
                }
                .with_context(context)
                .error);
            }
            CommandType::Foreach(_) => {
                let context = self.create_error_context("execute_single_step");
                return Err(MapReduceError::InvalidConfiguration {
                    reason: "Foreach commands are not supported in MapReduce".to_string(),
                    field: "command_type".to_string(),
                    value: "foreach".to_string(),
                }
                .with_context(context)
                .error);
            }
        };

        // Capture command output if requested (new capture field)
        if let Some(capture_name) = &step.capture {
            let command_result = crate::cook::workflow::variables::CommandResult {
                stdout: Some(result.stdout.clone()),
                stderr: Some(result.stderr.clone()),
                exit_code: result.exit_code.unwrap_or(-1),
                success: result.success,
                duration: std::time::Duration::from_secs(0), // TODO: Track actual duration
            };

            let capture_format = step.capture_format.unwrap_or_default();
            let capture_streams = &step.capture_streams;

            context
                .variable_store
                .capture_command_result(
                    capture_name,
                    command_result,
                    capture_format,
                    capture_streams,
                )
                .await
                .map_err(|e| {
                    let context = self.create_error_context("capture_command_result");
                    MapReduceError::General {
                        message: format!("Failed to capture command result: {}", e),
                        source: None,
                    }
                    .with_context(context)
                    .error
                })?;

            // Also update captured_outputs for backward compatibility
            context
                .captured_outputs
                .insert(capture_name.clone(), result.stdout.clone());
        }

        // Capture output if requested (deprecated capture_output field)
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
                    return Err(MapReduceError::AgentTimeout(Box::new(
                        crate::cook::execution::errors::AgentTimeoutError {
                            job_id: "<unknown>".to_string(),
                            agent_id: "<unknown>".to_string(),
                            item_id: "<unknown>".to_string(),
                            duration_secs: timeout_secs,
                            last_operation: "shell command execution".to_string(),
                        },
                    )));
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
            worktree_pool: self.worktree_pool.clone(),
            project_root: self.project_root.clone(),
            interpolation_engine: self.interpolation_engine.clone(),
            command_registry: self.command_registry.clone(),
            subprocess: self.subprocess.clone(),
            state_manager: self.state_manager.clone(),
            event_logger: self.event_logger.clone(),
            dlq: self.dlq.clone(),
            correlation_id: self.correlation_id.clone(),
            enhanced_progress_tracker: self.enhanced_progress_tracker.clone(),
            new_progress_tracker: self.new_progress_tracker.clone(),
            enable_web_dashboard: self.enable_web_dashboard,
            setup_variables: self.setup_variables.clone(),
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: Some(HandlerStep {
                name: "test_handler".to_string(),
                attributes: HashMap::new(),
            }),
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: Some("test_command".to_string()),
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: Some("/test_command".to_string()),
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
            goal_seek: None,
            foreach: None,
            handler: None,
            name: None,
            command: None,
            capture_output: CaptureOutput::Disabled,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            auto_commit: false,
            commit_config: None,
            validate: None,
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            retry: None,
            when: None,
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
}}
