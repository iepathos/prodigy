//! Workflow executor with commit verification
//!
//! Executes workflow steps in sequence, verifies git commits when required,
//! and manages iteration logic for continuous improvement sessions.

use crate::abstractions::git::{GitOperations, RealGitOperations};
use crate::commands::{AttributeValue, CommandRegistry, ExecutionContext};
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::expression::{ExpressionEvaluator, VariableContext};
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::retry_state::RetryStateManager;
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::cook::workflow::checkpoint::{
    self, create_checkpoint_with_total_steps, CheckpointManager,
    CompletedStep as CheckpointCompletedStep, ResumeContext,
};
use crate::cook::workflow::error_recovery::ErrorRecoveryState;
use crate::cook::workflow::normalized;
use crate::cook::workflow::normalized::NormalizedWorkflow;
use crate::cook::workflow::on_failure::{HandlerStrategy, OnFailureConfig};
use crate::cook::workflow::validation::{ValidationConfig, ValidationResult};
use crate::session::{format_duration, TimingTracker};
use crate::testing::config::TestConfiguration;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Capture output configuration - either a boolean or a variable name
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CaptureOutput {
    /// Don't capture output
    #[default]
    Disabled,
    /// Capture to default variable names (claude.output, shell.output, etc.)
    Default,
    /// Capture to a custom variable name
    Variable(String),
}

impl Serialize for CaptureOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CaptureOutput::Disabled => serializer.serialize_bool(false),
            CaptureOutput::Default => serializer.serialize_bool(true),
            CaptureOutput::Variable(s) => serializer.serialize_str(s),
        }
    }
}

impl CaptureOutput {
    /// Check if output should be captured
    pub fn is_enabled(&self) -> bool {
        !matches!(self, CaptureOutput::Disabled)
    }

    /// Get the variable name to use for captured output
    pub fn get_variable_name(&self, command_type: &CommandType) -> Option<String> {
        match self {
            CaptureOutput::Disabled => None,
            CaptureOutput::Default => {
                // Use command-type specific default names
                Some(match command_type {
                    CommandType::Claude(_) | CommandType::Legacy(_) => "claude.output".to_string(),
                    CommandType::Shell(_) => "shell.output".to_string(),
                    CommandType::Handler { .. } => "handler.output".to_string(),
                    CommandType::Test(_) => "test.output".to_string(),
                    CommandType::GoalSeek(_) => "goal_seek.output".to_string(),
                    CommandType::Foreach(_) => "foreach.output".to_string(),
                })
            }
            CaptureOutput::Variable(name) => Some(name.clone()),
        }
    }
}

/// Custom deserializer for CaptureOutput that accepts bool or string
fn deserialize_capture_output<'de, D>(deserializer: D) -> Result<CaptureOutput, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CaptureOutputHelper {
        Bool(bool),
        String(String),
    }

    match CaptureOutputHelper::deserialize(deserializer)? {
        CaptureOutputHelper::Bool(false) => Ok(CaptureOutput::Disabled),
        CaptureOutputHelper::Bool(true) => Ok(CaptureOutput::Default),
        CaptureOutputHelper::String(s) => Ok(CaptureOutput::Variable(s)),
    }
}

/// Command type for workflow steps
#[derive(Debug, Clone, PartialEq)]
pub enum CommandType {
    /// Claude CLI command with args
    Claude(String),
    /// Shell command to execute
    Shell(String),
    /// Test command with retry logic
    Test(crate::config::command::TestCommand),
    /// Goal-seeking command with iterative refinement
    GoalSeek(crate::cook::goal_seek::GoalSeekConfig),
    /// Foreach command for parallel iteration
    Foreach(crate::config::command::ForeachConfig),
    /// Legacy name-based approach
    Legacy(String),
    /// Modular command handler
    Handler {
        handler_name: String,
        attributes: HashMap<String, AttributeValue>,
    },
}

/// Result of executing a step
#[derive(Debug, Clone, Default)]
pub struct StepResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Variable resolution tracking for verbose output
#[derive(Debug, Clone)]
pub struct VariableResolution {
    pub name: String,
    pub raw_expression: String,
    pub resolved_value: String,
}

/// Workflow context for variable interpolation
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    pub variables: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub iteration_vars: HashMap<String, String>,
    pub validation_results: HashMap<String, ValidationResult>,
    /// Variable store for advanced capture functionality
    pub variable_store: Arc<super::variables::VariableStore>,
}

impl Default for WorkflowContext {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
            validation_results: HashMap::new(),
            variable_store: Arc::new(super::variables::VariableStore::new()),
        }
    }
}

impl WorkflowContext {
    /// Build InterpolationContext from WorkflowContext variables (pure function)
    fn build_interpolation_context(&self) -> InterpolationContext {
        let mut context = InterpolationContext::new();

        // Add variables as strings
        for (key, value) in &self.variables {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add captured outputs
        for (key, value) in &self.captured_outputs {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add variables from variable store
        let store_vars = futures::executor::block_on(self.variable_store.to_hashmap());
        for (key, value) in store_vars {
            context.set(key, Value::String(value));
        }

        // Add iteration variables
        for (key, value) in &self.iteration_vars {
            context.set(key.clone(), Value::String(value.clone()));
        }

        // Add validation results as structured data
        for (key, validation_result) in &self.validation_results {
            let validation_value = serde_json::json!({
                "completion": validation_result.completion_percentage,
                "missing": validation_result.missing,
                "missing_count": validation_result.missing.len(),
                "status": validation_result.status,
                "implemented": validation_result.implemented,
                "gaps": validation_result.gaps
            });
            context.set(key.clone(), validation_value);
        }

        context
    }

    /// Track variable resolutions from interpolation (pure function)
    fn extract_variable_resolutions(
        template: &str,
        _result: &str,
        context: &InterpolationContext,
    ) -> Vec<VariableResolution> {
        let mut resolutions = Vec::new();

        // Find ${...} patterns in original template
        let braced_var_regex = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

        for captures in braced_var_regex.captures_iter(template) {
            if let Some(var_match) = captures.get(0) {
                let full_expression = var_match.as_str();
                let var_expression = captures.get(1).unwrap().as_str();

                // Parse path segments (handle dotted paths like "map.successful")
                let path_segments: Vec<String> =
                    var_expression.split('.').map(|s| s.to_string()).collect();

                // Check if this variable was resolved by looking in the context
                if let Ok(value) = context.resolve_path(&path_segments) {
                    let resolved_value = Self::value_to_string(&value, var_expression);

                    resolutions.push(VariableResolution {
                        name: var_expression.to_string(),
                        raw_expression: full_expression.to_string(),
                        resolved_value,
                    });
                }
            }
        }

        // Find $VAR patterns (unbraced variables)
        let unbraced_var_regex = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();

        for captures in unbraced_var_regex.captures_iter(template) {
            if let Some(var_match) = captures.get(0) {
                let full_expression = var_match.as_str();
                let var_name = captures.get(1).unwrap().as_str();

                // Check if this variable was resolved by looking in the context
                let path_segments = vec![var_name.to_string()];
                if let Ok(value) = context.resolve_path(&path_segments) {
                    let resolved_value = Self::value_to_string(&value, var_name);

                    resolutions.push(VariableResolution {
                        name: var_name.to_string(),
                        raw_expression: full_expression.to_string(),
                        resolved_value,
                    });
                }
            }
        }

        resolutions
    }

    /// Convert a JSON value to string representation for display (pure function)
    fn value_to_string(value: &Value, _var_name: &str) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Array(arr) => {
                // For arrays, if they contain strings, join them
                if arr.iter().all(|v| matches!(v, Value::String(_))) {
                    let strings: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    strings.join(", ")
                } else {
                    serde_json::to_string(arr).unwrap_or_else(|_| format!("{:?}", arr))
                }
            }
            other => {
                // For complex objects, try to serialize to JSON
                serde_json::to_string(other).unwrap_or_else(|_| format!("{:?}", other))
            }
        }
    }

    /// Interpolate variables in a template string
    pub fn interpolate(&self, template: &str) -> String {
        self.interpolate_with_tracking(template).0
    }

    /// Interpolate variables and track resolutions for verbose output
    pub fn interpolate_with_tracking(&self, template: &str) -> (String, Vec<VariableResolution>) {
        // Build interpolation context using pure function
        let context = self.build_interpolation_context();

        // Use InterpolationEngine for proper template parsing and variable resolution
        let mut engine = InterpolationEngine::new(false); // non-strict mode for backward compatibility

        match engine.interpolate(template, &context) {
            Ok(result) => {
                // Extract variable resolutions for tracking
                let resolutions = Self::extract_variable_resolutions(template, &result, &context);
                (result, resolutions)
            }
            Err(error) => {
                // Log interpolation failure for debugging
                tracing::warn!(
                    "Variable interpolation failed for template '{}': {}",
                    template,
                    error
                );

                // Provide detailed error information
                let available_variables = Self::get_available_variable_summary(&context);
                tracing::debug!("Available variables: {}", available_variables);

                // Fallback to original template on error (non-strict mode behavior)
                (template.to_string(), Vec::new())
            }
        }
    }

    /// Get summary of available variables for debugging (pure function)
    fn get_available_variable_summary(context: &InterpolationContext) -> String {
        let mut variables: Vec<String> = context.variables.keys().cloned().collect();
        variables.sort();

        if variables.is_empty() {
            "none".to_string()
        } else if variables.len() > 10 {
            format!(
                "{} variables ({}...)",
                variables.len(),
                variables[..3].join(", ")
            )
        } else {
            variables.join(", ")
        }
    }

    /// Enhanced interpolation with strict mode and detailed error reporting
    pub fn interpolate_strict(&self, template: &str) -> Result<String, String> {
        let context = self.build_interpolation_context();
        let mut engine = InterpolationEngine::new(true); // strict mode

        engine.interpolate(template, &context).map_err(|error| {
            let available_variables = Self::get_available_variable_summary(&context);
            format!(
                "Variable interpolation failed for template '{}': {}. Available variables: {}",
                template, error, available_variables
            )
        })
    }

    /// Resolve a variable path from the store (async)
    pub async fn resolve_variable(&self, path: &str) -> Option<String> {
        if let Ok(value) = self.variable_store.resolve_path(path).await {
            Some(value.to_string())
        } else {
            None
        }
    }
}

/// Handler step configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerStep {
    /// Handler name (e.g., "shell", "claude", "git", "cargo", "file")
    pub name: String,
    /// Attributes to pass to the handler
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

