//! Argument Processing Module
//!
//! Handles workflow input collection from --map patterns and --args,
//! and processes each input through the workflow execution pipeline.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use super::{CookConfig, ExecutionEnvironment};
use crate::config::WorkflowCommand;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::cook::workflow::{ExtendedWorkflowConfig, WorkflowContext, WorkflowStep};
use crate::testing::config::TestConfiguration;
use crate::unified_session::{format_duration, TimingTracker};

/// Handles argument processing and workflow input iteration
pub struct ArgumentProcessor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    test_config: Option<Arc<TestConfiguration>>,
}

impl ArgumentProcessor {
    /// Create a new ArgumentProcessor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        test_config: Option<Arc<TestConfiguration>>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            test_config,
        }
    }

    /// Execute workflow with arguments from --map and --args
    pub async fn execute_workflow_with_args(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        log::debug!("execute_workflow_with_args started");
        let workflow_start = Instant::now();
        let mut timing_tracker = TimingTracker::new();

        // Collect all inputs from --map patterns and --args
        log::debug!("Collecting workflow inputs");
        let all_inputs = self.collect_workflow_inputs(config)?;
        log::debug!("Collected {} inputs", all_inputs.len());

        if all_inputs.is_empty() {
            return Err(anyhow!("No inputs found from --map patterns or --args"));
        }

        self.user_interaction
            .display_status(&format!("Total inputs to process: {}", all_inputs.len()));

        // Process each input
        for (index, input) in all_inputs.iter().enumerate() {
            timing_tracker.start_iteration();

            self.process_workflow_input(
                env,
                config,
                input,
                index,
                all_inputs.len(),
                &mut timing_tracker,
            )
            .await?;

            if let Some(iteration_duration) = timing_tracker.complete_iteration() {
                self.user_interaction.display_success(&format!(
                    "Input {} completed in {}",
                    index + 1,
                    format_duration(iteration_duration)
                ));
            }
        }

        self.user_interaction.display_success(&format!(
            "Processed all {} inputs successfully!",
            all_inputs.len()
        ));

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total workflow time",
            &format!(
                "{} for {} inputs",
                format_duration(total_duration),
                all_inputs.len()
            ),
        );

        Ok(())
    }

    /// Process a single workflow input
    async fn process_workflow_input(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        input: &str,
        index: usize,
        total: usize,
        _timing_tracker: &mut TimingTracker,
    ) -> Result<()> {
        self.user_interaction.display_progress(&format!(
            "Processing input {}/{}: {}",
            index + 1,
            total,
            input
        ));

        // Update session - increment iteration for each input processed
        self.session_manager
            .update_session(SessionUpdate::IncrementIteration)
            .await?;

        // Build variables map for this input
        let mut variables = HashMap::new();
        variables.insert("ARG".to_string(), input.to_string());
        variables.insert("INDEX".to_string(), (index + 1).to_string());
        variables.insert("TOTAL".to_string(), total.to_string());

        // Convert WorkflowCommands to WorkflowSteps to preserve validation config
        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .map(Self::convert_command_to_step)
            .collect();

        // Create extended workflow config with the converted steps
        let extended_workflow = ExtendedWorkflowConfig {
            name: "args-workflow".to_string(),
            mode: crate::cook::workflow::WorkflowMode::Sequential,
            steps,
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // Create workflow context with variables
        // Note: The context is managed internally by the executor, we just need to ensure
        // variables are set via the environment for command substitution
        let _workflow_context = WorkflowContext {
            variables: variables.clone(),
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
            validation_results: HashMap::new(),
            variable_store: std::sync::Arc::new(crate::cook::workflow::VariableStore::new()),
            git_tracker: None,
        };

        // Set the ARG environment variable so the executor can pick it up
        std::env::set_var("PRODIGY_ARG", input);

        // Create workflow executor with checkpoint support using session storage
        let checkpoint_storage = crate::cook::workflow::CheckpointStorage::Session {
            session_id: env.session_id.to_string(),
        };
        let checkpoint_manager = Arc::new(crate::cook::workflow::CheckpointManager::with_storage(
            checkpoint_storage,
        ));
        let workflow_id = format!("workflow-{}", chrono::Utc::now().timestamp_millis());

        let mut executor = self
            .create_workflow_executor_internal(config)
            .with_checkpoint_manager(checkpoint_manager, workflow_id)
            .with_dry_run(config.command.dry_run);

        // Set test config if available
        if let Some(test_config) = &self.test_config {
            executor = crate::cook::workflow::WorkflowExecutorImpl::with_test_config(
                self.claude_executor.clone(),
                self.session_manager.clone(),
                self.user_interaction.clone(),
                test_config.clone(),
            );
        }

        // Set global environment configuration if present in workflow
        if config.workflow.env.is_some()
            || config.workflow.secrets.is_some()
            || config.workflow.env_files.is_some()
            || config.workflow.profiles.is_some()
        {
            let global_env_config = crate::cook::environment::EnvironmentConfig {
                global_env: config
                    .workflow
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    crate::cook::environment::EnvValue::Static(v.clone()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                secrets: config.workflow.secrets.clone().unwrap_or_default(),
                env_files: config.workflow.env_files.clone().unwrap_or_default(),
                inherit: true,
                profiles: config.workflow.profiles.clone().unwrap_or_default(),
                active_profile: None,
            };
            executor = executor.with_environment_config(global_env_config)?;
        }

        // Execute the workflow through the executor to ensure validation is handled
        executor.execute(&extended_workflow, env).await?;

        Ok(())
    }

    /// Collect inputs from --map patterns and --args
    fn collect_workflow_inputs(&self, config: &CookConfig) -> Result<Vec<String>> {
        let mut all_inputs = Vec::new();

        // Process --map patterns
        for pattern in &config.command.map {
            self.user_interaction
                .display_info(&format!("🔍 Processing file pattern: {pattern}"));

            let pattern_inputs = self.process_glob_pattern(pattern)?;
            all_inputs.extend(pattern_inputs);
        }

        // Add direct arguments from --args
        if !config.command.args.is_empty() {
            self.user_interaction.display_action(&format!(
                "Adding {} direct arguments from --args",
                config.command.args.len()
            ));
            all_inputs.extend(config.command.args.clone());
        }

        Ok(all_inputs)
    }

    /// Process a single glob pattern and return extracted inputs
    fn process_glob_pattern(&self, pattern: &str) -> Result<Vec<String>> {
        let mut inputs = Vec::new();

        match glob::glob(pattern) {
            Ok(entries) => {
                let mut pattern_matches = 0;
                for path in entries.flatten() {
                    self.user_interaction
                        .display_success(&format!("Found file: {}", path.display()));

                    let input = self.extract_input_from_path(&path);
                    inputs.push(input);
                    pattern_matches += 1;
                }

                if pattern_matches == 0 {
                    self.user_interaction
                        .display_warning(&format!("No files matched pattern: {pattern}"));
                } else {
                    self.user_interaction.display_success(&format!(
                        "📁 Found {pattern_matches} files matching pattern: {pattern}"
                    ));
                }
            }
            Err(e) => {
                self.user_interaction
                    .display_error(&format!("Error processing pattern '{pattern}': {e}"));
            }
        }

        Ok(inputs)
    }

    /// Extract input string from a file path
    fn extract_input_from_path(&self, path: &Path) -> String {
        if let Some(stem) = path.file_stem() {
            let filename = stem.to_string_lossy();
            // Extract numeric prefix if present (e.g., "65-cook-refactor" -> "65")
            if let Some(dash_pos) = filename.find('-') {
                filename[..dash_pos].to_string()
            } else {
                filename.to_string()
            }
        } else {
            path.to_string_lossy().to_string()
        }
    }

    /// Convert WorkflowCommand to WorkflowStep
    fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
        super::normalization::convert_command_to_step(cmd)
    }

    /// Create workflow executor (internal helper)
    fn create_workflow_executor_internal(
        &self,
        config: &CookConfig,
    ) -> crate::cook::workflow::WorkflowExecutorImpl {
        crate::cook::workflow::WorkflowExecutorImpl::new(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.user_interaction.clone(),
        )
        .with_dry_run(config.command.dry_run)
    }
}
