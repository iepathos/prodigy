//! Workflow executor with commit verification
//!
//! Executes workflow steps in sequence, verifies git commits when required,
//! and manages iteration logic for continuous improvement sessions.

use crate::commands::{AttributeValue, CommandRegistry, ExecutionContext};
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::cook::workflow::on_failure::OnFailureConfig;
use crate::cook::workflow::validation::{ValidationConfig, ValidationResult};
use crate::session::{format_duration, TimingTracker};
use crate::testing::config::TestConfiguration;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Capture output configuration - either a boolean or a variable name
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum CaptureOutput {
    /// Don't capture output
    #[default]
    Disabled,
    /// Capture to default variable names (claude.output, shell.output, etc.)
    Default,
    /// Capture to a custom variable name
    Variable(String),
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
    /// Legacy name-based approach
    Legacy(String),
    /// Modular command handler
    Handler {
        handler_name: String,
        attributes: HashMap<String, AttributeValue>,
    },
}

/// Result of executing a step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Workflow context for variable interpolation
#[derive(Debug, Clone, Default)]
pub struct WorkflowContext {
    pub variables: HashMap<String, String>,
    pub captured_outputs: HashMap<String, String>,
    pub iteration_vars: HashMap<String, String>,
    pub validation_results: HashMap<String, ValidationResult>,
}

impl WorkflowContext {
    /// Interpolate variables in a template string
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Replace ${VAR} and $VAR patterns
        for (key, value) in &self.variables {
            result = result.replace(&format!("${{{key}}}"), value);
            result = result.replace(&format!("${key}"), value);
        }

        for (key, value) in &self.captured_outputs {
            result = result.replace(&format!("${{{key}}}"), value);
            result = result.replace(&format!("${key}"), value);
        }

        for (key, value) in &self.iteration_vars {
            result = result.replace(&format!("${{{key}}}"), value);
            result = result.replace(&format!("${key}"), value);
        }

        // Add validation context variables
        for (key, validation_result) in &self.validation_results {
            // Add completion percentage
            result = result.replace(
                &format!("${{{key}.completion}}"),
                &validation_result.completion_percentage.to_string(),
            );
            result = result.replace(
                &format!("${key}.completion"),
                &validation_result.completion_percentage.to_string(),
            );

            // Add gaps summary
            let gaps_summary = validation_result.gaps_summary();
            result = result.replace(&format!("${{{key}.gaps}}"), &gaps_summary);
            result = result.replace(&format!("${key}.gaps"), &gaps_summary);

            // Add missing items as comma-separated list
            let missing = validation_result.missing.join(", ");
            result = result.replace(&format!("${{{key}.missing}}"), &missing);
            result = result.replace(&format!("${key}.missing"), &missing);
        }