/// A workflow step with extended syntax support
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Legacy step name (for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Claude CLI command with args
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<String>,

    /// Shell command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Test command configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<crate::config::command::TestCommand>,

    /// Goal-seeking configuration for iterative refinement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_seek: Option<crate::cook::goal_seek::GoalSeekConfig>,

    /// Foreach configuration for parallel iteration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreach: Option<crate::config::command::ForeachConfig>,

    /// Legacy command field (for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Modular command handler with attributes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<HandlerStep>,

    /// Variable name to capture output into
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,

    /// Format for captured output (string, json, lines, number, boolean)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_format: Option<super::variables::CaptureFormat>,

    /// Which streams to capture (stdout, stderr, exit_code, etc.)
    #[serde(default)]
    pub capture_streams: super::variables::CaptureStreams,

    /// Output file for command results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<std::path::PathBuf>,

    /// Whether to capture command output (bool or variable name string) - DEPRECATED
    #[serde(default, deserialize_with = "deserialize_capture_output")]
    pub capture_output: CaptureOutput,

    /// Timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// Working directory for the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<PathBuf>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Conditional execution on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailureConfig>,

    /// Enhanced retry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<crate::cook::retry_v2::RetryConfig>,

    /// Conditional execution on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_success: Option<Box<WorkflowStep>>,

    /// Conditional execution based on exit codes
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub on_exit_code: HashMap<i32, Box<WorkflowStep>>,

    /// Whether this command is expected to create commits
    #[serde(default = "default_commit_required")]
    pub commit_required: bool,

    /// Commit configuration for auto-commit and validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_config: Option<crate::cook::commit_tracker::CommitConfig>,

    /// Auto-commit if changes detected
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub auto_commit: bool,

    /// Validation configuration for checking implementation completeness
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate: Option<ValidationConfig>,

    /// Step validation that runs after command execution to verify success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_validate: Option<super::step_validation::StepValidationSpec>,

    /// Skip step validation even if specified
    #[serde(default)]
    pub skip_validation: bool,

    /// Validation-specific timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_timeout: Option<u64>,

    /// Continue on validation failure
    #[serde(default)]
    pub ignore_validation_failure: bool,

    /// Conditional execution expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

fn default_commit_required() -> bool {
    false
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Configuration for sensitive variable patterns
#[derive(Debug, Clone)]
pub struct SensitivePatternConfig {
    /// Regex patterns to identify sensitive variable names
    pub name_patterns: Vec<Regex>,
    /// Regex patterns to identify sensitive values
    pub value_patterns: Vec<Regex>,
    /// Custom masking string (default: "***REDACTED***")
    pub mask_string: String,
}

impl Default for SensitivePatternConfig {
    fn default() -> Self {
        Self {
            // Default patterns for common sensitive variable names
            name_patterns: vec![
                Regex::new(r"(?i)(password|passwd|pwd)").unwrap(),
                Regex::new(r"(?i)(token|api[_-]?key|secret)").unwrap(),
                Regex::new(r"(?i)(auth|authorization|bearer)").unwrap(),
                Regex::new(r"(?i)(private[_-]?key|ssh[_-]?key)").unwrap(),
                Regex::new(r"(?i)(access[_-]?key|client[_-]?secret)").unwrap(),
            ],
            // Default patterns for common sensitive value formats
            value_patterns: vec![
                // GitHub/GitLab tokens (ghp_, glpat-, etc.)
                Regex::new(r"^(ghp_|gho_|ghu_|ghs_|ghr_|glpat-)").unwrap(),
                // AWS access keys
                Regex::new(r"^AKIA[0-9A-Z]{16}$").unwrap(),
                // JWT tokens
                Regex::new(r"^eyJ[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$").unwrap(),
                // Basic auth headers
                Regex::new(r"^(Basic|Bearer)\s+[A-Za-z0-9+/=]+$").unwrap(),
            ],
            mask_string: "***REDACTED***".to_string(),
        }
    }
}

/// Workflow execution mode
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowMode {
    /// Sequential execution (default)
    Sequential,
    /// MapReduce parallel execution
    MapReduce,
}

/// Extended workflow configuration
#[derive(Debug, Clone)]
pub struct ExtendedWorkflowConfig {
    /// Workflow name
    pub name: String,
    /// Execution mode
    pub mode: WorkflowMode,
    /// Steps to execute (for sequential mode or simple setup phase)
    pub steps: Vec<WorkflowStep>,
    /// Setup phase configuration (for advanced MapReduce setup with timeout and capture)
    pub setup_phase: Option<crate::cook::execution::SetupPhase>,
    /// Map phase configuration (for mapreduce mode)
    pub map_phase: Option<crate::cook::execution::MapPhase>,
    /// Reduce phase configuration (for mapreduce mode)
    pub reduce_phase: Option<crate::cook::execution::ReducePhase>,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Whether to iterate
    pub iterate: bool,
    /// Global retry defaults (applied to all steps unless overridden)
    pub retry_defaults: Option<crate::cook::retry_v2::RetryConfig>,
    /// Global environment configuration
    pub environment: Option<crate::cook::environment::EnvironmentConfig>,
    // collect_metrics removed - MMM focuses on orchestration, not metrics
}

/// Executes workflow steps with commit verification
pub struct WorkflowExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    timing_tracker: TimingTracker,
    test_config: Option<Arc<TestConfiguration>>,
    command_registry: Option<CommandRegistry>,
    subprocess: crate::subprocess::SubprocessManager,
    sensitive_config: SensitivePatternConfig,
    /// Track completed steps for resume functionality
    completed_steps: Vec<crate::cook::session::StepResult>,
    /// Checkpoint manager for workflow resumption
    checkpoint_manager: Option<Arc<CheckpointManager>>,
    /// Workflow ID for checkpoint tracking
    workflow_id: Option<String>,
    /// Checkpoint completed steps (separate from session steps)
    checkpoint_completed_steps: Vec<CheckpointCompletedStep>,
    /// Environment manager for workflow execution
    environment_manager: Option<crate::cook::environment::EnvironmentManager>,
    /// Global environment configuration
    global_environment_config: Option<crate::cook::environment::EnvironmentConfig>,
    /// Current workflow being executed (for checkpoint context)
    current_workflow: Option<NormalizedWorkflow>,
    /// Current step index being executed (for checkpoint context)
    current_step_index: Option<usize>,
    /// Git operations abstraction for testing
    git_operations: Arc<dyn GitOperations>,
    /// Resume context for handling interrupted workflows with error recovery state
    resume_context: Option<ResumeContext>,
    /// Retry state manager for checkpoint persistence
    retry_state_manager: Arc<RetryStateManager>,
    /// Path to the workflow file being executed (for checkpoint resume)
    workflow_path: Option<PathBuf>,
}

impl WorkflowExecutor {
    /// Sets the command registry for modular command handlers
    pub async fn with_command_registry(mut self) -> Self {
        self.command_registry = Some(CommandRegistry::with_defaults().await);
        self
    }

    /// Set the resume context for handling interrupted workflows
    pub fn with_resume_context(mut self, context: ResumeContext) -> Self {
        // Restore retry state if available in checkpoint
        if let Some(ref checkpoint) = context.checkpoint {
            if let Some(retry_checkpoint_state) = checkpoint.retry_checkpoint_state.clone() {
                // Clone the retry state manager Arc to avoid borrowing issues
                let retry_manager = self.retry_state_manager.clone();

                // Spawn a task to restore retry state asynchronously
                tokio::spawn(async move {
                    if let Err(e) = retry_manager
                        .restore_from_checkpoint(&retry_checkpoint_state)
                        .await
                    {
                        tracing::warn!("Failed to restore retry state from checkpoint: {}", e);
                    } else {
                        tracing::info!("Successfully restored retry state from checkpoint");
                    }
                });
            }
        }

        self.resume_context = Some(context);
        self
    }

    /// Save a checkpoint during step execution (e.g., during retries)
    async fn save_retry_checkpoint(
        &self,
        workflow: &NormalizedWorkflow,
        current_step_index: usize,
        retry_state: Option<checkpoint::RetryState>,
        ctx: &WorkflowContext,
    ) {
        if let Some(ref checkpoint_manager) = self.checkpoint_manager {
            if let Some(ref workflow_id) = self.workflow_id {
                let workflow_hash = format!("{:?}", workflow.steps.len());

                // Create a checkpoint with current retry state
                let mut checkpoint_steps = self.checkpoint_completed_steps.clone();

                // Add or update the current step with retry state
                if let Some(retry_state) = retry_state {
                    let step_name = if current_step_index < workflow.steps.len() {
                        match &workflow.steps[current_step_index].command {
                            normalized::StepCommand::Claude(cmd) => format!("claude: {}", cmd),
                            normalized::StepCommand::Shell(cmd) => format!("shell: {}", cmd),
                            normalized::StepCommand::Test { command, .. } => {
                                format!("test: {}", command)
                            }
                            normalized::StepCommand::Simple(cmd) => cmd.clone(),
                            _ => "complex command".to_string(),
                        }
                    } else {
                        "unknown step".to_string()
                    };

                    let retry_step = CheckpointCompletedStep {
                        step_index: current_step_index,
                        command: step_name,
                        success: false,
                        output: None,
                        captured_variables: HashMap::new(),
                        duration: Duration::from_secs(0),
                        completed_at: chrono::Utc::now(),
                        retry_state: Some(retry_state),
                    };

                    // Remove any existing entry for this step and add the new one
                    checkpoint_steps.retain(|s| s.step_index != current_step_index);
                    checkpoint_steps.push(retry_step);
                }

                let mut checkpoint = create_checkpoint_with_total_steps(
                    workflow_id.clone(),
                    workflow,
                    ctx,
                    checkpoint_steps,
                    current_step_index,
                    workflow_hash,
                    workflow.steps.len(),
                );

                // Set workflow path if available
                if let Some(ref path) = self.workflow_path {
                    checkpoint.workflow_path = Some(path.clone());
                }

                // Add retry state from RetryStateManager
                if let Ok(retry_checkpoint_state) =
                    self.retry_state_manager.create_checkpoint_state().await
                {
                    checkpoint.retry_checkpoint_state = Some(retry_checkpoint_state);
                }

                if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                    tracing::warn!("Failed to save retry checkpoint: {}", e);
                } else {
                    tracing::debug!(
                        "Saved retry checkpoint at step {} attempt",
                        current_step_index
                    );
                }
            }
        }
    }

    /// Execute a single workflow step (public for resume functionality)
    pub async fn execute_single_step(
        &mut self,
        step: &normalized::NormalizedStep,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Convert NormalizedStep to WorkflowStep for execution
        let workflow_step = self.normalized_to_workflow_step(step)?;

        // Create a minimal execution environment
        let env = ExecutionEnvironment {
            working_dir: std::env::current_dir()?,
            project_dir: std::env::current_dir()?,
            worktree_name: None,
            session_id: "resume-session".to_string(),
        };

        // Execute the step
        self.execute_step_internal(&workflow_step, &env, context)
            .await
    }

    /// Convert NormalizedStep to WorkflowStep
    fn normalized_to_workflow_step(
        &self,
        step: &normalized::NormalizedStep,
    ) -> Result<WorkflowStep> {
        use normalized::StepCommand;

        let mut workflow_step = WorkflowStep {
            name: Some(step.id.clone()),
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            command: None,
            handler: None,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            capture_output: CaptureOutput::Disabled,
            timeout: step.timeout.map(|d| d.as_secs()),
            working_dir: step.working_dir.clone(),
            env: step.env.clone(),
            on_failure: step.handlers.on_failure.clone(),
            retry: None,
            on_success: step.handlers.on_success.clone(),
            on_exit_code: step.handlers.on_exit_code.clone(),
            commit_required: step.commit_required,
            auto_commit: false,
            commit_config: None,
            validate: step.validation.clone(),
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            when: step.when.clone(),
        };

        // Set command based on step type
        match &step.command {
            StepCommand::Claude(cmd) => {
                workflow_step.claude = Some(cmd.clone());
            }
            StepCommand::Shell(cmd) => {
                workflow_step.shell = Some(cmd.clone());
            }
            StepCommand::Test {
                command,
                on_failure,
            } => {
                workflow_step.test = Some(crate::config::command::TestCommand {
                    command: command.clone(),
                    on_failure: on_failure.clone(),
                });
            }
            StepCommand::GoalSeek(config) => {
                workflow_step.goal_seek = Some(config.clone());
            }
            StepCommand::Handler(handler) => {
                workflow_step.handler = Some(HandlerStep {
                    name: handler.name.clone(),
                    attributes: handler.attributes.clone(),
                });
            }
            StepCommand::Simple(cmd) => {
                // For simple commands, use the legacy command field
                workflow_step.command = Some(cmd.clone());
            }
            StepCommand::Foreach(config) => {
                workflow_step.foreach = Some(config.clone());
            }
        }

        Ok(workflow_step)
    }

    /// Internal execute_step method that doesn't modify self
    async fn execute_step_internal(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Check conditional execution (when clause)
        if let Some(when_expr) = &step.when {
            let should_execute = self.evaluate_when_condition(when_expr, context)?;
            if !should_execute {
                tracing::info!("Skipping step due to when condition: {}", when_expr);
                return Ok(StepResult {
                    success: true,
                    exit_code: Some(0),
                    stdout: "Skipped due to when condition".to_string(),
                    stderr: String::new(),
                });
            }
        }

        // Determine command type
        let command_type = self.determine_command_type(step)?;

        // Prepare environment variables
        let env_vars = self.prepare_env_vars(step, env, context);

        // Execute the command based on its type
        self.execute_command_by_type(&command_type, step, env, context, env_vars)
            .await
    }

    /// Log variable resolutions when verbose mode is enabled
    fn log_variable_resolutions(&self, resolutions: &[VariableResolution]) {
        if tracing::enabled!(tracing::Level::DEBUG) && !resolutions.is_empty() {
            for resolution in resolutions {
                // Format the value for display, applying masking if sensitive
                let display_value = self.format_variable_value_with_masking(
                    &resolution.name,
                    &resolution.resolved_value,
                );
                tracing::debug!(
                    "   📊 Variable {} = {}",
                    resolution.raw_expression,
                    display_value
                );
            }
        }
    }

    /// Format variable value with sensitive data masking
    fn format_variable_value_with_masking(&self, name: &str, value: &str) -> String {
        // Check if this variable should be masked based on name patterns
        let should_mask_by_name = self
            .sensitive_config
            .name_patterns
            .iter()
            .any(|pattern| pattern.is_match(name));

        // Check if this value should be masked based on value patterns
        let should_mask_by_value = self
            .sensitive_config
            .value_patterns
            .iter()
            .any(|pattern| pattern.is_match(value));

        if should_mask_by_name || should_mask_by_value {
            // Return masked value
            self.sensitive_config.mask_string.clone()
        } else {
            // Format normally if not sensitive
            Self::format_variable_value_static(value)
        }
    }

    /// Format variable value for display (used by tests)
    #[cfg(test)]
    pub fn format_variable_value(&self, value: &str) -> String {
        Self::format_variable_value_static(value)
    }

    /// Static helper for formatting variable values
    fn format_variable_value_static(value: &str) -> String {
        const MAX_LENGTH: usize = 200;

        // Try to parse as JSON for pretty printing
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
            // Handle arrays and objects specially
            match &json_val {
                serde_json::Value::Array(arr) => {
                    if arr.is_empty() {
                        return "[]".to_string();
                    }
                    // For arrays, show as JSON if small, otherwise show count
                    let json_str =
                        serde_json::to_string(&json_val).unwrap_or_else(|_| value.to_string());
                    if json_str.len() <= MAX_LENGTH {
                        return json_str;
                    } else {
                        return format!("[...{} items...]", arr.len());
                    }
                }
                serde_json::Value::Object(obj) => {
                    if obj.is_empty() {
                        return "{}".to_string();
                    }
                    // For objects, pretty print if small
                    if let Ok(pretty) = serde_json::to_string_pretty(&json_val) {
                        if pretty.len() <= MAX_LENGTH {
                            return pretty;
                        } else {
                            // Show abbreviated version
                            let keys: Vec<_> = obj.keys().take(3).cloned().collect();
                            let preview = if obj.len() > 3 {
                                format!(
                                    "{{ {}, ... ({} total fields) }}",
                                    keys.join(", "),
                                    obj.len()
                                )
                            } else {
                                format!("{{ {} }}", keys.join(", "))
                            };
                            return preview;
                        }
                    }
                }
                _ => {
                    // For simple values, use as-is
                    return value.to_string();
                }
            }
        }

        // Not JSON, handle as plain string
        if value.len() <= MAX_LENGTH {
            // Quote strings to make them clear
            format!("\"{}\"", value)
        } else {
            // Truncate long values
            format!(
                "\"{}...\" (showing first {} chars)",
                &value[..MAX_LENGTH],
                MAX_LENGTH
            )
        }
    }

    /// Handle validation for a successful command
    async fn handle_validation(
        &mut self,
        validation_config: &crate::cook::workflow::validation::ValidationConfig,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<()> {
        // Execute validation
        let validation_result = self.execute_validation(validation_config, env, ctx).await?;

        // Store validation result in context
        ctx.validation_results
            .insert("validation".to_string(), validation_result.clone());

        // Always display validation percentage
        let percentage = validation_result.completion_percentage;
        let threshold = validation_config.threshold;

        // Check if validation passed
        if validation_config.is_complete(&validation_result) {
            self.user_interaction.display_success(&format!(
                "Validation passed: {:.1}% complete (threshold: {:.1}%)",
                percentage, threshold
            ));
        } else {
            self.user_interaction.display_warning(&format!(
                "Validation incomplete: {:.1}% complete (threshold: {:.1}%)",
                percentage, threshold
            ));

            // Handle incomplete validation
            if let Some(on_incomplete) = &validation_config.on_incomplete {
                self.handle_incomplete_validation(
                    validation_config,
                    on_incomplete,
                    validation_result,
                    env,
                    ctx,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Handle incomplete validation with retry logic
    async fn handle_incomplete_validation(
        &mut self,
        validation_config: &crate::cook::workflow::validation::ValidationConfig,
        on_incomplete: &crate::cook::workflow::validation::OnIncompleteConfig,
        initial_result: crate::cook::workflow::validation::ValidationResult,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<()> {
        let mut attempts = 0;
        let mut current_result = initial_result;

        while attempts < on_incomplete.max_attempts
            && !validation_config.is_complete(&current_result)
        {
            attempts += 1;

            self.user_interaction.display_info(&format!(
                "Attempting to complete implementation (attempt {}/{})",
                attempts, on_incomplete.max_attempts
            ));

            // Execute the completion handler
            if let Some(handler_step) = self.create_validation_handler(on_incomplete, ctx) {
                let step_display = self.get_step_display_name(&handler_step);
                self.user_interaction
                    .display_progress(&format!("Running recovery step: {}", step_display));

                let handler_result = Box::pin(self.execute_step(&handler_step, env, ctx)).await?;

                if !handler_result.success {
                    self.user_interaction
                        .display_error("Completion handler failed");
                    break;
                }

                // Re-run validation
                current_result = self.execute_validation(validation_config, env, ctx).await?;

                // Display validation percentage after each attempt
                let percentage = current_result.completion_percentage;
                let threshold = validation_config.threshold;
                if validation_config.is_complete(&current_result) {
                    self.user_interaction.display_success(&format!(
                        "Validation passed: {:.1}% complete (threshold: {:.1}%)",
                        percentage, threshold
                    ));
                } else {
                    self.user_interaction.display_info(&format!(
                        "Validation still incomplete: {:.1}% complete (threshold: {:.1}%)",
                        percentage, threshold
                    ));
                }

                // Update context
                ctx.validation_results
                    .insert("validation".to_string(), current_result.clone());
            } else {
                // Interactive mode
                if let Some(ref prompt) = on_incomplete.prompt {
                    let should_continue = self.user_interaction.prompt_confirmation(prompt).await?;

                    if !should_continue {
                        break;
                    }
                }
                break;
            }
        }

        // Check if we should fail the workflow
        if !validation_config.is_complete(&current_result) && on_incomplete.fail_workflow {
            return Err(anyhow!(
                "Validation failed after {} attempts. Completion: {:.1}%",
                attempts,
                current_result.completion_percentage
            ));
        }

        Ok(())
    }

    /// Handle step validation (first-class validation feature)
    async fn handle_step_validation(
        &mut self,
        validation_spec: &super::step_validation::StepValidationSpec,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        step: &WorkflowStep,
    ) -> Result<super::step_validation::StepValidationResult> {
        // Create a validation executor with the command executor
        let validation_executor = super::step_validation::StepValidationExecutor::new(Arc::new(
            StepValidationCommandExecutor {
                workflow_executor: self as *mut WorkflowExecutor,
                env: env.clone(),
                ctx: ctx.clone(),
            },
        )
            as Arc<dyn crate::cook::execution::CommandExecutor>);

        // Create execution context for validation
        let exec_context = crate::cook::execution::ExecutionContext {
            working_directory: env.working_dir.clone(),
            env_vars: std::collections::HashMap::new(),
            capture_output: true,
            timeout_seconds: step.validation_timeout,
            stdin: None,
            capture_streaming: false,
        };

        // Get step name for logging
        let step_name = step.name.as_deref().unwrap_or_else(|| {
            if step.claude.is_some() {
                "claude command"
            } else if step.shell.is_some() {
                "shell command"
            } else {
                "workflow step"
            }
        });

        // Execute validation with timeout if specified
        let validation_future =
            validation_executor.validate_step(validation_spec, &exec_context, step_name);

        let validation_result = if let Some(timeout_secs) = step.validation_timeout {
            let timeout = tokio::time::Duration::from_secs(timeout_secs);
            match tokio::time::timeout(timeout, validation_future).await {
                Ok(result) => result?,
                Err(_) => {
                    self.user_interaction.display_error(&format!(
                        "Step validation timed out after {} seconds",
                        timeout_secs
                    ));
                    super::step_validation::StepValidationResult {
                        passed: false,
                        results: vec![],
                        duration: std::time::Duration::from_secs(timeout_secs),
                        attempts: 1,
                    }
                }
            }
        } else {
            validation_future.await?
        };

        // Display validation result
        if validation_result.passed {
            self.user_interaction.display_success(&format!(
                "Step validation passed ({} validation{}, {} attempt{})",
                validation_result.results.len(),
                if validation_result.results.len() == 1 {
                    ""
                } else {
                    "s"
                },
                validation_result.attempts,
                if validation_result.attempts == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        } else {
            self.user_interaction.display_warning(&format!(
                "Step validation failed ({} validation{}, {} attempt{})",
                validation_result.results.len(),
                if validation_result.results.len() == 1 {
                    ""
                } else {
                    "s"
                },
                validation_result.attempts,
                if validation_result.attempts == 1 {
                    ""
                } else {
                    "s"
                }
            ));

            // Show details of failed validations
            for (idx, result) in validation_result.results.iter().enumerate() {
                if !result.passed {
                    self.user_interaction.display_info(&format!(
                        "  Validation {}: {} (exit code: {})",
                        idx + 1,
                        result.message,
                        result.exit_code
                    ));
                }
            }
        }

        Ok(validation_result)
    }

    /// Determine if workflow should fail based on command result
    /// Evaluate a when condition expression
    pub(crate) fn evaluate_when_condition(
        &self,
        when_expr: &str,
        context: &WorkflowContext,
    ) -> Result<bool> {
        let evaluator = ExpressionEvaluator::new();
        let mut variable_context = VariableContext::new();

        // Add workflow context variables to expression context
        for (key, value) in &context.variables {
            variable_context.set_string(key.clone(), value.clone());
        }

        // Add command outputs to expression context
        for (key, value) in &context.captured_outputs {
            variable_context.set_string(key.clone(), value.clone());
        }

        // Evaluate the expression
        evaluator
            .evaluate(when_expr, &variable_context)
            .with_context(|| format!("Failed to evaluate when condition: {}", when_expr))
    }

    fn should_fail_workflow(&self, result: &StepResult, step: &WorkflowStep) -> bool {
        if !result.success {
            // Command failed, check on_failure configuration
            if let Some(on_failure_config) = &step.on_failure {
                on_failure_config.should_fail_workflow()
            } else if let Some(test_cmd) = &step.test {
                // Legacy test command handling
                if let Some(test_on_failure) = &test_cmd.on_failure {
                    test_on_failure.fail_workflow
                } else {
                    true // No on_failure config, fail on error
                }
            } else {
                true // No on_failure handler, fail on error
            }
        } else {
            false // Command succeeded, don't fail
        }
    }

    /// Build a detailed error message for a failed step
    fn build_error_message(&self, step: &WorkflowStep, result: &StepResult) -> String {
        let step_display = self.get_step_display_name(step);
        let mut error_msg = format!("Step '{}' failed", step_display);

        if let Some(exit_code) = result.exit_code {
            error_msg.push_str(&format!(" with exit code {}", exit_code));
        }

        // Add stderr if available
        if !result.stderr.trim().is_empty() {
            error_msg.push_str("\n\n=== Error Output (stderr) ===");
            self.append_truncated_output(&mut error_msg, &result.stderr);
        }

        // Add stdout if stderr was empty but stdout has content
        if result.stderr.trim().is_empty() && !result.stdout.trim().is_empty() {
            error_msg.push_str("\n\n=== Standard Output (stdout) ===");
            self.append_truncated_output(&mut error_msg, &result.stdout);
        }

        error_msg
    }

    /// Append output to error message, truncating if necessary
    fn append_truncated_output(&self, error_msg: &mut String, output: &str) {
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() <= 50 {
            error_msg.push('\n');
            error_msg.push_str(output);
        } else {
            // Show first 25 and last 25 lines for large outputs
            error_msg.push('\n');
            for line in lines.iter().take(25) {
                error_msg.push_str(line);
                error_msg.push('\n');
            }
            error_msg.push_str("\n... [output truncated] ...\n\n");
            for line in lines.iter().rev().take(25).rev() {
                error_msg.push_str(line);
                error_msg.push('\n');
            }
        }
    }

    /// Execute command based on its type
    async fn execute_command_by_type(
        &mut self,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        // Add timeout to environment variables if configured for the step
        if let Some(timeout_secs) = step.timeout {
            env_vars.insert(
                "PRODIGY_COMMAND_TIMEOUT".to_string(),
                timeout_secs.to_string(),
            );
        }

        match command_type.clone() {
            CommandType::Claude(cmd) => {
                let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_claude_command(&interpolated_cmd, env, env_vars)
                    .await
            }
            CommandType::Shell(cmd) => {
                let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_shell_for_step(&interpolated_cmd, step, env, ctx, env_vars)
                    .await
            }
            CommandType::Test(test_cmd) => {
                self.execute_test_command(test_cmd, env, ctx, env_vars, None, None)
                    .await
            }
            CommandType::Legacy(cmd) => {
                let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_claude_command(&interpolated_cmd, env, env_vars)
                    .await
            }
            CommandType::Handler {
                handler_name,
                attributes,
            } => {
                self.execute_handler_command(handler_name, attributes, env, ctx, env_vars)
                    .await
            }
            CommandType::GoalSeek(goal_seek_config) => {
                self.execute_goal_seek_command(goal_seek_config, env, ctx, &env_vars)
                    .await
            }
            CommandType::Foreach(foreach_config) => {
                self.execute_foreach_command(foreach_config, env, ctx, &env_vars)
                    .await
            }
        }
    }

    /// Execute foreach command with parallel iteration
    async fn execute_foreach_command(
        &self,
        foreach_config: crate::config::command::ForeachConfig,
        _env: &ExecutionEnvironment,
        _ctx: &WorkflowContext,
        _env_vars: &HashMap<String, String>,
    ) -> Result<StepResult> {
        use crate::cook::execution::foreach::execute_foreach;

        let result = execute_foreach(&foreach_config).await?;

        // Return aggregated results
        Ok(StepResult {
            success: result.failed_items == 0,
            stdout: format!(
                "Foreach completed: {} total, {} successful, {} failed",
                result.total_items, result.successful_items, result.failed_items
            ),
            stderr: if result.failed_items > 0 {
                format!("{} items failed", result.failed_items)
            } else {
                String::new()
            },
            exit_code: Some(if result.failed_items == 0 { 0 } else { 1 }),
        })
    }

    /// Execute goal-seeking command with iterative refinement
    async fn execute_goal_seek_command(
        &self,
        goal_seek_config: crate::cook::goal_seek::GoalSeekConfig,
        _env: &ExecutionEnvironment,
        _ctx: &WorkflowContext,
        _env_vars: &HashMap<String, String>,
    ) -> Result<StepResult> {
        use crate::cook::goal_seek::{shell_executor::ShellCommandExecutor, GoalSeekEngine};

        // Create shell command executor for goal-seeking
        let executor = Box::new(ShellCommandExecutor::new());

        // Create goal-seek engine
        let mut engine = GoalSeekEngine::new(executor);

        // Execute goal-seeking
        let result = engine.seek(goal_seek_config.clone()).await?;

        // Convert result to StepResult
        match result {
            crate::cook::goal_seek::GoalSeekResult::Success {
                attempts,
                final_score,
                execution_time: _,
            } => Ok(StepResult {
                success: true,
                stdout: format!(
                    "Goal '{}' achieved in {} attempts with score {}%",
                    goal_seek_config.goal, attempts, final_score
                ),
                stderr: String::new(),
                exit_code: Some(0),
            }),
            crate::cook::goal_seek::GoalSeekResult::MaxAttemptsReached {
                attempts,
                best_score,
                last_output: _,
            } => {
                if goal_seek_config.fail_on_incomplete.unwrap_or(false) {
                    Err(anyhow::anyhow!(
                        "Goal '{}' not achieved after {} attempts. Best score: {}%",
                        goal_seek_config.goal,
                        attempts,
                        best_score
                    ))
                } else {
                    Ok(StepResult {
                        success: false,
                        stdout: format!(
                            "Goal '{}' not achieved after {} attempts. Best score: {}%",
                            goal_seek_config.goal, attempts, best_score
                        ),
                        stderr: String::new(),
                        exit_code: Some(1),
                    })
                }
            }
            crate::cook::goal_seek::GoalSeekResult::Timeout {
                attempts,
                best_score,
                elapsed,
            } => Err(anyhow::anyhow!(
                "Goal '{}' timed out after {} attempts and {:?}. Best score: {}%",
                goal_seek_config.goal,
                attempts,
                elapsed,
                best_score
            )),
            crate::cook::goal_seek::GoalSeekResult::Converged {
                attempts,
                final_score,
                reason,
            } => {
                if goal_seek_config.fail_on_incomplete.unwrap_or(false)
                    && final_score < goal_seek_config.threshold
                {
                    Err(anyhow::anyhow!(
                        "Goal '{}' converged after {} attempts but didn't reach threshold. Score: {}%, Reason: {}",
                        goal_seek_config.goal, attempts, final_score, reason
                    ))
                } else {
                    Ok(StepResult {
                        success: final_score >= goal_seek_config.threshold,
                        stdout: format!(
                            "Goal '{}' converged after {} attempts. Score: {}%, Reason: {}",
                            goal_seek_config.goal, attempts, final_score, reason
                        ),
                        stderr: String::new(),
                        exit_code: Some(if final_score >= goal_seek_config.threshold {
                            0
                        } else {
                            1
                        }),
                    })
                }
            }
            crate::cook::goal_seek::GoalSeekResult::Failed { attempts, error } => {
                Err(anyhow::anyhow!(
                    "Goal '{}' failed after {} attempts: {}",
                    goal_seek_config.goal,
                    attempts,
                    error
                ))
            }
        }
    }

    /// Execute shell command for a step with appropriate retry logic
    async fn execute_shell_for_step(
        &self,
        interpolated_cmd: &str,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        // Check if this shell command has test-style retry logic
        // For backward compatibility with converted test commands
        if let Some(test_cmd) = &step.test {
            if test_cmd.on_failure.is_some() {
                return self
                    .execute_shell_with_retry(
                        interpolated_cmd,
                        test_cmd.on_failure.as_ref(),
                        env,
                        ctx,
                        env_vars,
                        step.timeout,
                    )
                    .await;
            }
        }

        // Regular shell command without retry logic
        self.execute_shell_command(interpolated_cmd, env, env_vars, step.timeout)
            .await
    }

    /// Handle conditional execution (on_failure, on_success, on_exit_code)
    async fn handle_conditional_execution(
        &mut self,
        step: &WorkflowStep,
        mut result: StepResult,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Handle failure
        if !result.success {
            if let Some(on_failure_config) = &step.on_failure {
                result = self
                    .handle_on_failure(step, result, on_failure_config, env, ctx)
                    .await?;
            }
        } else if let Some(on_success) = &step.on_success {
            // Handle success
            self.user_interaction
                .display_info("Executing on_success step...");
            let success_result = Box::pin(self.execute_step(on_success, env, ctx)).await?;
            result.stdout.push_str("\n--- on_success output ---\n");
            result.stdout.push_str(&success_result.stdout);
        }

        // Handle exit code specific steps
        if let Some(exit_code) = result.exit_code {
            if let Some(exit_step) = step.on_exit_code.get(&exit_code) {
                self.user_interaction
                    .display_info(&format!("Executing on_exit_code[{exit_code}] step..."));
                let exit_result = Box::pin(self.execute_step(exit_step, env, ctx)).await?;
                result
                    .stdout
                    .push_str(&format!("\n--- on_exit_code[{exit_code}] output ---\n"));
                result.stdout.push_str(&exit_result.stdout);
            }
        }

        Ok(result)
    }

    /// Handle on_failure configuration with retry logic
    async fn handle_on_failure(
        &mut self,
        step: &WorkflowStep,
        mut result: StepResult,
        on_failure_config: &OnFailureConfig,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Inject error context as variables
        let step_name = self.get_step_display_name(step);
        ctx.variables
            .insert("error.message".to_string(), result.stderr.clone());
        ctx.variables.insert(
            "error.exit_code".to_string(),
            result.exit_code.unwrap_or(-1).to_string(),
        );
        ctx.variables
            .insert("error.step".to_string(), step_name.clone());
        ctx.variables
            .insert("error.timestamp".to_string(), Utc::now().to_rfc3339());

        // Get handler commands
        let handler_commands = on_failure_config.handler_commands();

        if !handler_commands.is_empty() {
            let strategy = on_failure_config.strategy();
            self.user_interaction.display_info(&format!(
                "Executing on_failure handler ({:?} strategy)...",
                strategy
            ));

            let mut handler_success = true;
            let mut handler_outputs = Vec::new();

            // Execute each handler command
            for (idx, cmd) in handler_commands.iter().enumerate() {
                self.user_interaction.display_progress(&format!(
                    "Handler command {}/{}",
                    idx + 1,
                    handler_commands.len()
                ));

                // Create a WorkflowStep from the HandlerCommand
                let handler_step = WorkflowStep {
                    name: None,
                    shell: cmd.shell.clone(),
                    claude: cmd.claude.clone(),
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    command: None,
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    auto_commit: false,
                    commit_config: None,
                    output_file: None,
                    timeout: on_failure_config.handler_timeout(),
                    capture_output: CaptureOutput::Disabled,
                    on_failure: None,
                    retry: None,
                    on_success: None,
                    on_exit_code: Default::default(),
                    commit_required: false,
                    working_dir: None,
                    env: Default::default(),
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                };

                // Execute the handler command
                match Box::pin(self.execute_step(&handler_step, env, ctx)).await {
                    Ok(handler_result) => {
                        handler_outputs.push(handler_result.stdout.clone());
                        if !handler_result.success && !cmd.continue_on_error {
                            handler_success = false;
                            self.user_interaction
                                .display_error(&format!("Handler command {} failed", idx + 1));
                            break;
                        }
                    }
                    Err(e) => {
                        self.user_interaction.display_error(&format!(
                            "Handler command {} error: {}",
                            idx + 1,
                            e
                        ));
                        if !cmd.continue_on_error {
                            handler_success = false;
                            break;
                        }
                    }
                }
            }

            // Add handler output to result
            result.stdout.push_str("\n--- on_failure output ---\n");
            result.stdout.push_str(&handler_outputs.join("\n"));

            // Check if step should be marked as recovered
            if handler_success && strategy == HandlerStrategy::Recovery {
                self.user_interaction
                    .display_success("Step recovered through on_failure handler");
                result.success = true;
                // Clear error information since we recovered
                result.stderr.clear();
                result.exit_code = Some(0);
            }

            // Check if handler failure should be fatal
            if !handler_success && on_failure_config.handler_failure_fatal() {
                return Err(anyhow!("Handler failure is fatal"));
            }

            // Check if we should retry the original command
            if on_failure_config.should_retry() && !result.success {
                let max_retries = on_failure_config.max_retries();
                for retry in 1..=max_retries {
                    self.user_interaction.display_info(&format!(
                        "Retrying original command (attempt {}/{})",
                        retry, max_retries
                    ));
                    // Create a copy of the step without on_failure to avoid recursion
                    let mut retry_step = step.clone();
                    retry_step.on_failure = None;
                    let retry_result = Box::pin(self.execute_step(&retry_step, env, ctx)).await?;
                    if retry_result.success {
                        result = retry_result;
                        break;
                    }
                }
            }
        } else if let Some(handler) = on_failure_config.handler() {
            // Fallback to legacy handler for backward compatibility
            self.user_interaction
                .display_info("Executing on_failure handler...");
            let failure_result = Box::pin(self.execute_step(&handler, env, ctx)).await?;
            result.stdout.push_str("\n--- on_failure output ---\n");
            result.stdout.push_str(&failure_result.stdout);

            // Check if we should retry the original command
            if on_failure_config.should_retry() {
                let max_retries = on_failure_config.max_retries();
                for retry in 1..=max_retries {
                    self.user_interaction.display_info(&format!(
                        "Retrying original command (attempt {}/{})",
                        retry, max_retries
                    ));
                    // Create a copy of the step without on_failure to avoid recursion
                    let mut retry_step = step.clone();
                    retry_step.on_failure = None;
                    let retry_result = Box::pin(self.execute_step(&retry_step, env, ctx)).await?;
                    if retry_result.success {
                        result = retry_result;
                        break;
                    }
                }
            }
        }

        // Clear error variables from context
        ctx.variables.remove("error.message");
        ctx.variables.remove("error.exit_code");
        ctx.variables.remove("error.step");
        ctx.variables.remove("error.timestamp");

        Ok(result)
    }

    /// Create a new workflow executor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            timing_tracker: TimingTracker::new(),
            test_config: None,
            command_registry: None,
            subprocess: crate::subprocess::SubprocessManager::production(),
            sensitive_config: SensitivePatternConfig::default(),
            completed_steps: Vec::new(),
            checkpoint_manager: None,
            workflow_id: None,
            checkpoint_completed_steps: Vec::new(),
            environment_manager: None,
            global_environment_config: None,
            current_workflow: None,
            current_step_index: None,
            git_operations: Arc::new(RealGitOperations::new()),
            resume_context: None,
            retry_state_manager: Arc::new(RetryStateManager::new()),
            workflow_path: None,
        }
    }

    /// Set the workflow file path (for checkpoint resume)
    pub fn with_workflow_path(mut self, path: PathBuf) -> Self {
        self.workflow_path = Some(path);
        self
    }

    /// Set the environment configuration for the workflow
    pub fn with_environment_config(
        mut self,
        config: crate::cook::environment::EnvironmentConfig,
    ) -> Result<Self> {
        // Initialize environment manager with current directory
        let current_dir = std::env::current_dir()?;
        self.environment_manager = Some(crate::cook::environment::EnvironmentManager::new(
            current_dir,
        )?);
        self.global_environment_config = Some(config);
        Ok(self)
    }

    /// Set the checkpoint manager for workflow resumption
    pub fn with_checkpoint_manager(
        mut self,
        manager: Arc<CheckpointManager>,
        workflow_id: String,
    ) -> Self {
        self.checkpoint_manager = Some(manager);
        self.workflow_id = Some(workflow_id);
        self
    }

    /// Configure sensitive pattern detection
    pub fn with_sensitive_patterns(mut self, config: SensitivePatternConfig) -> Self {
        self.sensitive_config = config;
        self
    }

    /// Create a new workflow executor with test configuration
    pub fn with_test_config(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        test_config: Arc<TestConfiguration>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            timing_tracker: TimingTracker::new(),
            test_config: Some(test_config),
            command_registry: None,
            subprocess: crate::subprocess::SubprocessManager::production(),
            sensitive_config: SensitivePatternConfig::default(),
            completed_steps: Vec::new(),
            checkpoint_manager: None,
            workflow_id: None,
            checkpoint_completed_steps: Vec::new(),
            environment_manager: None,
            global_environment_config: None,
            current_workflow: None,
            current_step_index: None,
            git_operations: Arc::new(RealGitOperations::new()),
            resume_context: None,
            retry_state_manager: Arc::new(RetryStateManager::new()),
            workflow_path: None,
        }
    }

    /// Create executor with test configuration and custom git operations
    #[cfg(test)]
    pub fn with_test_config_and_git(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        test_config: Arc<TestConfiguration>,
        git_operations: Arc<dyn GitOperations>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            timing_tracker: TimingTracker::new(),
            test_config: Some(test_config),
            command_registry: None,
            subprocess: crate::subprocess::SubprocessManager::production(),
            sensitive_config: SensitivePatternConfig::default(),
            completed_steps: Vec::new(),
            checkpoint_manager: None,
            workflow_id: None,
            checkpoint_completed_steps: Vec::new(),
            environment_manager: None,
            global_environment_config: None,
            current_workflow: None,
            current_step_index: None,
            git_operations,
            resume_context: None,
            retry_state_manager: Arc::new(RetryStateManager::new()),
            workflow_path: None,
        }
    }

    /// Convert serde_json::Value to AttributeValue
    fn json_to_attribute_value(&self, value: serde_json::Value) -> AttributeValue {
        Self::json_to_attribute_value_static(value)
    }

    fn json_to_attribute_value_static(value: serde_json::Value) -> AttributeValue {
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
            serde_json::Value::Array(arr) => AttributeValue::Array(
                arr.into_iter()
                    .map(Self::json_to_attribute_value_static)
                    .collect(),
            ),
            serde_json::Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    map.insert(k, Self::json_to_attribute_value_static(v));
                }
                AttributeValue::Object(map)
            }
            serde_json::Value::Null => AttributeValue::Null,
        }
    }

    /// Determine command type from a workflow step
    pub(crate) fn determine_command_type(&self, step: &WorkflowStep) -> Result<CommandType> {
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
        if step.goal_seek.is_some() {
            specified_count += 1;
        }
        if step.foreach.is_some() {
            specified_count += 1;
        }
        if step.name.is_some() || step.command.is_some() {
            specified_count += 1;
        }

        // Ensure only one command type is specified
        if specified_count > 1 {
            return Err(anyhow!(
                "Multiple command types specified. Use only one of: claude, shell, test, handler, goal_seek, foreach, or name/command"
            ));
        }

        if specified_count == 0 {
            return Err(anyhow!(
                "No command specified. Use one of: claude, shell, test, handler, goal_seek, foreach, or name/command"
            ));
        }

        // Return the appropriate command type
        if let Some(handler_step) = &step.handler {
            // Convert serde_json::Value to AttributeValue
            let mut attributes = HashMap::new();
            for (key, value) in &handler_step.attributes {
                attributes.insert(key.clone(), self.json_to_attribute_value(value.clone()));
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
        } else if let Some(goal_seek_config) = &step.goal_seek {
            Ok(CommandType::GoalSeek(goal_seek_config.clone()))
        } else if let Some(foreach_config) = &step.foreach {
            Ok(CommandType::Foreach(foreach_config.clone()))
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

    /// Get display name for a step
    pub(crate) fn get_step_display_name(&self, step: &WorkflowStep) -> String {
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

    /// Execute command with enhanced retry logic
    #[allow(clippy::too_many_arguments)]
    async fn execute_with_enhanced_retry(
        &mut self,
        retry_config: crate::cook::retry_v2::RetryConfig,
        step_name: &str,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        use crate::cook::retry_state::RetryAttempt;
        use crate::cook::retry_v2::RetryExecutor;

        let retry_executor = RetryExecutor::new(retry_config.clone());
        let mut attempt = 0;
        let mut last_error = None;
        let command_id = step_name.to_string();

        // Manual retry loop since we can't clone self
        loop {
            attempt += 1;

            // Check if we should retry
            if attempt > retry_config.attempts {
                if let Some(err) = last_error {
                    return Err(anyhow::anyhow!(
                        "Failed after {} attempts: {}",
                        retry_config.attempts,
                        err
                    ));
                }
                break;
            }

            // Calculate delay if this is a retry
            if attempt > 1 {
                let delay = retry_executor.calculate_delay(attempt - 1);
                let jittered_delay = retry_executor.apply_jitter(delay);

                self.user_interaction.display_info(&format!(
                    "Retrying {} (attempt {}/{}) after {:?}",
                    step_name, attempt, retry_config.attempts, jittered_delay
                ));

                tokio::time::sleep(jittered_delay).await;
            }

            // Execute the command
            let attempt_start = std::time::Instant::now();
            match self
                .execute_command_by_type(command_type, step, env, ctx, env_vars.clone())
                .await
            {
                Ok(result) => {
                    if attempt > 1 {
                        self.user_interaction
                            .display_info(&format!("Command succeeded after {} attempts", attempt));

                        // Record successful retry attempt
                        let retry_attempt = RetryAttempt {
                            attempt_number: attempt,
                            executed_at: chrono::Utc::now(),
                            duration: attempt_start.elapsed(),
                            success: true,
                            error: None,
                            backoff_applied: if attempt > 1 {
                                retry_executor.calculate_delay(attempt - 1)
                            } else {
                                Duration::from_secs(0)
                            },
                            exit_code: result.exit_code,
                        };
                        let _ = self
                            .retry_state_manager
                            .update_retry_state(&command_id, retry_attempt, &retry_config)
                            .await;
                    }
                    return Ok(result);
                }
                Err(err) => {
                    let error_str = err.to_string();

                    // Record failed retry attempt
                    let retry_attempt = RetryAttempt {
                        attempt_number: attempt,
                        executed_at: chrono::Utc::now(),
                        duration: attempt_start.elapsed(),
                        success: false,
                        error: Some(error_str.clone()),
                        backoff_applied: if attempt > 1 {
                            retry_executor.calculate_delay(attempt - 1)
                        } else {
                            Duration::from_secs(0)
                        },
                        exit_code: None,
                    };
                    let _ = self
                        .retry_state_manager
                        .update_retry_state(&command_id, retry_attempt, &retry_config)
                        .await;

                    // Check if we should retry this error
                    let should_retry = if retry_config.retry_on.is_empty() {
                        true // Retry all errors if no specific matchers
                    } else {
                        retry_config
                            .retry_on
                            .iter()
                            .any(|matcher| matcher.matches(&error_str))
                    };

                    if !should_retry || attempt >= retry_config.attempts {
                        return Err(err);
                    }

                    self.user_interaction.display_warning(&format!(
                        "Command failed (attempt {}/{}): {}",
                        attempt, retry_config.attempts, error_str
                    ));

                    last_error = Some(error_str);
                }
            }
        }

        Err(anyhow::anyhow!("Retry logic error: should not reach here"))
    }

    /// Execute a workflow
    pub async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Handle MapReduce mode
        if workflow.mode == WorkflowMode::MapReduce {
            return self.execute_mapreduce(workflow, env).await;
        }

        let workflow_start = Instant::now();

        // Only show workflow info for non-empty workflows
        if !workflow.steps.is_empty() {
            self.user_interaction.display_info(&format!(
                "Executing workflow: {} (max {} iterations)",
                workflow.name, workflow.max_iterations
            ));
        }

        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        let skip_validation =
            std::env::var("PRODIGY_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";

        if workflow.iterate {
            self.user_interaction
                .display_progress("Starting improvement loop");
        }

        let mut iteration = 0;
        let mut should_continue = true;
        let mut any_changes = false;

        // Clear completed steps at the start of a new workflow
        self.completed_steps.clear();

        // Initialize workflow context
        let mut workflow_context = WorkflowContext::default();

        // Add any command-line arguments or environment variables
        if let Ok(arg) = std::env::var("PRODIGY_ARG") {
            workflow_context.variables.insert("ARG".to_string(), arg);
        }

        // Add project root and working directory
        workflow_context.variables.insert(
            "PROJECT_ROOT".to_string(),
            env.working_dir.to_string_lossy().to_string(),
        );

        // Add worktree name if available
        if let Ok(worktree) = std::env::var("PRODIGY_WORKTREE") {
            workflow_context
                .variables
                .insert("WORKTREE".to_string(), worktree);
        }

        // Start workflow timing in session
        self.session_manager
            .update_session(SessionUpdate::StartWorkflow)
            .await?;

        while should_continue && iteration < workflow.max_iterations {
            iteration += 1;

            // Update iteration context
            workflow_context
                .iteration_vars
                .insert("ITERATION".to_string(), iteration.to_string());

            self.user_interaction.display_progress(&format!(
                "Starting iteration {}/{}",
                iteration, workflow.max_iterations
            ));

            // Start iteration timing
            self.timing_tracker.start_iteration();

            // Update session
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;
            self.session_manager
                .update_session(SessionUpdate::StartIteration(iteration))
                .await?;

            // Execute workflow steps
            for (step_index, step) in workflow.steps.iter().enumerate() {
                // Check if we should skip this step (already completed in previous run)
                if let Some(ref resume_ctx) = self.resume_context {
                    if resume_ctx
                        .skip_steps
                        .iter()
                        .any(|s| s.step_index == step_index)
                    {
                        self.user_interaction.display_info(&format!(
                            "Skipping already completed step {}/{}: {}",
                            step_index + 1,
                            workflow.steps.len(),
                            self.get_step_display_name(step)
                        ));
                        continue;
                    }

                    // Check if we have error recovery state stored in variables
                    if let Some(recovery_state_value) =
                        resume_ctx.variable_state.get("__error_recovery_state")
                    {
                        // Parse the error recovery state from JSON
                        if let Ok(error_recovery_state) = serde_json::from_value::<ErrorRecoveryState>(
                            recovery_state_value.clone(),
                        ) {
                            // If this step had an active error handler that wasn't completed,
                            // we need to ensure it gets executed if the step fails again
                            if !error_recovery_state.active_handlers.is_empty() {
                                tracing::info!(
                                    "Restored {} error handlers for step {}",
                                    error_recovery_state.active_handlers.len(),
                                    step_index
                                );
                                // Store error context in workflow variables for handler execution
                                for (key, value) in error_recovery_state.error_context {
                                    workflow_context
                                        .variables
                                        .insert(format!("error.{}", key), value.to_string());
                                }
                            }
                        }
                    }
                }

                // Store current workflow context for checkpoint tracking
                // TODO: Convert workflow to NormalizedWorkflow for checkpoint tracking
                // self.current_workflow = Some(workflow.clone());
                self.current_step_index = Some(step_index);

                let step_display = self.get_step_display_name(step);
                self.user_interaction.display_progress(&format!(
                    "Executing step {}/{}: {}",
                    step_index + 1,
                    workflow.steps.len(),
                    step_display
                ));

                // Get HEAD before command execution if we need to verify commits
                let head_before = if !skip_validation && step.commit_required && !test_mode {
                    Some(self.get_current_head(&env.working_dir).await?)
                } else {
                    None
                };

                // Start command timing
                self.timing_tracker.start_command(step_display.clone());
                let command_start = Instant::now();
                let step_started_at = chrono::Utc::now();

                // Execute the step with context
                let step_result = self
                    .execute_step(step, env, &mut workflow_context)
                    .await
                    .context(format!("Failed to execute step: {step_display}"))?;

                // Display subprocess output when verbose logging is enabled
                // Show summary at DEBUG level (-v), full output at TRACE level (-vv)
                if tracing::enabled!(tracing::Level::DEBUG) {
                    // At DEBUG level, show first/last few lines if output is large
                    if !step_result.stdout.is_empty() {
                        let stdout_lines: Vec<&str> = step_result.stdout.lines().collect();
                        if stdout_lines.len() <= 20 || tracing::enabled!(tracing::Level::TRACE) {
                            // Show full output if small or at TRACE level
                            tracing::debug!("Command stdout:\n{}", step_result.stdout);
                        } else {
                            // Show abbreviated output at DEBUG level
                            let preview: String = stdout_lines
                                .iter()
                                .take(10)
                                .chain(std::iter::once(&"... [output truncated] ..."))
                                .chain(stdout_lines.iter().rev().take(5).rev())
                                .copied()
                                .collect::<Vec<_>>()
                                .join("\n");
                            tracing::debug!("Command stdout (abbreviated):\n{}", preview);
                        }
                    }

                    if !step_result.stderr.is_empty() {
                        let stderr_lines: Vec<&str> = step_result.stderr.lines().collect();
                        if stderr_lines.len() <= 20 || tracing::enabled!(tracing::Level::TRACE) {
                            // Show full output if small or at TRACE level
                            tracing::debug!("Command stderr:\n{}", step_result.stderr);
                        } else {
                            // Show abbreviated output at DEBUG level
                            let preview: String = stderr_lines
                                .iter()
                                .take(10)
                                .chain(std::iter::once(&"... [output truncated] ..."))
                                .chain(stderr_lines.iter().rev().take(5).rev())
                                .copied()
                                .collect::<Vec<_>>()
                                .join("\n");
                            tracing::debug!("Command stderr (abbreviated):\n{}", preview);
                        }
                    }
                }

                // Complete command timing
                let command_duration = command_start.elapsed();
                let step_completed_at = chrono::Utc::now();
                if let Some((cmd_name, _)) = self.timing_tracker.complete_command() {
                    self.session_manager
                        .update_session(SessionUpdate::RecordCommandTiming(
                            cmd_name.clone(),
                            command_duration,
                        ))
                        .await?;
                }

                // Track the completed step with output
                let completed_step = crate::cook::session::StepResult {
                    step_index,
                    command: step_display.clone(),
                    success: step_result.success,
                    output: if step.capture_output.is_enabled() {
                        Some(step_result.stdout.clone())
                    } else {
                        None
                    },
                    duration: command_duration,
                    error: if !step_result.success {
                        Some(step_result.stderr.clone())
                    } else {
                        None
                    },
                    started_at: step_started_at,
                    completed_at: step_completed_at,
                    exit_code: step_result.exit_code,
                };
                self.completed_steps.push(completed_step.clone());

                // Also track for checkpoint system
                let checkpoint_step = CheckpointCompletedStep {
                    step_index,
                    command: step_display.clone(),
                    success: step_result.success,
                    output: if step.capture_output.is_enabled() {
                        Some(step_result.stdout.clone())
                    } else {
                        None
                    },
                    captured_variables: workflow_context.captured_outputs.clone(),
                    duration: command_duration,
                    completed_at: step_completed_at,
                    retry_state: None,
                };
                self.checkpoint_completed_steps.push(checkpoint_step);

                // Save checkpoint if manager is available
                if let Some(ref checkpoint_manager) = self.checkpoint_manager {
                    if let Some(ref workflow_id) = self.workflow_id {
                        // Create a normalized workflow for hashing (simplified)
                        let workflow_hash = format!("{:?}", workflow.steps.len());

                        // Build checkpoint
                        let mut checkpoint = create_checkpoint_with_total_steps(
                            workflow_id.clone(),
                            &normalized::NormalizedWorkflow {
                                name: workflow.name.clone(),
                                steps: vec![], // We'd need to convert, but for now use empty
                                execution_mode: normalized::ExecutionMode::Sequential,
                                variables: workflow_context.variables.clone(),
                            },
                            &workflow_context,
                            self.checkpoint_completed_steps.clone(),
                            step_index + 1,
                            workflow_hash,
                            workflow.steps.len(), // Pass the actual total steps count
                        );

                        // Set workflow path if available
                        if let Some(ref path) = self.workflow_path {
                            checkpoint.workflow_path = Some(path.clone());
                        }

                        // Save checkpoint asynchronously
                        if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                            tracing::warn!("Failed to save checkpoint: {}", e);
                        }
                    }
                }

                // Save checkpoint after successful step execution
                let workflow_state = crate::cook::session::WorkflowState {
                    current_iteration: iteration as usize - 1, // Convert to 0-based index
                    current_step: step_index + 1,              // Next step to execute
                    completed_steps: self.completed_steps.clone(), // Use accumulated steps
                    workflow_path: env.working_dir.join("workflow.yml"), // This would need to be passed in
                    input_args: Vec::new(), // Would need to be passed from config
                    map_patterns: Vec::new(), // Would need to be passed from config
                    using_worktree: env.worktree_name.is_some(),
                };
                self.session_manager
                    .update_session(SessionUpdate::UpdateWorkflowState(workflow_state))
                    .await?;

                // Check for commits if required
                if let Some(before) = head_before {
                    let head_after = self.get_current_head(&env.working_dir).await?;
                    if head_after == before {
                        // No commits were created - check if auto-commit is enabled
                        if step.auto_commit {
                            // Try to create an auto-commit
                            if let Ok(has_changes) = self.check_for_changes(&env.working_dir).await
                            {
                                if has_changes {
                                    let message =
                                        self.generate_commit_message(step, &workflow_context);
                                    if let Err(e) =
                                        self.create_auto_commit(&env.working_dir, &message).await
                                    {
                                        tracing::warn!("Failed to create auto-commit: {}", e);
                                        if step.commit_required {
                                            self.handle_no_commits_error(step)?;
                                        }
                                    } else {
                                        any_changes = true;
                                        self.user_interaction.display_success(&format!(
                                            "{step_display} auto-committed changes"
                                        ));
                                    }
                                } else if step.commit_required {
                                    self.handle_no_commits_error(step)?;
                                }
                            } else if step.commit_required {
                                self.handle_no_commits_error(step)?;
                            }
                        } else if step.commit_required {
                            self.handle_no_commits_error(step)?;
                        }
                    } else {
                        any_changes = true;
                        // Track commit metadata if available
                        if let Ok(commits) = self
                            .get_commits_between(&env.working_dir, &before, &head_after)
                            .await
                        {
                            let commit_count = commits.len();
                            let files_changed: std::collections::HashSet<_> = commits
                                .iter()
                                .flat_map(|c| c.files_changed.iter())
                                .collect();
                            self.user_interaction.display_success(&format!(
                                "{step_display} created {} commit{} affecting {} file{}",
                                commit_count,
                                if commit_count == 1 { "" } else { "s" },
                                files_changed.len(),
                                if files_changed.len() == 1 { "" } else { "s" }
                            ));

                            // Store commit info in context for later use
                            workflow_context.variables.insert(
                                "step.commits".to_string(),
                                commits
                                    .iter()
                                    .map(|c| &c.hash)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(","),
                            );
                            workflow_context.variables.insert(
                                "step.files_changed".to_string(),
                                files_changed
                                    .into_iter()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .collect::<Vec<_>>()
                                    .join(","),
                            );
                        } else {
                            self.user_interaction
                                .display_success(&format!("{step_display} created commits"));
                        }
                    }
                } else {
                    // In test mode or when commit_required is false
                    if step_result.success {
                        any_changes = true;
                    } else if test_mode && step.commit_required && !skip_validation {
                        // In test mode, if no changes were made and commits were required, fail
                        self.handle_no_commits_error(step)?;
                    }
                }
            }

            // Check if we should continue
            if workflow.iterate {
                if !any_changes {
                    self.user_interaction
                        .display_info("No changes were made - stopping early");
                    should_continue = false;
                } else if self.is_focus_tracking_test() {
                    // In focus tracking test, continue for all iterations
                    should_continue = iteration < workflow.max_iterations;
                } else if test_mode {
                    // In test mode, check for early termination
                    should_continue = !self.should_stop_early_in_test_mode();
                } else {
                    // Check based on metrics or ask user
                    should_continue = self.should_continue_iterations(env).await?;
                }
            } else {
                // Single iteration workflow
                should_continue = false;
            }

            // Complete iteration timing
            if let Some(iteration_duration) = self.timing_tracker.complete_iteration() {
                self.session_manager
                    .update_session(SessionUpdate::CompleteIteration)
                    .await?;

                // Display iteration timing
                self.user_interaction.display_success(&format!(
                    "Iteration {} completed in {}",
                    iteration,
                    format_duration(iteration_duration)
                ));
            }

            // Analysis between iterations removed in this version
            // if should_continue && workflow.analyze_between {
            //     self.user_interaction
            //         .display_progress("Running analysis between iterations...");
            //     let analysis = self
            //         .analysis_coordinator
            //         .analyze_project(&env.working_dir)
            //         .await?;
            //     self.analysis_coordinator
            //         .save_analysis(&env.working_dir, &analysis)
            //         .await?;
            // }
        }

        // Metrics collection removed in v0.3.0

        // Check if any step has squash enabled in commit_config
        let should_squash = workflow.steps.iter().any(|step| {
            step.commit_config
                .as_ref()
                .map(|config| config.squash)
                .unwrap_or(false)
        });

        // If squash is enabled, squash all commits at the end of workflow
        if should_squash && any_changes {
            self.user_interaction
                .display_progress("Squashing workflow commits...");

            // Try to get all commits created during this workflow
            if let Ok(head_after) = self.get_current_head(&env.working_dir).await {
                // Use a reasonable range for getting commits (last 20 commits should be enough for a workflow)
                if let Ok(commits) = self
                    .get_commits_between(&env.working_dir, "HEAD~20", &head_after)
                    .await
                {
                    if !commits.is_empty() {
                        // Create commit tracker and squash
                        let git_ops = Arc::new(crate::abstractions::git::RealGitOperations::new());
                        let commit_tracker = crate::cook::commit_tracker::CommitTracker::new(
                            git_ops,
                            env.working_dir.clone(),
                        );

                        // Generate squash message
                        let squash_message = format!(
                            "Squashed {} workflow commits from {}",
                            commits.len(),
                            workflow.name
                        );

                        if let Err(e) = commit_tracker
                            .squash_commits(&commits, &squash_message)
                            .await
                        {
                            tracing::warn!("Failed to squash commits: {}", e);
                        } else {
                            self.user_interaction.display_success(&format!(
                                "Squashed {} commits into one",
                                commits.len()
                            ));
                        }
                    }
                }
            }
        }

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total workflow time",
            &format!(
                "{} across {} iteration{}",
                format_duration(total_duration),
                iteration,
                if iteration == 1 { "" } else { "s" }
            ),
        );

        Ok(())
    }

    /// Prepare environment variables for step execution
    fn prepare_env_vars(
        &self,
        step: &WorkflowStep,
        _env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        // Add automation flag
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Add step-specific environment variables with interpolation
        for (key, value) in &step.env {
            let (interpolated_value, resolutions) = ctx.interpolate_with_tracking(value);
            if !resolutions.is_empty() {
                tracing::debug!("   📊 Environment variable {} resolved:", key);
                self.log_variable_resolutions(&resolutions);
            }
            env_vars.insert(key.clone(), interpolated_value);
        }

        env_vars
    }

    /// Execute a single workflow step
    pub async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Display what step we're executing
        let step_name = self.get_step_display_name(step);
        self.user_interaction
            .display_progress(&format!("Executing: {}", step_name));

        // Log verbose execution context
        tracing::info!("=== Step Execution Context ===");
        tracing::info!("Step: {}", step_name);
        tracing::info!("Working Directory: {}", env.working_dir.display());
        tracing::info!("Project Directory: {}", env.project_dir.display());
        if let Some(ref worktree) = env.worktree_name {
            tracing::info!("Worktree: {}", worktree);
        }
        tracing::info!("Session ID: {}", env.session_id);

        // Log variables if any
        if !ctx.variables.is_empty() {
            tracing::info!("Variables:");
            for (key, value) in &ctx.variables {
                // Truncate long values for readability
                let display_value = if value.len() > 100 {
                    format!("{}... (truncated)", &value[..100])
                } else {
                    value.clone()
                };
                tracing::info!("  {} = {}", key, display_value);
            }
        }

        // Log captured outputs if any
        if !ctx.captured_outputs.is_empty() {
            tracing::info!("Captured Outputs:");
            for (key, value) in &ctx.captured_outputs {
                let display_value = if value.len() > 100 {
                    format!("{}... (truncated)", &value[..100])
                } else {
                    value.clone()
                };
                tracing::info!("  {} = {}", key, display_value);
            }
        }

        // Initialize CommitTracker for this step using the executor's git operations (enables mocking)
        let git_ops = self.git_operations.clone();
        let working_dir = env.working_dir.clone();
        let mut commit_tracker =
            crate::cook::commit_tracker::CommitTracker::new(git_ops, working_dir);
        commit_tracker.initialize().await?;

        // Get the HEAD before step execution
        let before_head = commit_tracker.get_current_head().await?;

        // Determine command type
        let command_type = self.determine_command_type(step)?;

        // Set up environment for this step
        let (env_vars, working_dir_override) =
            if let Some(ref mut env_manager) = self.environment_manager {
                // Use environment manager to set up step environment
                let env_context = env_manager
                    .setup_step_environment(
                        step,
                        self.global_environment_config.as_ref(),
                        &ctx.variables,
                    )
                    .await?;

                // Update working directory if overridden
                let working_dir_override = if env_context.working_dir != env.working_dir {
                    Some(env_context.working_dir.clone())
                } else {
                    None
                };

                (env_context.env, working_dir_override)
            } else {
                // Fall back to traditional environment preparation
                let env_vars = self.prepare_env_vars(step, env, ctx);
                let working_dir_override = step.working_dir.clone();
                (env_vars, working_dir_override)
            };

        // Update execution environment if working directory is overridden
        let mut actual_env = env.clone();
        if let Some(ref dir) = working_dir_override {
            actual_env.working_dir = dir.clone();
            tracing::info!("Working directory overridden to: {}", dir.display());
        }

        // Log environment variables being set
        if !env_vars.is_empty() {
            tracing::info!("Environment Variables:");
            for (key, value) in &env_vars {
                // Don't log sensitive values
                if key.to_lowercase().contains("secret")
                    || key.to_lowercase().contains("token")
                    || key.to_lowercase().contains("password")
                    || key.to_lowercase().contains("key")
                {
                    tracing::info!("  {} = <redacted>", key);
                } else {
                    let display_value = if value.len() > 100 {
                        format!("{}... (truncated)", &value[..100])
                    } else {
                        value.clone()
                    };
                    tracing::info!("  {} = {}", key, display_value);
                }
            }
        }

        tracing::info!(
            "Actual execution directory: {}",
            actual_env.working_dir.display()
        );
        tracing::info!("==============================");

        // Handle test mode
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return self.handle_test_mode_execution(step, &command_type);
        }

        // Execute the command with retry if configured
        let mut result = if let Some(retry_config) = &step.retry {
            // Use enhanced retry executor
            let step_name = self.get_step_display_name(step);
            self.execute_with_enhanced_retry(
                retry_config.clone(),
                &step_name,
                &command_type,
                step,
                &actual_env,
                ctx,
                env_vars,
            )
            .await?
        } else {
            // Execute without enhanced retry
            self.execute_command_by_type(&command_type, step, &actual_env, ctx, env_vars)
                .await?
        };

        // Track commits created during step execution
        let after_head = commit_tracker.get_current_head().await?;
        let step_name = self.get_step_display_name(step);
        let mut tracked_commits = commit_tracker
            .track_step_commits(&step_name, &before_head, &after_head)
            .await?;

        // Create auto-commit if configured and changes exist
        if step.auto_commit && commit_tracker.has_changes().await? {
            let message_template = step
                .commit_config
                .as_ref()
                .and_then(|c| c.message_template.as_deref());
            let auto_commit = commit_tracker
                .create_auto_commit(
                    &step_name,
                    message_template,
                    &ctx.variables,
                    step.commit_config.as_ref(),
                )
                .await?;

            // Add auto-commit to tracked commits
            tracked_commits.push(auto_commit);
        }

        // Populate commit variables in context if we have commits
        if !tracked_commits.is_empty() {
            let tracking_result = crate::cook::commit_tracker::CommitTrackingResult::from_commits(
                tracked_commits.clone(),
            );
            ctx.variables.insert(
                "step.commits".to_string(),
                serde_json::to_string(&tracked_commits)?,
            );
            ctx.variables.insert(
                "step.files_changed".to_string(),
                tracking_result.total_files_changed.to_string(),
            );
            ctx.variables.insert(
                "step.insertions".to_string(),
                tracking_result.total_insertions.to_string(),
            );
            ctx.variables.insert(
                "step.deletions".to_string(),
                tracking_result.total_deletions.to_string(),
            );
        }

        // Enforce commit_required if configured
        if step.commit_required && tracked_commits.is_empty() && after_head == before_head {
            return Err(anyhow::anyhow!(
                "Step '{}' has commit_required=true but no commits were created",
                step_name
            ));
        }

        // Capture command output if requested
        if let Some(capture_name) = &step.capture {
            let command_result = super::variables::CommandResult {
                stdout: Some(result.stdout.clone()),
                stderr: Some(result.stderr.clone()),
                exit_code: result.exit_code.unwrap_or(-1),
                success: result.success,
                duration: std::time::Duration::from_secs(0), // TODO: Track actual duration
            };

            let capture_format = step.capture_format.unwrap_or_default();
            let capture_streams = &step.capture_streams;

            ctx.variable_store
                .capture_command_result(
                    capture_name,
                    command_result,
                    capture_format,
                    capture_streams,
                )
                .await
                .map_err(|e| anyhow!("Failed to capture command result: {}", e))?;

            // Also update captured_outputs for backward compatibility
            ctx.captured_outputs
                .insert(capture_name.clone(), result.stdout.clone());
        }

        // Write output to file if requested
        if let Some(output_file) = &step.output_file {
            use std::fs;

            let output_path = if output_file.is_absolute() {
                output_file.clone()
            } else {
                actual_env.working_dir.join(output_file)
            };

            // Create parent directory if needed
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create output directory: {}", e))?;
            }

            // Write output to file
            fs::write(&output_path, &result.stdout)
                .map_err(|e| anyhow!("Failed to write output to file {:?}: {}", output_path, e))?;
        }

        // Capture output if requested (deprecated)
        if step.capture_output.is_enabled() {
            // Get the variable name for this output (custom or default)
            if let Some(var_name) = step.capture_output.get_variable_name(&command_type) {
                // Store with the specified variable name
                ctx.captured_outputs.insert(var_name, result.stdout.clone());
            }

            // Also store as generic CAPTURED_OUTPUT for backward compatibility
            ctx.captured_outputs
                .insert("CAPTURED_OUTPUT".to_string(), result.stdout.clone());
        }

        // Handle validation if configured and command succeeded
        if result.success {
            if let Some(validation_config) = &step.validate {
                self.handle_validation(validation_config, &actual_env, ctx)
                    .await?;
            }

            // Handle step validation (first-class validation feature)
            if let Some(step_validation) = &step.step_validate {
                if !step.skip_validation {
                    let validation_result = self
                        .handle_step_validation(step_validation, &actual_env, ctx, step)
                        .await?;

                    // Update result based on validation
                    if !validation_result.passed && !step.ignore_validation_failure {
                        result.success = false;
                        result.stdout.push_str(&format!(
                            "\n[Validation Failed: {} validation(s) executed, {} attempt(s) made]",
                            validation_result.results.len(),
                            validation_result.attempts
                        ));
                        if result.exit_code == Some(0) {
                            result.exit_code = Some(1); // Set exit code to indicate validation failure
                        }
                    }
                }
            }
        }

        // Handle conditional execution (failure, success, exit codes)
        result = self
            .handle_conditional_execution(step, result, &actual_env, ctx)
            .await?;

        // Check if we should fail the workflow based on the result
        let should_fail = self.should_fail_workflow(&result, step);

        if should_fail {
            let error_msg = self.build_error_message(step, &result);
            anyhow::bail!(error_msg);
        }

        // If the command failed but we're not failing the workflow (should_fail is false),
        // we need to modify the result to indicate success so the workflow continues
        if !result.success && !should_fail {
            result.success = true;
            result.stdout.push_str(
                "\n[Note: Command failed but workflow continues due to on_failure configuration]",
            );
        }

        // Count files changed
        self.session_manager
            .update_session(SessionUpdate::AddFilesChanged(1))
            .await?;

        Ok(result)
    }

    /// Execute a modular handler command
    async fn execute_handler_command(
        &self,
        handler_name: String,
        mut attributes: HashMap<String, AttributeValue>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        // Check if command registry is available
        let registry = self.command_registry.as_ref().ok_or_else(|| {
            anyhow!("Command registry not initialized. Call with_command_registry() first.")
        })?;

        // Create execution context for the handler
        let mut exec_context = ExecutionContext::new(env.working_dir.clone());
        exec_context.add_env_vars(env_vars);

        // Add session information if available
        if let Some(session_id) = ctx.variables.get("SESSION_ID") {
            exec_context = exec_context.with_session_id(session_id.clone());
        }
        if let Some(iteration) = ctx.iteration_vars.get("ITERATION") {
            if let Ok(iter_num) = iteration.parse::<usize>() {
                exec_context = exec_context.with_iteration(iter_num);
            }
        }

        // Interpolate attribute values and track resolutions
        let mut all_resolutions = Vec::new();
        for (attr_name, value) in attributes.iter_mut() {
            if let AttributeValue::String(s) = value {
                let (interpolated, resolutions) = ctx.interpolate_with_tracking(s);
                if !resolutions.is_empty() {
                    tracing::debug!("   📊 Handler attribute '{}' variables:", attr_name);
                    all_resolutions.extend(resolutions);
                }
                *s = interpolated;
            }
        }
        self.log_variable_resolutions(&all_resolutions);

        // Execute the handler
        let result = registry
            .execute(&handler_name, &exec_context, attributes)
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

    /// Execute a Claude command
    pub(crate) async fn execute_claude_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        let result = self
            .claude_executor
            .execute_claude_command(command, &env.working_dir, env_vars)
            .await?;

        Ok(StepResult {
            success: result.success,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
        })
    }

    /// Execute a shell command
    pub(crate) async fn execute_shell_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        use tokio::process::Command;
        use tokio::time::{timeout as tokio_timeout, Duration};

        // Log shell command execution details
        tracing::info!("Executing shell command: {}", command);
        tracing::info!("  Working directory: {}", env.working_dir.display());
        if !env_vars.is_empty() {
            tracing::debug!("  With {} environment variables set", env_vars.len());
        }

        // Create command (Unix-like systems only)
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);

        // Set working directory
        cmd.current_dir(&env.working_dir);

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Execute with optional timeout
        let output = if let Some(timeout_secs) = timeout {
            let duration = Duration::from_secs(timeout_secs);
            match tokio_timeout(duration, cmd.output()).await {
                Ok(result) => result?,
                Err(_) => {
                    return Ok(StepResult {
                        success: false,
                        exit_code: Some(-1),
                        stdout: String::new(),
                        stderr: format!("Command timed out after {timeout_secs} seconds"),
                    });
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

    /// Execute a shell command with retry logic (for shell commands with on_failure)
    async fn execute_shell_with_retry(
        &self,
        command: &str,
        on_failure: Option<&crate::config::command::TestDebugConfig>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        use std::fs;
        use tempfile::NamedTempFile;

        let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(command);
        self.log_variable_resolutions(&resolutions);

        // Execute the shell command with retry logic
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.user_interaction.display_progress(&format!(
                "Running shell command (attempt {attempt}): {interpolated_cmd}"
            ));

            // Add attempt number to environment
            env_vars.insert("SHELL_ATTEMPT".to_string(), attempt.to_string());

            // Execute the shell command
            let shell_result = self
                .execute_shell_command(&interpolated_cmd, env, env_vars.clone(), timeout)
                .await?;

            // Check if command succeeded
            if shell_result.success {
                self.user_interaction
                    .display_success(&format!("Shell command succeeded on attempt {attempt}"));
                return Ok(shell_result);
            }

            // Command failed - check if we should retry
            if let Some(debug_config) = on_failure {
                if attempt >= debug_config.max_attempts {
                    self.user_interaction.display_error(&format!(
                        "Shell command failed after {} attempts",
                        debug_config.max_attempts
                    ));

                    if debug_config.fail_workflow {
                        return Err(anyhow!(
                            "Shell command failed after {} attempts and fail_workflow is true",
                            debug_config.max_attempts
                        ));
                    } else {
                        // Return the last result
                        return Ok(shell_result);
                    }
                }

                // Save shell output to a temp file if it's too large
                let temp_file = if shell_result.stdout.len() + shell_result.stderr.len() > 10000 {
                    // Create a temporary file for large outputs
                    let temp_file = NamedTempFile::new()?;
                    let combined_output = format!(
                        "=== STDOUT ===\n{}\n\n=== STDERR ===\n{}",
                        shell_result.stdout, shell_result.stderr
                    );
                    fs::write(temp_file.path(), &combined_output)?;
                    Some(temp_file)
                } else {
                    None
                };

                let output_path = temp_file
                    .as_ref()
                    .map(|f| f.path().to_string_lossy().to_string());

                // Prepare the debug command with variables
                let mut debug_cmd = debug_config.claude.clone();

                // Add shell-specific variables to context
                ctx.variables
                    .insert("shell.attempt".to_string(), attempt.to_string());
                ctx.variables.insert(
                    "shell.exit_code".to_string(),
                    shell_result.exit_code.unwrap_or(-1).to_string(),
                );

                if let Some(output_file) = output_path {
                    ctx.variables
                        .insert("shell.output".to_string(), output_file);
                } else {
                    // For smaller outputs, pass directly
                    let combined_output = format!(
                        "STDOUT:\n{}\n\nSTDERR:\n{}",
                        shell_result.stdout, shell_result.stderr
                    );
                    ctx.variables
                        .insert("shell.output".to_string(), combined_output);
                }

                // Interpolate the debug command
                let (interpolated_debug_cmd, debug_resolutions) =
                    ctx.interpolate_with_tracking(&debug_cmd);
                self.log_variable_resolutions(&debug_resolutions);
                debug_cmd = interpolated_debug_cmd;

                // Log the actual command being run
                self.user_interaction.display_info(&format!(
                    "Shell command failed, running: {} (attempt {}/{})",
                    debug_cmd, attempt, debug_config.max_attempts
                ));

                // Execute the debug command
                let debug_result = self
                    .execute_claude_command(&debug_cmd, env, env_vars.clone())
                    .await?;

                if !debug_result.success {
                    self.user_interaction
                        .display_error("Debug command failed, but continuing with retry");
                }

                // Clean up temp file
                drop(temp_file);

                // Continue to next attempt
            } else {
                // No on_failure configuration, return the failed result
                return Ok(shell_result);
            }
        }
    }

    /// Execute a test command with retry logic
    async fn execute_test_command(
        &self,
        test_cmd: crate::config::command::TestCommand,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
        _workflow: Option<&NormalizedWorkflow>,
        _step_index: Option<usize>,
    ) -> Result<StepResult> {
        use std::fs;
        use tempfile::NamedTempFile;

        let (interpolated_test_cmd, resolutions) = ctx.interpolate_with_tracking(&test_cmd.command);
        self.log_variable_resolutions(&resolutions);

        // Track failure history for retry state
        let mut failure_history: Vec<String> = Vec::new();
        let _max_attempts = test_cmd
            .on_failure
            .as_ref()
            .map(|f| f.max_attempts)
            .unwrap_or(1);

        // First, execute the test command
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.user_interaction.display_progress(&format!(
                "Running test command (attempt {attempt}): {interpolated_test_cmd}"
            ));

            // Add test-specific variables
            env_vars.insert("TEST_ATTEMPT".to_string(), attempt.to_string());

            // Execute the test command
            let test_result = self
                .execute_shell_command(&interpolated_test_cmd, env, env_vars.clone(), None)
                .await?;

            // Check if tests passed
            if test_result.success {
                self.user_interaction
                    .display_success(&format!("Tests passed on attempt {attempt}"));
                return Ok(test_result);
            }

            // Tests failed - check if we should retry
            if let Some(debug_config) = &test_cmd.on_failure {
                // Add failure to history
                failure_history.push(format!(
                    "Attempt {}: exit code {}",
                    attempt,
                    test_result.exit_code.unwrap_or(-1)
                ));

                if attempt >= debug_config.max_attempts {
                    self.user_interaction.display_error(&format!(
                        "Tests failed after {} attempts",
                        debug_config.max_attempts
                    ));

                    if debug_config.fail_workflow {
                        return Err(anyhow!(
                            "Test command failed after {} attempts and fail_workflow is true",
                            debug_config.max_attempts
                        ));
                    } else {
                        // Return the last test result
                        return Ok(test_result);
                    }
                }

                // Save checkpoint after test failure but before retry
                if let (Some(workflow), Some(step_index)) =
                    (&self.current_workflow, self.current_step_index)
                {
                    let retry_state = checkpoint::RetryState {
                        current_attempt: attempt as usize,
                        max_attempts: debug_config.max_attempts as usize,
                        failure_history: failure_history.clone(),
                        in_retry_loop: true,
                    };
                    self.save_retry_checkpoint(workflow, step_index, Some(retry_state), ctx)
                        .await;
                    tracing::info!(
                        "Saved checkpoint for test retry at attempt {}/{}",
                        attempt,
                        debug_config.max_attempts
                    );
                }

                // Save test output to a temp file if it's too large
                // We need to keep the temp file alive until after the debug command runs
                let temp_file = if test_result.stdout.len() + test_result.stderr.len() > 10000 {
                    // Create a temporary file for large outputs
                    let temp_file = NamedTempFile::new()?;
                    let combined_output = format!(
                        "=== STDOUT ===\n{}\n\n=== STDERR ===\n{}",
                        test_result.stdout, test_result.stderr
                    );
                    fs::write(temp_file.path(), &combined_output)?;
                    Some(temp_file)
                } else {
                    None
                };

                let output_path = temp_file
                    .as_ref()
                    .map(|f| f.path().to_string_lossy().to_string());

                // Prepare the debug command with variables
                let mut debug_cmd = debug_config.claude.clone();

                // Add test-specific variables to context
                ctx.variables
                    .insert("test.attempt".to_string(), attempt.to_string());
                ctx.variables.insert(
                    "test.exit_code".to_string(),
                    test_result.exit_code.unwrap_or(-1).to_string(),
                );

                if let Some(output_file) = output_path {
                    ctx.variables.insert("test.output".to_string(), output_file);
                } else {
                    // For smaller outputs, pass directly
                    let combined_output = format!(
                        "STDOUT:\n{}\n\nSTDERR:\n{}",
                        test_result.stdout, test_result.stderr
                    );
                    ctx.variables
                        .insert("test.output".to_string(), combined_output);
                }

                // Interpolate the debug command
                let (interpolated_debug_cmd, debug_resolutions) =
                    ctx.interpolate_with_tracking(&debug_cmd);
                self.log_variable_resolutions(&debug_resolutions);
                debug_cmd = interpolated_debug_cmd;

                // Log the actual command being run
                self.user_interaction.display_info(&format!(
                    "Tests failed, running: {} (attempt {}/{})",
                    debug_cmd, attempt, debug_config.max_attempts
                ));

                // Execute the debug command
                let debug_result = self
                    .execute_claude_command(&debug_cmd, env, env_vars.clone())
                    .await?;

                // Note: commit verification for debug commands happens at a higher level
                // The debug_config.commit_required field indicates that the command
                // should create commits, which is enforced in the command template

                if !debug_result.success {
                    self.user_interaction
                        .display_error("Debug command failed, but continuing with retry");
                }

                // The temp_file will be dropped here, which is safe because the debug command
                // has already been executed and no longer needs the file
                drop(temp_file);

                // Continue to next attempt
            } else {
                // No on_failure configuration, return the failed result
                return Ok(test_result);
            }
        }
    }

    /// Handle test mode execution
    pub(crate) fn handle_test_mode_execution(
        &self,
        step: &WorkflowStep,
        command_type: &CommandType,
    ) -> Result<StepResult> {
        let command_str = match command_type {
            CommandType::Claude(cmd) => format!("Claude command: {cmd}"),
            CommandType::Shell(cmd) => format!("Shell command: {cmd}"),
            CommandType::Test(test_cmd) => format!("Test command: {}", test_cmd.command),
            CommandType::Legacy(cmd) => format!("Legacy command: {cmd}"),
            CommandType::Handler { handler_name, .. } => format!("Handler command: {handler_name}"),
            CommandType::GoalSeek(config) => format!("Goal-seek command: {}", config.goal),
            CommandType::Foreach(config) => {
                let item_count = match &config.input {
                    crate::config::command::ForeachInput::List(items) => items.len(),
                    crate::config::command::ForeachInput::Command(_) => 0,
                };
                format!("Foreach command: {} items", item_count)
            }
        };

        println!("[TEST MODE] Would execute {command_str}");

        // Check if we should simulate no changes
        let should_simulate_no_changes = match command_type {
            CommandType::Claude(cmd) | CommandType::Legacy(cmd) => {
                self.is_test_mode_no_changes_command(cmd)
            }
            CommandType::Shell(_) => false,
            CommandType::Test(_) => false,
            CommandType::Handler { .. } => false,
            CommandType::GoalSeek(_) => false,
            CommandType::Foreach(_) => false,
        };

        if should_simulate_no_changes {
            println!("[TEST MODE] Simulating no changes");
            // If this command requires commits but simulates no changes,
            // it should fail UNLESS commit validation is explicitly skipped
            let skip_validation =
                std::env::var("PRODIGY_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";
            if step.commit_required && !skip_validation {
                return Err(anyhow::anyhow!(
                    "No changes were committed by {}",
                    self.get_step_display_name(step)
                ));
            }
            return Ok(StepResult {
                success: true,
                exit_code: Some(0),
                stdout: "[TEST MODE] No changes made".to_string(),
                stderr: String::new(),
            });
        }

        Ok(StepResult {
            success: true,
            exit_code: Some(0),
            stdout: "[TEST MODE] Command executed successfully".to_string(),
            stderr: String::new(),
        })
    }

    /// Get current git HEAD
    async fn get_current_head(&self, working_dir: &std::path::Path) -> Result<String> {
        // We need to run git commands in the correct working directory (especially for worktrees)
        let output = self
            .git_operations
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", working_dir)
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if there are uncommitted changes
    async fn check_for_changes(&self, working_dir: &std::path::Path) -> Result<bool> {
        let output = self
            .git_operations
            .git_command_in_dir(&["status", "--porcelain"], "check status", working_dir)
            .await
            .context("Failed to check git status")?;

        Ok(!output.stdout.is_empty())
    }

    /// Create an auto-commit with the given message
    async fn create_auto_commit(&self, working_dir: &std::path::Path, message: &str) -> Result<()> {
        // Stage all changes
        self.git_operations
            .git_command_in_dir(&["add", "."], "stage changes", working_dir)
            .await
            .context("Failed to stage changes")?;

        // Create commit
        let output = self
            .git_operations
            .git_command_in_dir(&["commit", "-m", message], "create commit", working_dir)
            .await
            .context("Failed to create commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to create commit: {stderr}"));
        }

        Ok(())
    }

    /// Generate a commit message from template or default
    fn generate_commit_message(&self, step: &WorkflowStep, context: &WorkflowContext) -> String {
        if let Some(ref config) = step.commit_config {
            if let Some(ref template) = config.message_template {
                // Interpolate variables in template
                let mut message = template.clone();
                message = message.replace("${step.name}", &self.get_step_display_name(step));

                // Replace other variables from context
                for (key, value) in &context.variables {
                    message = message.replace(&format!("${{{key}}}"), value);
                    message = message.replace(&format!("${key}"), value);
                }

                return message;
            }
        }

        format!("Auto-commit: {}", self.get_step_display_name(step))
    }

    /// Get commits between two refs
    async fn get_commits_between(
        &self,
        working_dir: &std::path::Path,
        from: &str,
        to: &str,
    ) -> Result<Vec<crate::cook::commit_tracker::TrackedCommit>> {
        use crate::cook::commit_tracker::TrackedCommit;
        use chrono::{DateTime, Utc};

        let output = self
            .git_operations
            .git_command_in_dir(
                &[
                    "log",
                    &format!("{from}..{to}"),
                    "--pretty=format:%H|%s|%an|%aI",
                    "--name-only",
                ],
                "get commit log",
                working_dir,
            )
            .await
            .context("Failed to get commit log")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut commits = Vec::new();
        let mut current_commit: Option<TrackedCommit> = None;

        for line in stdout.lines() {
            if line.contains('|') {
                // This is a commit header line
                if let Some(commit) = current_commit.take() {
                    commits.push(commit);
                }

                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    current_commit = Some(TrackedCommit {
                        hash: parts[0].to_string(),
                        message: parts[1].to_string(),
                        author: parts[2].to_string(),
                        timestamp: parts[3]
                            .parse::<DateTime<Utc>>()
                            .unwrap_or_else(|_| Utc::now()),
                        files_changed: Vec::new(),
                        insertions: 0,
                        deletions: 0,
                        step_name: String::new(),
                        agent_id: None,
                    });
                }
            } else if !line.is_empty() {
                // This is a file name
                if let Some(ref mut commit) = current_commit {
                    commit.files_changed.push(std::path::PathBuf::from(line));
                }
            }
        }

        if let Some(commit) = current_commit {
            commits.push(commit);
        }

        Ok(commits)
    }

    /// Handle the case where no commits were created when expected
    pub(crate) fn handle_no_commits_error(&self, step: &WorkflowStep) -> Result<()> {
        let step_display = self.get_step_display_name(step);
        let command_type = self.determine_command_type(step)?;

        let command_name = match &command_type {
            CommandType::Claude(cmd) | CommandType::Legacy(cmd) => cmd
                .trim_start_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or(""),
            CommandType::Shell(cmd) => cmd,
            CommandType::Test(test_cmd) => &test_cmd.command,
            CommandType::Handler { handler_name, .. } => handler_name,
            CommandType::GoalSeek(config) => &config.goal,
            CommandType::Foreach(_) => "foreach",
        };

        eprintln!("\nWorkflow stopped: No changes were committed by {step_display}");
        eprintln!("\nThe command executed successfully but did not create any git commits.");

        // Check if this is a command that might legitimately not create commits
        if matches!(
            command_name,
            "prodigy-lint" | "prodigy-code-review" | "prodigy-analyze"
        ) {
            eprintln!(
                "This may be expected if there were no {} to fix.",
                if command_name == "prodigy-lint" {
                    "linting issues"
                } else if command_name == "prodigy-code-review" {
                    "issues found"
                } else {
                    "changes needed"
                }
            );
            eprintln!("\nTo allow this command to proceed without commits, set commit_required: false in your workflow");
        } else {
            eprintln!("Possible reasons:");
            eprintln!("- The specification may already be implemented");
            eprintln!("- The command may have encountered an issue without reporting an error");
            eprintln!("- No changes were needed");
            eprintln!("\nTo investigate:");
            eprintln!("- Check if the spec is already implemented");
            eprintln!("- Review the command output above for any warnings");
            eprintln!("- Run 'git status' to check for uncommitted changes");
        }

        eprintln!(
            "\nAlternatively, run with PRODIGY_NO_COMMIT_VALIDATION=true to skip all validation."
        );

        Err(anyhow!("No commits created by {}", step_display))
    }

    /// Execute a MapReduce workflow
    async fn execute_mapreduce(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        use crate::cook::execution::setup_executor::SetupPhaseExecutor;
        use crate::cook::execution::{MapReduceExecutor, SetupPhase};
        use crate::worktree::WorktreeManager;

        let workflow_start = Instant::now();

        // Don't duplicate the message - it's already shown by the orchestrator

        let mut workflow_context = WorkflowContext::default();
        let mut generated_input_file: Option<String> = None;
        let mut _captured_variables = HashMap::new();

        // Execute setup phase if present
        if !workflow.steps.is_empty() || workflow.setup_phase.is_some() {
            self.user_interaction
                .display_progress("Running setup phase...");

            // Use provided setup_phase configuration or create a default one
            let setup_phase = if let Some(ref setup) = workflow.setup_phase {
                setup.clone()
            } else if !workflow.steps.is_empty() {
                // For backward compatibility, use default timeout and no capture_outputs
                SetupPhase {
                    commands: workflow.steps.clone(),
                    timeout: 300,                    // 5 minutes default
                    capture_outputs: HashMap::new(), // No variables to capture by default
                }
            } else {
                // No setup phase
                SetupPhase {
                    commands: vec![],
                    timeout: 300,
                    capture_outputs: HashMap::new(),
                }
            };

            if !setup_phase.commands.is_empty() {
                let setup_executor = SetupPhaseExecutor::new(&setup_phase);

                // Execute setup phase with file detection
                let (captured, gen_file) = setup_executor
                    .execute_with_file_detection(
                        &setup_phase.commands,
                        self,
                        env,
                        &mut workflow_context,
                    )
                    .await
                    .map_err(|e| anyhow!("Setup phase failed: {}", e))?;

                _captured_variables = captured;
                generated_input_file = gen_file;
            }

            self.user_interaction
                .display_success("Setup phase completed");
        }

        // Ensure we have map phase configuration
        let mut map_phase = workflow
            .map_phase
            .as_ref()
            .ok_or_else(|| anyhow!("MapReduce workflow requires map phase configuration"))?
            .clone();

        // Update map phase input if setup generated a work-items.json file
        if let Some(generated_file) = generated_input_file {
            map_phase.config.input = generated_file;
        }

        // Create worktree manager
        let worktree_manager = Arc::new(WorktreeManager::new(
            env.project_dir.clone(),
            self.subprocess.clone(),
        )?);

        // Create MapReduce executor
        let mut mapreduce_executor = MapReduceExecutor::new(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.user_interaction.clone(),
            worktree_manager,
            env.working_dir.clone(), // Use working_dir instead of project_dir to handle worktrees correctly
        )
        .await;

        // Start workflow timing in session
        self.session_manager
            .update_session(SessionUpdate::StartWorkflow)
            .await?;

        // Execute MapReduce workflow with setup context
        let results = mapreduce_executor
            .execute_with_context(
                &map_phase,
                workflow.reduce_phase.as_ref(),
                env,
                workflow_context.captured_outputs.clone(),
            )
            .await?;

        // Update session with results
        let successful_count = results
            .iter()
            .filter(|r| matches!(r.status, crate::cook::execution::AgentStatus::Success))
            .count();

        self.session_manager
            .update_session(SessionUpdate::AddFilesChanged(successful_count))
            .await?;

        // Metrics collection removed in v0.3.0

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total MapReduce workflow time",
            &format_duration(total_duration),
        );

        Ok(())
    }

    /// Check if we should continue iterations
    async fn should_continue_iterations(&self, _env: &ExecutionEnvironment) -> Result<bool> {
        // Always continue iterations until max_iterations is reached
        // The iteration loop already handles the max_iterations check
        Ok(true)
    }

    /// Check if we should stop early in test mode
    pub fn should_stop_early_in_test_mode(&self) -> bool {
        // Check if we're configured to simulate no changes
        self.test_config.as_ref().is_some_and(|c| {
            c.no_changes_commands
                .iter()
                .any(|cmd| cmd.trim() == "prodigy-code-review" || cmd.trim() == "prodigy-lint")
        })
    }

    /// Check if this is the focus tracking test
    pub(crate) fn is_focus_tracking_test(&self) -> bool {
        self.test_config.as_ref().is_some_and(|c| c.track_focus)
    }

    /// Check if this is a test mode command that should simulate no changes
    pub fn is_test_mode_no_changes_command(&self, command: &str) -> bool {
        if let Some(config) = &self.test_config {
            let command_name = command.trim_start_matches('/');
            // Extract just the command name, ignoring arguments
            let command_name = command_name
                .split_whitespace()
                .next()
                .unwrap_or(command_name);
            return config
                .no_changes_commands
                .iter()
                .any(|cmd| cmd.trim() == command_name);
        }
        false
    }

    /// Execute validation command and parse results
    async fn execute_validation(
        &self,
        validation_config: &crate::cook::workflow::validation::ValidationConfig,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<crate::cook::workflow::validation::ValidationResult> {
        use crate::cook::workflow::validation::ValidationResult;

        // Execute either claude or shell command
        let result = if let Some(claude_cmd) = &validation_config.claude {
            let (command, resolutions) = ctx.interpolate_with_tracking(claude_cmd);
            self.log_variable_resolutions(&resolutions);
            self.user_interaction
                .display_progress(&format!("Running validation (Claude): {}", command));

            // Execute Claude command for validation
            let env_vars = HashMap::new();
            self.execute_claude_command(&command, env, env_vars).await?
        } else if let Some(shell_cmd) = validation_config
            .shell
            .as_ref()
            .or(validation_config.command.as_ref())
        {
            // Prefer 'shell' field, fall back to 'command' for backward compatibility
            let (command, resolutions) = ctx.interpolate_with_tracking(shell_cmd);
            self.log_variable_resolutions(&resolutions);
            self.user_interaction
                .display_progress(&format!("Running validation (shell): {}", command));

            // Execute shell command
            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_VALIDATION".to_string(), "true".to_string());

            self.execute_shell_command(&command, env, env_vars, validation_config.timeout)
                .await?
        } else {
            return Ok(ValidationResult::failed(
                "No validation command specified".to_string(),
            ));
        };

        if !result.success {
            // Validation command failed
            return Ok(ValidationResult::failed(format!(
                "Validation command failed with exit code: {}",
                result.exit_code.unwrap_or(-1)
            )));
        }

        // If result_file is specified, read from file instead of stdout
        let json_content = if let Some(result_file) = &validation_config.result_file {
            let (interpolated_file, _resolutions) = ctx.interpolate_with_tracking(result_file);
            // No need to log resolutions for result file path
            let file_path = env.working_dir.join(&interpolated_file);

            // Read the validation result from the file
            match tokio::fs::read_to_string(&file_path).await {
                Ok(content) => content,
                Err(e) => {
                    return Ok(ValidationResult::failed(format!(
                        "Failed to read validation result from {}: {}",
                        interpolated_file, e
                    )));
                }
            }
        } else {
            // Use stdout as before
            result.stdout.clone()
        };

        // Try to parse the JSON content
        match ValidationResult::from_json(&json_content) {
            Ok(mut validation) => {
                // Store raw output
                validation.raw_output = Some(result.stdout);
                Ok(validation)
            }
            Err(_) => {
                // If not JSON, treat as simple pass/fail based on exit code
                if result.success {
                    Ok(ValidationResult::complete())
                } else {
                    Ok(ValidationResult::failed(
                        "Validation failed (non-JSON output)".to_string(),
                    ))
                }
            }
        }
    }

    /// Create a workflow step for validation handler
    fn create_validation_handler(
        &self,
        on_incomplete: &crate::cook::workflow::validation::OnIncompleteConfig,
        _ctx: &WorkflowContext,
    ) -> Option<WorkflowStep> {
        // Create a step based on the handler configuration
        if on_incomplete.claude.is_some() || on_incomplete.shell.is_some() {
            Some(WorkflowStep {
                name: None,
                claude: on_incomplete.claude.clone(),
                shell: on_incomplete.shell.clone(),
                test: None,
                goal_seek: None,
                foreach: None,
                command: None,
                handler: None,
                capture: None,
                capture_format: None,
                capture_streams: Default::default(),
                output_file: None,
                timeout: None,
                capture_output: CaptureOutput::Disabled,
                on_failure: None,
                retry: None,
                on_success: None,
                on_exit_code: Default::default(),
                commit_required: on_incomplete.commit_required,
                auto_commit: false,
                commit_config: None,
                working_dir: None,
                env: Default::default(),
                validate: None,
                step_validate: None,
                skip_validation: false,
                validation_timeout: None,
                ignore_validation_failure: false,
                when: None,
            })
        } else {
            None
        }
    }
}

// Implement the WorkflowExecutor trait
#[async_trait::async_trait]
impl super::traits::StepExecutor for WorkflowExecutor {
    async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Call the existing execute_step method
        self.execute_step(step, env, context).await
    }
}

#[async_trait::async_trait]
impl super::traits::WorkflowExecutor for WorkflowExecutor {
    async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Call the existing execute method
        self.execute(workflow, env).await
    }
}

/// Adapter to allow StepValidationExecutor to use WorkflowExecutor for command execution
struct StepValidationCommandExecutor {
    workflow_executor: *mut WorkflowExecutor,
    env: ExecutionEnvironment,
    ctx: WorkflowContext,
}

unsafe impl Send for StepValidationCommandExecutor {}
unsafe impl Sync for StepValidationCommandExecutor {}

#[async_trait::async_trait]
impl crate::cook::execution::CommandExecutor for StepValidationCommandExecutor {
    async fn execute(
        &self,
        command_type: &str,
        args: &[String],
        _context: crate::cook::execution::ExecutionContext,
    ) -> Result<crate::cook::execution::ExecutionResult> {
        // Safety: We ensure the workflow executor pointer is valid during validation
        let executor = unsafe { &mut *self.workflow_executor };

        // Create a workflow step for the validation command
        let step = match command_type {
            "claude" => WorkflowStep {
                claude: Some(args.join(" ")),
                ..Default::default()
            },
            "shell" => WorkflowStep {
                shell: Some(args.join(" ")),
                ..Default::default()
            },
            _ => {
                return Err(anyhow!(
                    "Unsupported validation command type: {}",
                    command_type
                ));
            }
        };

        // Execute the step
        let mut ctx_clone = self.ctx.clone();
        let result = executor
            .execute_step(&step, &self.env, &mut ctx_clone)
            .await?;

        Ok(crate::cook::execution::ExecutionResult {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            success: result.success,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper function to test get_current_head directly without needing a full executor
    #[cfg(test)]
    async fn test_get_current_head(working_dir: &std::path::Path) -> Result<String> {
        use crate::abstractions::git::RealGitOperations;
        let git_ops = RealGitOperations::new();
        let output = git_ops
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", working_dir)
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[test]
    fn test_variable_interpolation_with_tracking() {
        let mut ctx = WorkflowContext::default();
        ctx.variables.insert("ARG".to_string(), "98".to_string());
        ctx.variables
            .insert("USER".to_string(), "alice".to_string());

        let template = "Running command with $ARG and ${USER}";
        let (result, resolutions) = ctx.interpolate_with_tracking(template);

        assert_eq!(result, "Running command with 98 and alice");
        assert_eq!(resolutions.len(), 2);

        // Check resolutions - order may vary due to HashMap iteration
        let arg_resolution = resolutions.iter().find(|r| r.name == "ARG").unwrap();
        assert_eq!(arg_resolution.raw_expression, "$ARG");
        assert_eq!(arg_resolution.resolved_value, "98");

        let user_resolution = resolutions.iter().find(|r| r.name == "USER").unwrap();
        assert_eq!(user_resolution.raw_expression, "${USER}");
        assert_eq!(user_resolution.resolved_value, "alice");
    }

    #[test]
    fn test_variable_interpolation_with_validation_results() {
        let mut ctx = WorkflowContext::default();

        // Add a validation result
        let validation = crate::cook::workflow::validation::ValidationResult {
            completion_percentage: 95.5,
            status: crate::cook::workflow::validation::ValidationStatus::Incomplete,
            implemented: vec![],
            missing: vec!["test coverage".to_string(), "documentation".to_string()],
            gaps: Default::default(),
            raw_output: None,
        };
        ctx.validation_results
            .insert("validation".to_string(), validation);

        let template = "Completion: ${validation.completion}%, missing: ${validation.missing}";
        let (result, resolutions) = ctx.interpolate_with_tracking(template);

        assert_eq!(
            result,
            "Completion: 95.5%, missing: test coverage, documentation"
        );
        assert_eq!(resolutions.len(), 2);
        assert_eq!(resolutions[0].name, "validation.completion");
        assert_eq!(resolutions[0].resolved_value, "95.5");
        assert_eq!(resolutions[1].name, "validation.missing");
        assert_eq!(
            resolutions[1].resolved_value,
            "test coverage, documentation"
        );
    }

    #[test]
    fn test_variable_interpolation_no_variables() {
        let ctx = WorkflowContext::default();
        let template = "No variables here";
        let (result, resolutions) = ctx.interpolate_with_tracking(template);

        assert_eq!(result, "No variables here");
        assert_eq!(resolutions.len(), 0);
    }

    // Minimal mock implementations for tests
    #[cfg(test)]
    mod test_mocks {
        use super::*;
        use crate::cook::execution::{ClaudeExecutor, ExecutionResult};
        use crate::cook::interaction::VerbosityLevel;
        use crate::cook::interaction::{SpinnerHandle, UserInteraction};
        use crate::cook::session::{
            SessionInfo, SessionManager, SessionState, SessionSummary, SessionUpdate,
        };
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::path::Path;

        pub struct MockClaudeExecutor;

        impl MockClaudeExecutor {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl ClaudeExecutor for MockClaudeExecutor {
            async fn execute_claude_command(
                &self,
                _command: &str,
                _project_path: &Path,
                _env_vars: HashMap<String, String>,
            ) -> Result<ExecutionResult> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn check_claude_cli(&self) -> Result<bool> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn get_claude_version(&self) -> Result<String> {
                unreachable!("Not used in format_variable_value tests")
            }
        }

        pub struct MockSessionManager;

        impl MockSessionManager {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl SessionManager for MockSessionManager {
            async fn start_session(&self, _session_id: &str) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn update_session(&self, _update: SessionUpdate) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn complete_session(&self) -> Result<SessionSummary> {
                unreachable!("Not used in format_variable_value tests")
            }

            fn get_state(&self) -> SessionState {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn save_state(&self, _path: &Path) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn load_state(&self, _path: &Path) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn load_session(&self, _session_id: &str) -> Result<SessionState> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn save_checkpoint(&self, _state: &SessionState) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn list_resumable(&self) -> Result<Vec<SessionInfo>> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn get_last_interrupted(&self) -> Result<Option<String>> {
                unreachable!("Not used in format_variable_value tests")
            }
        }

        pub struct MockUserInteraction;

        impl MockUserInteraction {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl UserInteraction for MockUserInteraction {
            async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
                unreachable!("Not used in format_variable_value tests")
            }

            fn display_info(&self, _message: &str) {}
            fn display_warning(&self, _message: &str) {}
            fn display_error(&self, _message: &str) {}
            fn display_success(&self, _message: &str) {}
            fn display_progress(&self, _message: &str) {}
            fn start_spinner(&self, _message: &str) -> Box<dyn SpinnerHandle> {
                struct NoOpSpinner;
                impl SpinnerHandle for NoOpSpinner {
                    fn update_message(&mut self, _message: &str) {}
                    fn success(&mut self, _message: &str) {}
                    fn fail(&mut self, _message: &str) {}
                }
                Box::new(NoOpSpinner)
            }
            fn display_action(&self, _message: &str) {}
            fn display_metric(&self, _label: &str, _value: &str) {}
            fn display_status(&self, _message: &str) {}
            fn iteration_start(&self, _current: u32, _total: u32) {}
            fn iteration_end(&self, _current: u32, _duration: std::time::Duration, _success: bool) {
            }
            fn step_start(&self, _step: u32, _total: u32, _description: &str) {}
            fn step_end(&self, _step: u32, _success: bool) {}
            fn command_output(&self, _output: &str, _verbosity: VerbosityLevel) {}
            fn debug_output(&self, _message: &str, _min_verbosity: VerbosityLevel) {}
            fn verbosity(&self) -> VerbosityLevel {
                VerbosityLevel::Normal
            }
        }
    }

    #[test]
    fn test_format_variable_value_short_string() {
        use self::test_mocks::{MockClaudeExecutor, MockSessionManager, MockUserInteraction};
        use std::sync::Arc;

        let executor = WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        );

        let value = "simple value";
        let formatted = executor.format_variable_value(value);
        assert_eq!(formatted, "\"simple value\"");
    }

    #[test]
    fn test_format_variable_value_json_array() {
        use self::test_mocks::{MockClaudeExecutor, MockSessionManager, MockUserInteraction};
        use std::sync::Arc;

        let executor = WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        );

        let value = r#"["item1", "item2", "item3"]"#;
        let formatted = executor.format_variable_value(value);
        assert_eq!(formatted, r#"["item1","item2","item3"]"#);
    }

    #[test]
    fn test_format_variable_value_large_array() {
        use self::test_mocks::{MockClaudeExecutor, MockSessionManager, MockUserInteraction};
        use std::sync::Arc;

        let executor = WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        );

        // Create a large array
        let items: Vec<String> = (0..100).map(|i| format!("\"item{}\"", i)).collect();
        let value = format!("[{}]", items.join(","));
        let formatted = executor.format_variable_value(&value);
        assert!(formatted.contains("...100 items..."));
    }

    #[test]
    fn test_format_variable_value_json_object() {
        use self::test_mocks::{MockClaudeExecutor, MockSessionManager, MockUserInteraction};
        use std::sync::Arc;

        let executor = WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        );

        let value = r#"{"name": "test", "value": 42}"#;
        let formatted = executor.format_variable_value(value);
        // Should be pretty-printed
        assert!(formatted.contains("name"));
        assert!(formatted.contains("test"));
        assert!(formatted.contains("value"));
        assert!(formatted.contains("42"));
    }

    #[test]
    fn test_format_variable_value_truncated() {
        use self::test_mocks::{MockClaudeExecutor, MockSessionManager, MockUserInteraction};
        use std::sync::Arc;

        let executor = WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        );

        let value = "a".repeat(300);
        let formatted = executor.format_variable_value(&value);
        assert!(formatted.contains("...\" (showing first 200 chars)"));
        assert!(formatted.starts_with("\""));
    }

    #[test]
    fn test_json_to_attribute_value_static_string() {
        let json = serde_json::json!("hello world");
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::String("hello world".to_string()));
    }

    #[test]
    fn test_json_to_attribute_value_static_integer() {
        let json = serde_json::json!(42);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(42.0));
    }

    #[test]
    fn test_json_to_attribute_value_static_float() {
        let json = serde_json::json!(123.456);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(123.456));
    }