        result
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Legacy command field (for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Modular command handler with attributes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<HandlerStep>,

    /// Whether to capture command output (bool or variable name string)
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

    /// Conditional execution on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_success: Option<Box<WorkflowStep>>,

    /// Conditional execution based on exit codes
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub on_exit_code: HashMap<i32, Box<WorkflowStep>>,

    /// Whether this command is expected to create commits
    #[serde(default = "default_commit_required")]
    pub commit_required: bool,

    /// Validation configuration for checking implementation completeness
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate: Option<ValidationConfig>,
}

fn default_commit_required() -> bool {
    true
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
    /// Steps to execute (for sequential mode)
    pub steps: Vec<WorkflowStep>,
    /// Map phase configuration (for mapreduce mode)
    pub map_phase: Option<crate::cook::execution::MapPhase>,
    /// Reduce phase configuration (for mapreduce mode)
    pub reduce_phase: Option<crate::cook::execution::ReducePhase>,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Whether to iterate
    pub iterate: bool,
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
}

impl WorkflowExecutor {
    /// Sets the command registry for modular command handlers
    pub async fn with_command_registry(mut self) -> Self {
        self.command_registry = Some(CommandRegistry::with_defaults().await);
        self
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

        // Check if validation passed
        if !validation_config.is_complete(&validation_result) {
            self.user_interaction.display_warning(&format!(
                "Validation incomplete: {:.1}% complete",
                validation_result.completion_percentage
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
                let handler_result = Box::pin(self.execute_step(&handler_step, env, ctx)).await?;

                if !handler_result.success {
                    self.user_interaction
                        .display_error("Completion handler failed");
                    break;
                }

                // Re-run validation
                current_result = self.execute_validation(validation_config, env, ctx).await?;

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

    /// Determine if workflow should fail based on command result
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
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        match command_type.clone() {
            CommandType::Claude(cmd) => {
                let interpolated_cmd = ctx.interpolate(&cmd);
                self.execute_claude_command(&interpolated_cmd, env, env_vars)
                    .await
            }
            CommandType::Shell(cmd) => {
                let interpolated_cmd = ctx.interpolate(&cmd);
                self.execute_shell_for_step(&interpolated_cmd, step, env, ctx, env_vars)
                    .await
            }
            CommandType::Test(test_cmd) => {
                self.execute_test_command(test_cmd, env, ctx, env_vars)
                    .await
            }
            CommandType::Legacy(cmd) => {
                let interpolated_cmd = ctx.interpolate(&cmd);
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
        // Check if there's a handler to execute
        if let Some(handler) = on_failure_config.handler() {
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
        }
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
            let mut any_changes = false;
            for (step_index, step) in workflow.steps.iter().enumerate() {
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
                if let Some((cmd_name, _)) = self.timing_tracker.complete_command() {
                    self.session_manager
                        .update_session(SessionUpdate::RecordCommandTiming(
                            cmd_name,
                            command_duration,
                        ))
                        .await?;
                }

                // Check for commits if required
                if let Some(before) = head_before {
                    let head_after = self.get_current_head(&env.working_dir).await?;
                    if head_after == before {
                        // No commits were created
                        self.handle_no_commits_error(step)?;
                    } else {
                        any_changes = true;
                        self.user_interaction
                            .display_success(&format!("✓ {step_display} created commits"));
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
                self.user_interaction.display_info(&format!(
                    "✓ Iteration {} completed in {}",
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

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_info(&format!(
            "\n📊 Total workflow time: {} across {} iteration{}",
            format_duration(total_duration),
            iteration,
            if iteration == 1 { "" } else { "s" }
        ));

        Ok(())
    }

    /// Prepare environment variables for step execution
    fn prepare_env_vars(
        &self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        // Add MMM context variables
        env_vars.insert("PRODIGY_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "PRODIGY_CONTEXT_DIR".to_string(),
            env.working_dir
                .join(".prodigy/context")
                .to_string_lossy()
                .to_string(),
        );
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Add step-specific environment variables with interpolation
        for (key, value) in &step.env {
            let interpolated_value = ctx.interpolate(value);
            env_vars.insert(key.clone(), interpolated_value);
        }

        env_vars
    }

    /// Execute a single workflow step
    async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Determine command type
        let command_type = self.determine_command_type(step)?;

        // Prepare environment variables
        let env_vars = self.prepare_env_vars(step, env, ctx);

        // Handle test mode
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return self.handle_test_mode_execution(step, &command_type);
        }

        // Execute the command based on its type
        let mut result = self
            .execute_command_by_type(&command_type, step, env, ctx, env_vars)
            .await?;

        // Capture output if requested
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
                self.handle_validation(validation_config, env, ctx).await?;
            }
        }

        // Handle conditional execution (failure, success, exit codes)
        result = self
            .handle_conditional_execution(step, result, env, ctx)
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

        // Interpolate attribute values
        for (_, value) in attributes.iter_mut() {
            if let AttributeValue::String(s) = value {
                *s = ctx.interpolate(s);
            }
        }

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
    async fn execute_claude_command(
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
    async fn execute_shell_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        use tokio::process::Command;
        use tokio::time::{timeout as tokio_timeout, Duration};

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

        let interpolated_cmd = ctx.interpolate(command);

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
                    .display_success(&format!("✓ Shell command succeeded on attempt {attempt}"));
                return Ok(shell_result);
            }

            // Command failed - check if we should retry
            if let Some(debug_config) = on_failure {
                if attempt >= debug_config.max_attempts {
                    self.user_interaction.display_error(&format!(
                        "❌ Shell command failed after {} attempts",
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
                debug_cmd = ctx.interpolate(&debug_cmd);

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
    ) -> Result<StepResult> {
        use std::fs;
        use tempfile::NamedTempFile;

        let interpolated_test_cmd = ctx.interpolate(&test_cmd.command);

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
                    .display_success(&format!("✓ Tests passed on attempt {attempt}"));
                return Ok(test_result);
            }

            // Tests failed - check if we should retry
            if let Some(debug_config) = &test_cmd.on_failure {
                if attempt >= debug_config.max_attempts {
                    self.user_interaction.display_error(&format!(
                        "❌ Tests failed after {} attempts",
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
                debug_cmd = ctx.interpolate(&debug_cmd);

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
    fn handle_test_mode_execution(
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
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(working_dir)
            .output()
            .await
            .context("Failed to execute git rev-parse HEAD")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get git HEAD: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Handle the case where no commits were created when expected
    fn handle_no_commits_error(&self, step: &WorkflowStep) -> Result<()> {
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
        };

        eprintln!("\n❌ Workflow stopped: No changes were committed by {step_display}");
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
        use crate::cook::execution::MapReduceExecutor;
        use crate::worktree::WorktreeManager;

        let workflow_start = Instant::now();

        // Don't duplicate the message - it's already shown by the orchestrator

        // Execute setup phase if present
        if !workflow.steps.is_empty() {
            self.user_interaction
                .display_progress("🔄 Running setup phase...");

            // Execute setup steps in the main worktree
            let mut workflow_context = WorkflowContext::default();
            for (step_index, step) in workflow.steps.iter().enumerate() {
                let step_display = self.get_step_display_name(step);
                self.user_interaction.display_progress(&format!(
                    "Setup step {}/{}: {}",
                    step_index + 1,
                    workflow.steps.len(),
                    step_display
                ));

                // Execute the setup step
                let _step_result = match self.execute_step(step, env, &mut workflow_context).await {
                    Ok(result) => result,
                    Err(e) => {
                        // The execute_step error already contains detailed stdout/stderr
                        // We want to preserve the full error message, not just add context
                        return Err(anyhow!(
                            "Setup step {}/{}: {}\n\n{}",
                            step_index + 1,
                            workflow.steps.len(),
                            step_display,
                            e
                        ));
                    }
                };

                // Note: execute_step will return an error if the step fails and should_fail is true
                // It already includes detailed stdout/stderr in the error message
                // We only get here if the step succeeded or if on_failure handling allowed continuation
            }

            self.user_interaction
                .display_success("✓ Setup phase completed");
        }

        // Ensure we have map phase configuration
        let map_phase = workflow
            .map_phase
            .as_ref()
            .ok_or_else(|| anyhow!("MapReduce workflow requires map phase configuration"))?;

        // Create worktree manager
        let worktree_manager = Arc::new(WorktreeManager::new(
            env.project_dir.clone(),
            self.subprocess.clone(),
        )?);

        // Create MapReduce executor
        let mapreduce_executor = MapReduceExecutor::new(
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

        // Execute MapReduce workflow
        let results = mapreduce_executor
            .execute(map_phase, workflow.reduce_phase.as_ref(), env)
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
        self.user_interaction.display_info(&format!(
            "\n📊 Total MapReduce workflow time: {}",
            format_duration(total_duration),
        ));

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
    fn is_focus_tracking_test(&self) -> bool {
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
            let command = ctx.interpolate(claude_cmd);
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
            let command = ctx.interpolate(shell_cmd);
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
            let interpolated_file = ctx.interpolate(result_file);
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
                command: None,
                handler: None,
                timeout: None,
                capture_output: CaptureOutput::Disabled,
                on_failure: None,
                on_success: None,
                on_exit_code: Default::default(),
                commit_required: on_incomplete.commit_required,
                working_dir: None,
                env: Default::default(),
                validate: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod executor_tests;

// Implement the WorkflowExecutor trait
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper function to test get_current_head directly without needing a full executor
    async fn test_get_current_head(working_dir: &std::path::Path) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(working_dir)
            .output()
            .await
            .context("Failed to execute git rev-parse HEAD")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get git HEAD: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