    #[test]
    fn test_json_to_attribute_value_static_boolean_true() {
        let json = serde_json::json!(true);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Boolean(true));
    }

    #[test]
    fn test_json_to_attribute_value_static_boolean_false() {
        let json = serde_json::json!(false);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Boolean(false));
    }

    #[test]
    fn test_json_to_attribute_value_static_null() {
        let json = serde_json::json!(null);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Null);
    }

    #[test]
    fn test_json_to_attribute_value_static_array() {
        let json = serde_json::json!([1, "two", true, null]);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(
            result,
            AttributeValue::Array(vec![
                AttributeValue::Number(1.0),
                AttributeValue::String("two".to_string()),
                AttributeValue::Boolean(true),
                AttributeValue::Null,
            ])
        );
    }

    #[test]
    fn test_json_to_attribute_value_static_nested_array() {
        let json = serde_json::json!([[1, 2], [3, 4]]);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(
            result,
            AttributeValue::Array(vec![
                AttributeValue::Array(vec![
                    AttributeValue::Number(1.0),
                    AttributeValue::Number(2.0),
                ]),
                AttributeValue::Array(vec![
                    AttributeValue::Number(3.0),
                    AttributeValue::Number(4.0),
                ]),
            ])
        );
    }

    #[test]
    fn test_json_to_attribute_value_static_object() {
        let json = serde_json::json!({
            "name": "test",
            "count": 42,
            "active": true,
            "data": null
        });
        let result = WorkflowExecutor::json_to_attribute_value_static(json);

        if let AttributeValue::Object(map) = result {
            assert_eq!(
                map.get("name"),
                Some(&AttributeValue::String("test".to_string()))
            );
            assert_eq!(map.get("count"), Some(&AttributeValue::Number(42.0)));
            assert_eq!(map.get("active"), Some(&AttributeValue::Boolean(true)));
            assert_eq!(map.get("data"), Some(&AttributeValue::Null));
            assert_eq!(map.len(), 4);
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_json_to_attribute_value_static_nested_object() {
        let json = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30
            },
            "settings": {
                "theme": "dark",
                "notifications": true
            }
        });
        let result = WorkflowExecutor::json_to_attribute_value_static(json);

        if let AttributeValue::Object(map) = result {
            // Check user object
            if let Some(AttributeValue::Object(user_map)) = map.get("user") {
                assert_eq!(
                    user_map.get("name"),
                    Some(&AttributeValue::String("Alice".to_string()))
                );
                assert_eq!(user_map.get("age"), Some(&AttributeValue::Number(30.0)));
            } else {
                panic!("Expected user to be an Object");
            }

            // Check settings object
            if let Some(AttributeValue::Object(settings_map)) = map.get("settings") {
                assert_eq!(
                    settings_map.get("theme"),
                    Some(&AttributeValue::String("dark".to_string()))
                );
                assert_eq!(
                    settings_map.get("notifications"),
                    Some(&AttributeValue::Boolean(true))
                );
            } else {
                panic!("Expected settings to be an Object");
            }
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_json_to_attribute_value_static_large_numbers() {
        // Test large integer
        let json = serde_json::json!(9007199254740991i64); // Max safe integer in JavaScript
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(9007199254740991.0));

        // Test negative integer
        let json = serde_json::json!(-42);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(-42.0));
    }

    #[tokio::test]
    async fn test_get_current_head_in_regular_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to init git repo");

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to set git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to set git name");

        // Create initial commit
        std::fs::write(repo_path.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("Failed to stage files");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to create commit");

        // Test get_current_head
        let head = test_get_current_head(repo_path).await.unwrap();
        assert!(!head.is_empty());
        assert_eq!(head.len(), 40); // SHA-1 hash is 40 characters
    }

    #[tokio::test]
    async fn test_get_current_head_in_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let main_repo = temp_dir.path().join("main");
        let worktree_path = temp_dir.path().join("worktree");

        // Create main repo
        std::fs::create_dir(&main_repo).unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to init git repo");

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to set git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to set git name");

        // Create initial commit in main repo
        std::fs::write(main_repo.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to stage files");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to create commit");

        // Create worktree
        std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                "test-branch",
            ])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to create worktree");

        // Make a commit in the worktree
        std::fs::write(worktree_path.join("worktree.txt"), "worktree content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to stage files in worktree");

        std::process::Command::new("git")
            .args(["commit", "-m", "Worktree commit"])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to create commit in worktree");

        // Test get_current_head in worktree
        let worktree_head = test_get_current_head(&worktree_path).await.unwrap();
        assert!(!worktree_head.is_empty());
        assert_eq!(worktree_head.len(), 40);

        // Get main repo head
        let main_head = test_get_current_head(&main_repo).await.unwrap();

        // Heads should be different
        assert_ne!(
            worktree_head, main_head,
            "Worktree HEAD should differ from main repo HEAD"
        );
    }

    #[tokio::test]
    async fn test_get_current_head_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let non_git_dir = temp_dir.path();

        // Test in non-git directory
        let result = test_get_current_head(non_git_dir).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to get git HEAD"));
    }

    #[tokio::test]
    async fn test_get_current_head_respects_working_directory() {
        // This test verifies that the git command runs in the correct directory
        let temp_dir = TempDir::new().unwrap();
        let repo1 = temp_dir.path().join("repo1");
        let repo2 = temp_dir.path().join("repo2");

        // Create two separate repos
        for (repo_path, commit_msg) in &[(&repo1, "Repo 1 commit"), (&repo2, "Repo 2 commit")] {
            std::fs::create_dir(repo_path).unwrap();
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to init git repo");

            std::process::Command::new("git")
                .args(["config", "user.email", "test@example.com"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to set git email");

            std::process::Command::new("git")
                .args(["config", "user.name", "Test User"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to set git name");

            std::fs::write(
                repo_path.join("test.txt"),
                format!("content for {commit_msg}"),
            )
            .unwrap();
            std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(repo_path)
                .output()
                .expect("Failed to stage files");

            std::process::Command::new("git")
                .args(["commit", "-m", commit_msg])
                .current_dir(repo_path)
                .output()
                .expect("Failed to create commit");
        }

        // Get heads from both repos
        let head1 = test_get_current_head(&repo1).await.unwrap();
        let head2 = test_get_current_head(&repo2).await.unwrap();

        // They should be different
        assert_ne!(
            head1, head2,
            "Different repos should have different HEAD commits"
        );
    }
}
