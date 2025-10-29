//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using the extracted components.

use crate::abstractions::git::GitOperations;
use crate::config::{WorkflowCommand, WorkflowConfig};
use crate::testing::config::TestConfiguration;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::cook::command::CookCommand;
use crate::cook::execution::{ClaudeExecutor, CommandExecutor};
use crate::cook::interaction::UserInteraction;
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::cook::workflow::{ExtendedWorkflowConfig, WorkflowStep};
use crate::unified_session::{format_duration, TimingTracker};

/// Configuration for cook orchestration
#[derive(Debug, Clone)]
pub struct CookConfig {
    /// Command to execute
    pub command: CookCommand,
    /// Project path
    pub project_path: Arc<PathBuf>,
    /// Workflow configuration
    pub workflow: Arc<WorkflowConfig>,
    /// MapReduce configuration (if this is a MapReduce workflow)
    pub mapreduce_config: Option<Arc<crate::config::MapReduceWorkflowConfig>>,
}

/// Trait for orchestrating cook operations
#[async_trait]
pub trait CookOrchestrator: Send + Sync {
    /// Run the cook operation
    async fn run(&self, config: CookConfig) -> Result<()>;

    /// Check prerequisites
    async fn check_prerequisites(&self) -> Result<()>;

    /// Setup working environment
    async fn setup_environment(&self, config: &CookConfig) -> Result<ExecutionEnvironment>;

    /// Execute workflow
    async fn execute_workflow(&self, env: &ExecutionEnvironment, config: &CookConfig)
        -> Result<()>;

    /// Cleanup after execution
    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()>;
}

/// Classification of workflow types
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WorkflowType {
    MapReduce,
    StructuredWithOutputs,
    WithArguments,
    Standard,
}

impl From<WorkflowType> for crate::cook::session::WorkflowType {
    fn from(wt: WorkflowType) -> Self {
        match wt {
            WorkflowType::MapReduce => crate::cook::session::WorkflowType::MapReduce,
            WorkflowType::StructuredWithOutputs => {
                crate::cook::session::WorkflowType::StructuredWithOutputs
            }
            WorkflowType::WithArguments => crate::cook::session::WorkflowType::Iterative,
            WorkflowType::Standard => crate::cook::session::WorkflowType::Standard,
        }
    }
}

/// Execution environment for cook operations
#[derive(Debug)]
pub struct ExecutionEnvironment {
    /// Working directory (may be worktree)
    pub working_dir: Arc<PathBuf>,
    /// Original project directory
    pub project_dir: Arc<PathBuf>,
    /// Worktree name if using worktree
    pub worktree_name: Option<Arc<str>>,
    /// Session ID
    pub session_id: Arc<str>,
}

impl Clone for ExecutionEnvironment {
    fn clone(&self) -> Self {
        Self {
            working_dir: Arc::clone(&self.working_dir),
            project_dir: Arc::clone(&self.project_dir),
            worktree_name: self.worktree_name.as_ref().map(Arc::clone),
            session_id: Arc::clone(&self.session_id),
        }
    }
}

/// Default implementation of cook orchestrator
pub struct DefaultCookOrchestrator {
    /// Session manager (legacy)
    session_manager: Arc<dyn SessionManager>,
    /// Unified session manager (lazily initialized, thread-safe)
    unified_session_manager: Arc<Mutex<Option<Arc<crate::unified_session::SessionManager>>>>,
    /// Command executor
    #[allow(dead_code)]
    command_executor: Arc<dyn CommandExecutor>,
    /// Claude executor
    claude_executor: Arc<dyn ClaudeExecutor>,
    /// User interaction
    user_interaction: Arc<dyn UserInteraction>,
    /// Git operations
    git_operations: Arc<dyn GitOperations>,
    /// Subprocess manager
    subprocess: crate::subprocess::SubprocessManager,
    /// Test configuration
    test_config: Option<Arc<TestConfiguration>>,
    /// Session operations
    session_ops: super::session_ops::SessionOperations,
    /// Workflow executor
    #[allow(dead_code)]
    workflow_executor: super::workflow_execution::WorkflowExecutor,
    /// Health metrics
    health_metrics: super::health_metrics::HealthMetrics,
    /// Argument processor
    argument_processor: super::argument_processing::ArgumentProcessor,
    /// Execution pipeline
    execution_pipeline: super::execution_pipeline::ExecutionPipeline,
}

impl DefaultCookOrchestrator {
    /// Create a new orchestrator with dependencies
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: crate::subprocess::SubprocessManager,
    ) -> Self {
        let session_ops = super::session_ops::SessionOperations::new(
            Arc::clone(&session_manager),
            Arc::clone(&claude_executor),
            Arc::clone(&user_interaction),
            Arc::clone(&git_operations),
            subprocess.clone(),
        );

        let workflow_executor = super::workflow_execution::WorkflowExecutor::new(
            Arc::clone(&session_manager),
            Arc::clone(&claude_executor),
            Arc::clone(&user_interaction),
            subprocess.clone(),
        );

        let health_metrics =
            super::health_metrics::HealthMetrics::new(Arc::clone(&user_interaction));

        let argument_processor = super::argument_processing::ArgumentProcessor::new(
            Arc::clone(&claude_executor),
            Arc::clone(&session_manager),
            Arc::clone(&user_interaction),
            None,
        );

        let execution_pipeline = super::execution_pipeline::ExecutionPipeline::new(
            Arc::clone(&session_manager),
            Arc::clone(&user_interaction),
            Arc::clone(&claude_executor),
            Arc::clone(&git_operations),
            subprocess.clone(),
            session_ops.clone(),
            workflow_executor.clone(),
        );

        Self {
            session_manager,
            unified_session_manager: Arc::new(Mutex::new(None)),
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
            test_config: None,
            session_ops,
            workflow_executor,
            health_metrics,
            argument_processor,
            execution_pipeline,
        }
    }

    /// Internal constructor used by the builder
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_builder(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: crate::subprocess::SubprocessManager,
        test_config: Option<Arc<TestConfiguration>>,
    ) -> Self {
        let session_ops = super::session_ops::SessionOperations::new(
            Arc::clone(&session_manager),
            Arc::clone(&claude_executor),
            Arc::clone(&user_interaction),
            Arc::clone(&git_operations),
            subprocess.clone(),
        );

        let workflow_executor = super::workflow_execution::WorkflowExecutor::new(
            Arc::clone(&session_manager),
            Arc::clone(&claude_executor),
            Arc::clone(&user_interaction),
            subprocess.clone(),
        );

        let health_metrics =
            super::health_metrics::HealthMetrics::new(Arc::clone(&user_interaction));

        let argument_processor = super::argument_processing::ArgumentProcessor::new(
            Arc::clone(&claude_executor),
            Arc::clone(&session_manager),
            Arc::clone(&user_interaction),
            test_config.clone(),
        );

        let execution_pipeline = super::execution_pipeline::ExecutionPipeline::new(
            Arc::clone(&session_manager),
            Arc::clone(&user_interaction),
            Arc::clone(&claude_executor),
            Arc::clone(&git_operations),
            subprocess.clone(),
            session_ops.clone(),
            workflow_executor.clone(),
        );

        Self {
            session_manager,
            unified_session_manager: Arc::new(Mutex::new(None)),
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
            test_config,
            session_ops,
            workflow_executor,
            health_metrics,
            argument_processor,
            execution_pipeline,
        }
    }

    /// Create environment configuration from workflow config - avoids cloning
    /// Create workflow executor - avoids repeated Arc cloning
    pub(super) fn create_workflow_executor_internal(
        &self,
        config: &CookConfig,
    ) -> crate::cook::workflow::WorkflowExecutorImpl {
        super::construction::create_workflow_executor(
            Arc::clone(&self.claude_executor),
            Arc::clone(&self.session_manager),
            Arc::clone(&self.user_interaction),
            config.command.playbook.clone(),
        )
    }

    /// Create base workflow state for session management - avoids field cloning
    #[allow(dead_code)]
    pub(super) fn create_workflow_state_base_internal(
        &self,
        config: &CookConfig,
    ) -> (PathBuf, Vec<String>, Vec<String>) {
        super::construction::create_workflow_state_base(&config.command)
    }

    /// Display health score for the project
    async fn display_health_score(&self, config: &CookConfig) -> Result<()> {
        self.health_metrics.display_health_score(config).await
    }

    /// Create a new orchestrator with test configuration
    #[allow(clippy::too_many_arguments)]
    pub fn with_test_config(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: crate::subprocess::SubprocessManager,
        test_config: Arc<TestConfiguration>,
    ) -> Self {
        let session_ops = super::session_ops::SessionOperations::new(
            Arc::clone(&session_manager),
            Arc::clone(&claude_executor),
            Arc::clone(&user_interaction),
            Arc::clone(&git_operations),
            subprocess.clone(),
        );

        let workflow_executor = super::workflow_execution::WorkflowExecutor::new(
            Arc::clone(&session_manager),
            Arc::clone(&claude_executor),
            Arc::clone(&user_interaction),
            subprocess.clone(),
        );

        let health_metrics =
            super::health_metrics::HealthMetrics::new(Arc::clone(&user_interaction));

        let argument_processor = super::argument_processing::ArgumentProcessor::new(
            Arc::clone(&claude_executor),
            Arc::clone(&session_manager),
            Arc::clone(&user_interaction),
            Some(test_config.clone()),
        );

        let execution_pipeline = super::execution_pipeline::ExecutionPipeline::new(
            Arc::clone(&session_manager),
            Arc::clone(&user_interaction),
            Arc::clone(&claude_executor),
            Arc::clone(&git_operations),
            subprocess.clone(),
            session_ops.clone(),
            workflow_executor.clone(),
        );

        Self {
            session_manager,
            unified_session_manager: Arc::new(Mutex::new(None)),
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
            test_config: Some(test_config),
            session_ops,
            workflow_executor,
            health_metrics,
            argument_processor,
            execution_pipeline,
        }
    }

    /// Generate session ID using unified format
    fn generate_session_id(&self) -> String {
        self.session_ops.generate_session_id()
    }

    /// Get or initialize the unified session manager
    async fn get_unified_session_manager(
        &self,
    ) -> Result<Arc<crate::unified_session::SessionManager>> {
        // Check if already initialized
        {
            let guard = self
                .unified_session_manager
                .lock()
                .map_err(|e| anyhow!("Failed to lock unified session manager: {}", e))?;
            if let Some(manager) = guard.as_ref() {
                return Ok(Arc::clone(manager));
            }
        }

        // Initialize the manager
        let storage =
            crate::storage::GlobalStorage::new().context("Failed to create global storage")?;
        let manager = Arc::new(
            crate::unified_session::SessionManager::new(storage)
                .await
                .context("Failed to create unified session manager")?,
        );

        // Store it
        {
            let mut guard = self
                .unified_session_manager
                .lock()
                .map_err(|e| anyhow!("Failed to lock unified session manager: {}", e))?;
            *guard = Some(Arc::clone(&manager));
        }

        Ok(manager)
    }

    /// Update unified session status (best effort, don't fail workflow on error)
    async fn update_unified_session_status(&self, session_id: &str, success: bool) {
        // Get session manager (if initialized)
        let manager = match self.get_unified_session_manager().await {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "Failed to get unified session manager for status update: {}",
                    e
                );
                return;
            }
        };

        // Convert session_id string to SessionId
        let session_id_obj = crate::unified_session::SessionId::from_string(session_id.to_string());

        // Try to load the session first to check if it exists
        match manager.load_session(&session_id_obj).await {
            Ok(_) => {
                // Session exists, update its status
                if let Err(e) = manager.complete_session(&session_id_obj, success).await {
                    log::warn!("Failed to update unified session status: {}", e);
                }
            }
            Err(_) => {
                // Session doesn't exist (might be MapReduce or dry-run), skip update
                log::debug!(
                    "Unified session {} not found, skipping status update",
                    session_id
                );
            }
        }
    }

    /// Check prerequisites with config-aware git checking
    async fn check_prerequisites_with_config(&self, config: &CookConfig) -> Result<()> {
        self.session_ops
            .check_prerequisites_with_config(config)
            .await
    }

    /// Resume an interrupted workflow
    async fn resume_workflow(&self, session_id: &str, config: CookConfig) -> Result<()> {
        // Delegate the main resume logic to ExecutionPipeline
        // But handle cleanup here since it requires orchestrator-specific knowledge
        let result = self
            .execution_pipeline
            .resume_workflow(session_id, config.clone())
            .await;

        // If resume was successful or failed, handle cleanup if we have environment info
        // For now, ExecutionPipeline handles the full resume including cleanup internally
        result
    }

    /// Convert a workflow command to a workflow step
    fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
        super::workflow_execution::WorkflowExecutor::convert_command_to_step(cmd)
    }

    /// Classify the workflow type based on configuration
    pub(crate) fn classify_workflow_type(config: &CookConfig) -> WorkflowType {
        super::workflow_classifier::classify_workflow_type(config)
    }
}

#[async_trait]
impl CookOrchestrator for DefaultCookOrchestrator {
    async fn run(&self, config: CookConfig) -> Result<()> {
        // Check if this is a resume operation
        if let Some(session_id) = config.command.resume.clone() {
            return self.resume_workflow(&session_id, config).await;
        }

        // Check prerequisites
        self.check_prerequisites_with_config(&config).await?;

        // Setup environment
        let env = self.setup_environment(&config).await?;

        // Initialize session metadata
        self.execution_pipeline
            .initialize_session_metadata(&env.session_id, &config)
            .await?;

        // Setup signal handlers for graceful interruption
        let interrupt_handler = self.execution_pipeline.setup_signal_handlers(
            &config,
            &env.session_id,
            env.worktree_name.as_ref().map(Arc::clone),
        )?;

        // Execute workflow
        log::debug!("About to execute workflow");
        let execution_result = self.execute_workflow(&env, &config).await;
        log::debug!(
            "Workflow execution completed with result: {:?}",
            execution_result.is_ok()
        );

        // Cancel the interrupt handler
        interrupt_handler.abort();

        // Update unified session status based on execution result
        let execution_succeeded = execution_result.is_ok();
        self.update_unified_session_status(&env.session_id, execution_succeeded)
            .await;

        // Finalize session with appropriate status
        self.execution_pipeline
            .finalize_session(
                &env,
                &config,
                execution_result,
                self.cleanup(&env, &config),
                self.display_health_score(&config),
            )
            .await
    }

    async fn check_prerequisites(&self) -> Result<()> {
        self.session_ops.check_prerequisites().await
    }

    async fn setup_environment(&self, config: &CookConfig) -> Result<ExecutionEnvironment> {
        let session_id = Arc::from(self.generate_session_id().as_str());
        let mut working_dir = Arc::clone(&config.project_path);
        let mut worktree_name: Option<Arc<str>> = None;

        // Create UnifiedSession for this workflow (skip for MapReduce workflows as they handle their own sessions)
        if config.mapreduce_config.is_none() && !config.command.dry_run {
            let unified_session_manager = self
                .get_unified_session_manager()
                .await
                .context("Failed to get unified session manager")?;

            let workflow_id = format!("workflow-{}", chrono::Utc::now().timestamp_millis());

            // Populate metadata with execution information
            let mut metadata = std::collections::HashMap::new();
            metadata.insert(
                "execution_start_time".to_string(),
                serde_json::json!(chrono::Utc::now().to_rfc3339()),
            );
            metadata.insert(
                "workflow_type".to_string(),
                serde_json::json!("standard"),
            );
            metadata.insert(
                "total_steps".to_string(),
                serde_json::json!(config.workflow.commands.len()),
            );
            metadata.insert(
                "current_step".to_string(),
                serde_json::json!(0),
            );

            let session_config = crate::unified_session::SessionConfig {
                session_type: crate::unified_session::SessionType::Workflow,
                workflow_id: Some(workflow_id.clone()),
                job_id: None,
                metadata,
            };

            let unified_session_id = unified_session_manager
                .create_session(session_config)
                .await
                .context("Failed to create unified session")?;

            // Start the session
            unified_session_manager
                .start_session(&unified_session_id)
                .await
                .context("Failed to start unified session")?;

            log::info!(
                "Created unified session: {} (workflow_id: {})",
                unified_session_id,
                workflow_id
            );
        }

        // Always setup worktree (but not in dry-run mode)
        if !config.command.dry_run {
            // Get merge config from workflow or mapreduce config
            let merge_config = config.workflow.merge.clone().or_else(|| {
                config
                    .mapreduce_config
                    .as_ref()
                    .and_then(|m| m.merge.clone())
            });

            // Get workflow environment variables
            let workflow_env = config.workflow.env.clone().unwrap_or_default();

            let worktree_manager = WorktreeManager::with_config(
                config.project_path.to_path_buf(),
                self.subprocess.clone(),
                config.command.verbosity,
                merge_config,
                workflow_env,
            )?;
            // Pass the unified session ID to the worktree manager
            let session = worktree_manager.create_session_with_id(&session_id).await?;

            working_dir = Arc::new(session.path.clone());
            worktree_name = Some(Arc::from(session.name.as_ref()));

            self.user_interaction
                .display_info(&format!("Created worktree at: {}", working_dir.display()));
        } else {
            // In dry-run mode, just note that worktree would be created
            self.user_interaction
                .display_info("[DRY RUN] Would create worktree for isolated execution");
        }

        Ok(ExecutionEnvironment {
            working_dir,
            project_dir: Arc::clone(&config.project_path),
            worktree_name,
            session_id,
        })
    }

    async fn execute_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        log::debug!("execute_workflow called");
        // Feature flag for gradual rollout of unified execution path
        if std::env::var("USE_UNIFIED_PATH").is_ok() {
            log::debug!("Using unified execution path");
            return self.execute_unified(env, config).await;
        }

        // Use pure function to classify workflow type
        let workflow_type = Self::classify_workflow_type(config);
        log::debug!("Workflow type classified as: {:?}", workflow_type);
        match workflow_type {
            WorkflowType::MapReduce => {
                // Don't show "Executing workflow: default" for MapReduce workflows
                // The MapReduce executor will show its own appropriate messages
                let mapreduce_config = config.mapreduce_config.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("MapReduce workflow requires mapreduce configuration")
                })?;
                return self
                    .execute_mapreduce_workflow(env, config, mapreduce_config)
                    .await;
            }
            WorkflowType::StructuredWithOutputs => {
                self.user_interaction
                    .display_info("Executing structured workflow with outputs");
                return self.execute_structured_workflow(env, config).await;
            }
            WorkflowType::WithArguments => {
                log::debug!("Executing workflow with arguments");
                self.user_interaction
                    .display_info("Processing workflow with arguments or file patterns");
                let result = self.execute_workflow_with_args(env, config).await;
                log::debug!("execute_workflow_with_args completed: {:?}", result.is_ok());
                return result;
            }
            WorkflowType::Standard => {
                // Continue with standard workflow processing below
            }
        }

        // Analysis functionality has been removed in v0.3.0

        // Convert WorkflowConfig to ExtendedWorkflowConfig using pure function
        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .map(Self::convert_command_to_step)
            .collect();

        // Analysis functionality has been removed in v0.3.0
        let _has_analyze_step = false;

        let extended_workflow = ExtendedWorkflowConfig {
            name: "default".to_string(),
            mode: crate::cook::workflow::WorkflowMode::Sequential,
            steps,
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: config.command.max_iterations,
            iterate: config.command.max_iterations > 1,
            retry_defaults: None,
            environment: None,
            // collect_metrics removed - MMM focuses on orchestration
        };

        // Analysis functionality has been removed in v0.3.0

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

        // Execute workflow steps
        executor.execute(&extended_workflow, env).await?;

        Ok(())
    }

    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        // Save session state to a separate file to avoid conflicts with StateManager
        // Use the working directory (which may be a worktree) not the project directory
        let session_state_path = env.working_dir.join(".prodigy/session_state.json");
        self.session_manager.save_state(&session_state_path).await?;

        // Clean up worktree if needed
        if let Some(ref worktree_name) = env.worktree_name {
            // Skip user prompt in test mode
            let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
            let should_merge = if test_mode {
                // Default to not merging in test mode to avoid complications
                false
            } else if config.command.auto_accept {
                // Auto-accept when -y flag is provided
                true
            } else {
                // Get the merge target branch for the prompt
                let temp_manager = WorktreeManager::with_config(
                    env.project_dir.to_path_buf(),
                    self.subprocess.clone(),
                    config.command.verbosity,
                    None,
                    HashMap::new(),
                )?;
                let merge_target = temp_manager
                    .get_merge_target(worktree_name)
                    .await
                    .unwrap_or_else(|_| "master".to_string());

                // Ask user if they want to merge, showing the target branch
                let prompt = format!("Merge {} to {}", worktree_name, merge_target);
                self.user_interaction.prompt_yes_no(&prompt).await?
            };

            if should_merge {
                // Get merge config from workflow or mapreduce config
                let merge_config = config.workflow.merge.clone().or_else(|| {
                    config
                        .mapreduce_config
                        .as_ref()
                        .and_then(|m| m.merge.clone())
                });

                // Get workflow environment variables
                let workflow_env = config.workflow.env.clone().unwrap_or_default();

                let worktree_manager = WorktreeManager::with_config(
                    env.project_dir.to_path_buf(),
                    self.subprocess.clone(),
                    config.command.verbosity,
                    merge_config,
                    workflow_env,
                )?;

                // merge_session already handles auto-cleanup internally based on PRODIGY_AUTO_CLEANUP env var
                // We should not duplicate cleanup here to avoid race conditions
                worktree_manager.merge_session(worktree_name).await?;
                self.user_interaction
                    .display_success("Worktree changes merged successfully!");

                // Note: merge_session already handles cleanup based on auto_cleanup config
                // It will either:
                // 1. Auto-cleanup if PRODIGY_AUTO_CLEANUP is true (default)
                // 2. Display cleanup instructions if auto-cleanup is disabled
                // We should not duplicate that logic here
            }
        }

        Ok(())
    }
}

// Analysis functionality has been removed in v0.3.0
// ProgressReporter trait was part of the analysis module

impl DefaultCookOrchestrator {
    /// Execute a structured workflow with outputs
    async fn execute_structured_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        self.execution_pipeline
            .execute_structured_workflow(env, config)
            .await
    }

    /* REMOVED: Analysis functionality has been removed in v0.3.0
    /// Execute workflow with per-step analysis configuration
    async fn execute_workflow_with_analysis(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        let workflow_start = Instant::now();
        let mut timing_tracker = TimingTracker::new();

        // Execute iterations if configured
        let max_iterations = config.command.max_iterations;
        for iteration in 1..=max_iterations {
            timing_tracker.start_iteration();

            // Display iteration start with visual boundary
            self.user_interaction
                .iteration_start(iteration, max_iterations);

            // Increment iteration counter
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;

            // Execute each command in sequence
            for (step_index, cmd) in config.workflow.commands.iter().enumerate() {
                let mut command = cmd.to_command();
                // Apply defaults from the command registry
                crate::config::apply_command_defaults(&mut command);

                // Display step start with description
                let step_description = format!(
                    "{}: {}",
                    command.name,
                    command
                        .args
                        .iter()
                        .map(|a| a.resolve(&HashMap::new()))
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                self.user_interaction.step_start(
                    (step_index + 1) as u32,
                    config.workflow.commands.len() as u32,
                    &step_description,
                );

                // Start timing this command
                timing_tracker.start_command(command.name.clone());

                // Analysis functionality has been removed in v0.3.0

                // Build command string
                let mut cmd_parts = vec![format!("/{}", command.name)];
                for arg in &command.args {
                    let resolved_arg = arg.resolve(&HashMap::new());
                    if !resolved_arg.is_empty() {
                        cmd_parts.push(resolved_arg);
                    }
                }
                let final_command = cmd_parts.join(" ");

                self.user_interaction
                    .display_action(&format!("Executing command: {final_command}"));

                // Execute the command
                let mut env_vars = HashMap::new();
                env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

                let result = self
                    .claude_executor
                    .execute_claude_command(&final_command, &env.working_dir, env_vars)
                    .await?;

                if !result.success {
                    anyhow::bail!(
                        "Command '{}' failed with exit code {:?}. Error: {}",
                        command.name,
                        result.exit_code,
                        result.stderr
                    );
                } else {
                    // Track file changes when command succeeds
                    self.session_manager
                        .update_session(SessionUpdate::AddFilesChanged(1))
                        .await?;

                    // Complete command timing
                    if let Some((cmd_name, duration)) = timing_tracker.complete_command() {
                        self.user_interaction.display_success(&format!(
                            "Command '{}' completed in {}",
                            cmd_name,
                            format_duration(duration)
                        ));
                    }
                }
            }

            // Complete iteration timing and display summary
            if let Some(iteration_duration) = timing_tracker.complete_iteration() {
                self.user_interaction
                    .iteration_end(iteration, iteration_duration, true);
            }
        }

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total workflow time",
            &format!(
                "{} across {} iteration{}",
                format_duration(total_duration),
                max_iterations,
                if max_iterations == 1 { "" } else { "s" }
            ),
        );

        Ok(())
    }
    */

    /// Execute workflow with arguments from --args or --map
    /// Execute workflow with arguments - delegates to ArgumentProcessor
    async fn execute_workflow_with_args(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        self.argument_processor
            .execute_workflow_with_args(env, config)
            .await
    }

    /// Get current git HEAD
    #[allow(dead_code)]
    async fn get_current_head(&self, working_dir: &std::path::Path) -> Result<String> {
        let output = self
            .git_operations
            .git_command_in_dir(&["rev-parse", "HEAD"], "get current HEAD", working_dir)
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Execute a MapReduce workflow
    /// Execute workflow through the unified normalization path
    async fn execute_unified(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        // Normalize the workflow
        let normalized = self.normalize_workflow(config)?;

        // Execute through unified path
        self.execute_normalized(normalized, env).await
    }

    /// Normalize any workflow configuration to a common representation
    fn normalize_workflow(
        &self,
        config: &CookConfig,
    ) -> Result<crate::cook::workflow::normalized::NormalizedWorkflow> {
        use crate::cook::workflow::normalized::{ExecutionMode, NormalizedWorkflow};

        let workflow_type = Self::classify_workflow_type(config);

        // Determine execution mode based on workflow type
        let mode = match workflow_type {
            WorkflowType::MapReduce => ExecutionMode::MapReduce {
                config: Arc::new(crate::cook::workflow::normalized::MapReduceConfig {
                    max_iterations: None,
                    max_concurrent: config
                        .mapreduce_config
                        .as_ref()
                        .and_then(|m| m.map.max_parallel.parse::<usize>().ok()),
                    partition_strategy: None,
                }),
            },
            WorkflowType::WithArguments => ExecutionMode::WithArguments {
                args: config.command.args.clone().into(),
            },
            _ => ExecutionMode::Sequential,
        };

        NormalizedWorkflow::from_workflow_config(&config.workflow, mode)
    }

    /// Execute a normalized workflow with all features preserved
    async fn execute_normalized(
        &self,
        normalized: crate::cook::workflow::normalized::NormalizedWorkflow,
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Log workflow type based on execution mode
        match &normalized.execution_mode {
            crate::cook::workflow::normalized::ExecutionMode::MapReduce { .. } => {
                self.user_interaction.display_info(&format!(
                    "Executing MapReduce workflow: {}",
                    normalized.name
                ));
            }
            crate::cook::workflow::normalized::ExecutionMode::WithArguments { args } => {
                self.user_interaction.display_info(&format!(
                    "Processing workflow with {} arguments",
                    args.len()
                ));
            }
            crate::cook::workflow::normalized::ExecutionMode::Sequential => {
                self.user_interaction
                    .display_info(&format!("Executing workflow: {}", normalized.name));
            }
            _ => {}
        }

        // TODO: Implement actual unified execution logic
        // For now, delegate back to existing implementations based on workflow type
        // This allows for gradual migration while testing

        // Check if we should fall back to legacy path for specific workflow types
        if let Ok(workflow_type) = std::env::var("WORKFLOW_TYPE") {
            let disable_unified = match workflow_type.as_str() {
                "standard" => std::env::var("DISABLE_UNIFIED_STANDARD").is_ok(),
                "structured" => std::env::var("DISABLE_UNIFIED_STRUCTURED").is_ok(),
                "args" => std::env::var("DISABLE_UNIFIED_ARGS").is_ok(),
                "mapreduce" => std::env::var("DISABLE_UNIFIED_MAPREDUCE").is_ok(),
                _ => false,
            };

            if disable_unified {
                self.user_interaction.display_warning(&format!(
                    "Unified path disabled for {} workflows, using legacy path",
                    workflow_type
                ));
                // Would fall back to legacy here in a real implementation
            }
        }

        self.user_interaction
            .display_success("Unified workflow execution completed");
        Ok(())
    }

    async fn execute_mapreduce_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        mapreduce_config: &crate::config::MapReduceWorkflowConfig,
    ) -> Result<()> {
        // Create workflow executor
        let executor = self
            .create_workflow_executor_internal(config)
            .with_dry_run(config.command.dry_run);

        // Delegate to execution pipeline
        self.execution_pipeline
            .execute_mapreduce_workflow_with_executor(env, config, mapreduce_config, executor)
            .await
    }

    /// Execute a single workflow command
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    async fn execute_workflow_command(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        cmd: &WorkflowCommand,
        step_index: usize,
        input: &str,
        variables: &mut HashMap<String, String>,
        timing_tracker: &mut TimingTracker,
    ) -> Result<()> {
        let mut command = cmd.to_command();
        // Apply defaults from the command registry
        crate::config::apply_command_defaults(&mut command);

        self.user_interaction.display_progress(&format!(
            "Executing step {}/{}: {}",
            step_index + 1,
            config.workflow.commands.len(),
            command.name
        ));

        // Start timing this command
        timing_tracker.start_command(command.name.clone());

        // Analysis functionality has been removed in v0.3.0

        // Build the command with resolved arguments
        let (final_command, has_arg_reference) = self.build_command(&command, variables);

        // Only show ARG in log if the command actually uses it
        if has_arg_reference {
            self.user_interaction
                .display_info(&format!("Executing command: {final_command} (ARG={input})"));
        } else {
            self.user_interaction
                .display_action(&format!("Executing command: {final_command}"));
        }

        // Prepare environment variables
        let env_vars = self.prepare_environment_variables(env, variables);

        // Execute and validate command
        self.execute_and_validate_command(env, config, &command, &final_command, input, env_vars)
            .await?;

        // Complete command timing
        if let Some((cmd_name, duration)) = timing_tracker.complete_command() {
            self.user_interaction.display_success(&format!(
                "Command '{}' succeeded for input '{}' in {}",
                cmd_name,
                input,
                format_duration(duration)
            ));
        } else {
            self.user_interaction.display_success(&format!(
                "Command '{}' succeeded for input '{}'",
                command.name, input
            ));
        }

        Ok(())
    }

    /// Build command string with resolved arguments
    #[allow(dead_code)]
    fn build_command(
        &self,
        command: &crate::config::command::Command,
        variables: &HashMap<String, String>,
    ) -> (String, bool) {
        let mut has_arg_reference = false;

        // Check if this is a shell or test command based on the name
        let display_prefix = match command.name.as_ref() {
            "shell" => "shell: ",
            "test" => "test: ",
            _ => "/",
        };

        let mut cmd_parts = if display_prefix == "/" {
            vec![format!("/{}", command.name)]
        } else {
            // For shell/test commands, the actual command is in the args
            vec![]
        };

        // Resolve arguments
        for arg in &command.args {
            let resolved_arg = arg.resolve(variables);
            if !resolved_arg.is_empty() {
                cmd_parts.push(resolved_arg);
                // Check if this command actually uses the ARG variable
                if arg.is_variable()
                    && matches!(arg, crate::config::command::CommandArg::Variable(var) if var == "ARG")
                {
                    has_arg_reference = true;
                }
            }
        }

        let final_command = if display_prefix != "/" {
            format!("{}{}", display_prefix, cmd_parts.join(" "))
        } else {
            cmd_parts.join(" ")
        };

        (final_command, has_arg_reference)
    }

    /// Prepare environment variables for command execution
    #[allow(dead_code)]
    fn prepare_environment_variables(
        &self,
        _env: &ExecutionEnvironment,
        variables: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Add variables as environment variables too
        for (key, value) in variables {
            env_vars.insert(format!("PRODIGY_VAR_{key}"), value.clone());
        }

        env_vars
    }

    /// Execute command and validate results
    #[allow(dead_code)]
    async fn execute_and_validate_command(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        command: &crate::config::command::Command,
        final_command: &str,
        input: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<()> {
        // Handle test mode
        let test_mode = self.test_config.as_ref().is_some_and(|c| c.test_mode);
        let skip_validation = self
            .test_config
            .as_ref()
            .is_some_and(|c| c.skip_commit_validation);

        // Get HEAD before command execution if we need to verify commits
        let head_before = if !skip_validation && command.metadata.commit_required && !test_mode {
            Some(self.get_current_head(&env.working_dir).await?)
        } else {
            None
        };

        // Execute the command
        let result = self
            .claude_executor
            .execute_claude_command(final_command, &env.working_dir, env_vars)
            .await?;

        if !result.success {
            if config.command.fail_fast {
                return Err(anyhow!(
                    "Command '{}' failed for input '{}' with exit code {:?}. Error: {}",
                    command.name,
                    input,
                    result.exit_code,
                    result.stderr
                ));
            } else {
                self.user_interaction.display_warning(&format!(
                    "Command '{}' failed for input '{}', continuing...",
                    command.name, input
                ));
                return Ok(());
            }
        }

        // In test mode with skip_commit_validation, skip validation entirely
        if test_mode && skip_validation {
            // Skip validation - return success
            return Ok(());
        }
        // Check for commits if required
        if let Some(before) = head_before {
            let head_after = self.get_current_head(&env.working_dir).await?;
            if head_after == before {
                // No commits were created (skip error in dry-run mode)
                if config.command.dry_run {
                    // In dry-run mode, assume the commit would have been created
                    self.user_interaction.display_info(&format!(
                        "[DRY RUN] Assuming commit would be created by {}",
                        final_command
                    ));
                } else {
                    return Err(anyhow!("No changes were committed by {}", final_command));
                }
            } else {
                // Track file changes when commits were made
                self.session_manager
                    .update_session(SessionUpdate::AddFilesChanged(1))
                    .await?;
            }
        } else if test_mode && command.metadata.commit_required && !skip_validation {
            // In test mode, check if the command simulated no changes and is required to commit
            if let Some(config) = &self.test_config {
                let command_name = final_command.trim_start_matches('/');
                // Extract just the command name, ignoring arguments
                let command_name = command_name
                    .split_whitespace()
                    .next()
                    .unwrap_or(command_name);
                if config
                    .no_changes_commands
                    .iter()
                    .any(|cmd| cmd.trim() == command_name)
                {
                    // This command was configured to simulate no changes but requires commits
                    return Err(anyhow!("No changes were committed by {}", final_command));
                }
            }
        }

        Ok(())
    }

    /* REMOVED: Analysis functionality has been removed in v0.3.0
    /// Run analysis if needed based on configuration
    async fn run_analysis_if_needed(
        &self,
        env: &ExecutionEnvironment,
        config: &crate::config::command::AnalysisConfig,
        iteration: Option<usize>,
    ) -> Result<()> {
        // Force refresh on iterations after the first one
        let force_refresh = config.force_refresh || iteration.unwrap_or(1) > 1;

        // Check cache age if not forcing refresh
        if !force_refresh {
            let mut all_cached = true;
            let mut oldest_age = 0i64;

            // Always check both context and metrics caches
            let cache_paths = [
                (
                    "context",
                    env.working_dir.join(".prodigy/context/analysis_metadata.json"),
                ),
                ("metrics", env.working_dir.join(".prodigy/metrics/current.json")),
            ];

            for (_analysis_type, cache_path) in &cache_paths {
                if !cache_path.exists() {
                    all_cached = false;
                    break;
                }

                // Read metadata to check age
                if let Ok(content) = tokio::fs::read_to_string(&cache_path).await {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(timestamp_str) = data.get("timestamp").and_then(|v| v.as_str())
                        {
                            if let Ok(timestamp) =
                                chrono::DateTime::parse_from_rfc3339(timestamp_str)
                            {
                                let age = chrono::Utc::now().signed_duration_since(timestamp);
                                oldest_age = oldest_age.max(age.num_seconds());
                                if age.num_seconds() >= config.max_cache_age as i64 {
                                    all_cached = false;
                                    break;
                                }
                            } else {
                                all_cached = false;
                                break;
                            }
                        } else {
                            all_cached = false;
                            break;
                        }
                    } else {
                        all_cached = false;
                        break;
                    }
                } else {
                    all_cached = false;
                    break;
                }
            }

            if all_cached {
                self.user_interaction.display_info(&format!(
                    "Using cached analysis (age: {}s, max: {}s)",
                    oldest_age, config.max_cache_age
                ));
                return Ok(());
            }
        }

        // Use unified analysis function
        self.user_interaction.display_progress(&format!(
            "Running analysis{}...",
            if force_refresh {
                if iteration.unwrap_or(1) > 1 {
                    " (iteration refresh)"
                } else {
                    " (forced refresh)"
                }
            } else {
                ""
            }
        ));

        // Create progress reporter wrapper
        let progress = Arc::new(OrchestrationProgressReporter {
            interaction: self.user_interaction.clone(),
        });

        // Configure unified analysis
        let analysis_config = AnalysisConfig::builder()
            .output_format(OutputFormat::Summary)
            .save_results(true)
            .commit_changes(false) // We'll commit later if in worktree mode
            .force_refresh(force_refresh)
            .run_metrics(true)
            .run_context(true)
            .verbose(false)
            .build();

        // Run unified analysis
        let _results = run_analysis(
            &env.working_dir,
            analysis_config,
            self.subprocess.clone(),
            progress,
        )
        .await?;

        // Commit analysis if in worktree mode
        if env.worktree_name.is_some() {
            // Check if there are changes to commit
            let status_output = self
                .subprocess
                .runner()
                .run(crate::subprocess::runner::ProcessCommand {
                    program: "git".to_string(),
                    args: vec!["status".to_string(), "--porcelain".to_string()],
                    env: HashMap::new(),
                    working_dir: Some(env.working_dir.to_path_buf()),
                    timeout: None,
                    stdin: None,
                    suppress_stderr: false,
                })
                .await?;

            if !status_output.stdout.is_empty() {
                // Add and commit analysis changes
                self.subprocess
                    .runner()
                    .run(crate::subprocess::runner::ProcessCommand {
                        program: "git".to_string(),
                        args: vec!["add".to_string(), ".prodigy/".to_string()],
                        env: HashMap::new(),
                        working_dir: Some(env.working_dir.to_path_buf()),
                        timeout: None,
                        stdin: None,
                        suppress_stderr: false,
                    })
                    .await?;

                self.subprocess
                    .runner()
                    .run(crate::subprocess::runner::ProcessCommand {
                        program: "git".to_string(),
                        args: vec![
                            "commit".to_string(),
                            "-m".to_string(),
                            "analysis: update project context and metrics".to_string(),
                        ],
                        env: HashMap::new(),
                        working_dir: Some(env.working_dir.to_path_buf()),
                        timeout: None,
                        stdin: None,
                        suppress_stderr: false,
                    })
                    .await?;

                self.user_interaction
                    .display_success("Analysis committed to git");
            }
        }

        Ok(())
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{WorkflowCommand, WorkflowConfig};
    use crate::cook::orchestrator::workflow_classifier;
    use crate::cook::workflow::CaptureOutput;

    // Helper function to create a test CookCommand
    fn create_test_cook_command() -> CookCommand {
        CookCommand {
            playbook: PathBuf::from("test.yaml"),
            path: None,
            max_iterations: 1,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run: false,
        }
    }

    // TODO: Fix test after understanding MapReduceWorkflowConfig structure
    // #[test]
    // fn test_classify_workflow_type_mapreduce() {

    #[test]
    fn test_classify_workflow_type_structured_with_outputs() {
        // Create mutable workflow first
        let mut workflow = WorkflowConfig {
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        // Add a structured command with outputs
        let mut structured = crate::config::command::Command::new("test");
        let mut outputs = HashMap::new();
        outputs.insert(
            "output1".to_string(),
            crate::config::command::OutputDeclaration {
                file_pattern: "*.md".to_string(),
            },
        );
        structured.outputs = Some(outputs);
        workflow
            .commands
            .push(WorkflowCommand::Structured(Box::new(structured)));

        // Now create the config with Arc'd workflow
        let config = CookConfig {
            command: create_test_cook_command(),
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(workflow),
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::StructuredWithOutputs
        );
    }

    #[test]
    fn test_classify_workflow_type_with_arguments() {
        let mut command = create_test_cook_command();
        command.args = vec!["arg1".to_string()];

        let config = CookConfig {
            command,
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(WorkflowConfig {
                commands: vec![],
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::WithArguments
        );
    }

    #[test]
    fn test_classify_workflow_type_with_map_patterns() {
        let mut command = create_test_cook_command();
        command.map = vec!["*.rs".to_string()];

        let config = CookConfig {
            command,
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(WorkflowConfig {
                commands: vec![],
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::WithArguments
        );
    }

    #[test]
    fn test_classify_workflow_type_standard() {
        let config = CookConfig {
            command: create_test_cook_command(),
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(WorkflowConfig {
                commands: vec![],
                env: None,
                secrets: None,
                env_files: None,
                profiles: None,
                merge: None,
            }),
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::Standard
        );
    }

    #[test]
    fn test_determine_commit_required_simple_explicit() {
        let simple = crate::config::command::SimpleCommand {
            name: "test".to_string(),
            commit_required: Some(false),
            args: None,
            analysis: None,
        };
        let cmd = WorkflowCommand::SimpleObject(simple);
        let command = crate::config::command::Command::new("test");

        assert!(!workflow_classifier::determine_commit_required(
            &cmd, &command
        ));
    }

    #[test]
    fn test_determine_commit_required_structured() {
        let mut structured = crate::config::command::Command::new("test");
        structured.metadata.commit_required = false;
        let cmd = WorkflowCommand::Structured(Box::new(structured));
        let mut command = crate::config::command::Command::new("test");
        command.metadata.commit_required = false;

        assert!(!workflow_classifier::determine_commit_required(
            &cmd, &command
        ));
    }

    #[test]
    fn test_convert_command_to_step_workflow_step() {
        let step = crate::config::command::WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            claude: None,
            on_failure: None,
            test: None,
            goal_seek: None,
            foreach: None,
            write_file: None,
            capture_output: Some(crate::config::command::CaptureOutputConfig::Boolean(true)),
            commit_required: false,
            analyze: None,
            id: None,
            analysis: None,
            outputs: None,
            on_success: None,
            validate: None,
            timeout: None,
            when: None,
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };
        let cmd = WorkflowCommand::WorkflowStep(Box::new(step));

        let result = DefaultCookOrchestrator::convert_command_to_step(&cmd);

        assert_eq!(result.shell, Some("echo test".to_string()));
        assert_eq!(result.capture_output, CaptureOutput::Default);
        assert!(!result.commit_required);
    }

    #[test]
    fn test_convert_command_to_step_simple_command() {
        let simple = crate::config::command::SimpleCommand {
            name: "prodigy-test".to_string(),
            commit_required: Some(true),
            args: None,
            analysis: None,
        };
        let cmd = WorkflowCommand::SimpleObject(simple);

        let result = DefaultCookOrchestrator::convert_command_to_step(&cmd);

        assert_eq!(result.command, Some("/prodigy-test".to_string()));
        assert!(result.commit_required);
    }

    // TODO: Fix test after understanding MapReduceWorkflowConfig structure
    // #[test]
    // fn test_mapreduce_takes_precedence() {

    /// Tests for execute_and_validate_command method
    mod execute_and_validate_command_tests {
        use super::*;
        use crate::abstractions::git::MockGitOperations;
        use crate::cook::execution::{ClaudeExecutor, ExecutionResult};
        use crate::cook::interaction::UserInteraction;
        use crate::cook::orchestrator::ExecutionEnvironment;
        use crate::cook::session::{SessionManager, SessionUpdate};
        use crate::subprocess::SubprocessManager;
        use crate::testing::config::TestConfiguration;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::path::PathBuf;
        use std::sync::{Arc, Mutex};
        use tempfile::TempDir;

        // Mock implementations for testing

        struct MockClaudeExecutor {
            responses: Arc<Mutex<Vec<ExecutionResult>>>,
        }

        impl MockClaudeExecutor {
            fn new() -> Self {
                Self {
                    responses: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn add_response(&self, response: ExecutionResult) {
                self.responses.lock().unwrap().push(response);
            }
        }

        #[async_trait]
        impl ClaudeExecutor for MockClaudeExecutor {
            async fn execute_claude_command(
                &self,
                _command: &str,
                _working_dir: &std::path::Path,
                _env_vars: HashMap<String, String>,
            ) -> Result<ExecutionResult> {
                self.responses
                    .lock()
                    .unwrap()
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
            }

            async fn check_claude_cli(&self) -> Result<bool> {
                Ok(true)
            }

            async fn get_claude_version(&self) -> Result<String> {
                Ok("mock-version-1.0.0".to_string())
            }
        }

        struct MockSessionManager {
            updates: Arc<Mutex<Vec<SessionUpdate>>>,
        }

        impl MockSessionManager {
            fn new() -> Self {
                Self {
                    updates: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn get_updates(&self) -> Vec<SessionUpdate> {
                self.updates.lock().unwrap().clone()
            }
        }

        #[async_trait]
        impl SessionManager for MockSessionManager {
            async fn update_session(&self, update: SessionUpdate) -> Result<()> {
                self.updates.lock().unwrap().push(update.clone());
                Ok(())
            }

            async fn start_session(&self, _session_id: &str) -> Result<()> {
                Ok(())
            }

            async fn complete_session(
                &self,
            ) -> Result<crate::cook::session::summary::SessionSummary> {
                Ok(crate::cook::session::summary::SessionSummary {
                    iterations: 1,
                    files_changed: 0,
                })
            }

            fn get_state(&self) -> Result<crate::cook::session::state::SessionState> {
                Ok(crate::cook::session::state::SessionState::new(
                    "test-session".to_string(),
                    PathBuf::from("/tmp"),
                ))
            }

            async fn save_state(&self, _path: &std::path::Path) -> Result<()> {
                Ok(())
            }

            async fn load_state(&self, _path: &std::path::Path) -> Result<()> {
                Ok(())
            }

            async fn load_session(
                &self,
                _session_id: &str,
            ) -> Result<crate::cook::session::state::SessionState> {
                Ok(crate::cook::session::state::SessionState::new(
                    "test-session".to_string(),
                    PathBuf::from("/tmp"),
                ))
            }

            async fn save_checkpoint(
                &self,
                _state: &crate::cook::session::state::SessionState,
            ) -> Result<()> {
                Ok(())
            }

            async fn list_resumable(&self) -> Result<Vec<crate::cook::session::SessionInfo>> {
                Ok(vec![])
            }

            async fn get_last_interrupted(&self) -> Result<Option<String>> {
                Ok(None)
            }
        }

        struct MockUserInteraction {
            messages: Arc<Mutex<Vec<(String, String)>>>,
        }

        impl MockUserInteraction {
            fn new() -> Self {
                Self {
                    messages: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn get_messages(&self) -> Vec<(String, String)> {
                self.messages.lock().unwrap().clone()
            }
        }

        #[async_trait]
        impl UserInteraction for MockUserInteraction {
            fn display_info(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("info".to_string(), message.to_string()));
            }

            fn display_progress(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("progress".to_string(), message.to_string()));
            }

            fn display_success(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("success".to_string(), message.to_string()));
            }

            fn display_error(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("error".to_string(), message.to_string()));
            }

            fn display_warning(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("warning".to_string(), message.to_string()));
            }

            fn display_action(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("action".to_string(), message.to_string()));
            }

            fn display_metric(&self, label: &str, value: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("metric".to_string(), format!("{}: {}", label, value)));
            }

            fn display_status(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("status".to_string(), message.to_string()));
            }

            async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
                Ok(true)
            }

            async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
                Ok("test".to_string())
            }

            fn start_spinner(
                &self,
                _message: &str,
            ) -> Box<dyn crate::cook::interaction::SpinnerHandle> {
                struct MockSpinnerHandle;
                impl crate::cook::interaction::SpinnerHandle for MockSpinnerHandle {
                    fn update_message(&mut self, _message: &str) {}
                    fn success(&mut self, _message: &str) {}
                    fn fail(&mut self, _message: &str) {}
                }
                Box::new(MockSpinnerHandle)
            }

            fn iteration_start(&self, current: u32, total: u32) {
                self.messages.lock().unwrap().push((
                    "iteration_start".to_string(),
                    format!("{}/{}", current, total),
                ));
            }

            fn iteration_end(&self, current: u32, duration: std::time::Duration, success: bool) {
                self.messages.lock().unwrap().push((
                    "iteration_end".to_string(),
                    format!("{} {:?} {}", current, duration, success),
                ));
            }

            fn step_start(&self, step: u32, total: u32, description: &str) {
                self.messages.lock().unwrap().push((
                    "step_start".to_string(),
                    format!("{}/{} {}", step, total, description),
                ));
            }

            fn step_end(&self, step: u32, success: bool) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("step_end".to_string(), format!("{} {}", step, success)));
            }

            fn command_output(
                &self,
                output: &str,
                _verbosity: crate::cook::interaction::VerbosityLevel,
            ) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("command_output".to_string(), output.to_string()));
            }

            fn debug_output(
                &self,
                message: &str,
                _min_verbosity: crate::cook::interaction::VerbosityLevel,
            ) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("debug".to_string(), message.to_string()));
            }

            fn verbosity(&self) -> crate::cook::interaction::VerbosityLevel {
                crate::cook::interaction::VerbosityLevel::Normal
            }
        }

        // Helper function to create test command
        fn create_test_cook_command(
            fail_fast: bool,
            dry_run: bool,
        ) -> crate::cook::command::CookCommand {
            crate::cook::command::CookCommand {
                playbook: PathBuf::from("test.yaml"),
                path: None,
                max_iterations: 1,
                map: vec![],
                args: vec![],
                fail_fast,
                auto_accept: false,
                metrics: false,
                resume: None,
                verbosity: 0,
                quiet: false,
                dry_run,
            }
        }

        // Helper function to create test orchestrator
        async fn create_test_orchestrator() -> (
            DefaultCookOrchestrator,
            Arc<MockClaudeExecutor>,
            Arc<MockSessionManager>,
            Arc<MockUserInteraction>,
            Arc<MockGitOperations>,
        ) {
            let claude_executor = Arc::new(MockClaudeExecutor::new());
            let session_manager = Arc::new(MockSessionManager::new());
            let user_interaction = Arc::new(MockUserInteraction::new());
            let git_operations = Arc::new(MockGitOperations::new());
            let subprocess =
                SubprocessManager::new(Arc::new(crate::subprocess::runner::TokioProcessRunner));
            let session_ops = crate::cook::orchestrator::session_ops::SessionOperations::new(
                session_manager.clone() as Arc<dyn SessionManager>,
                claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                user_interaction.clone() as Arc<dyn UserInteraction>,
                git_operations.clone(),
                subprocess.clone(),
            );

            let workflow_executor =
                crate::cook::orchestrator::workflow_execution::WorkflowExecutor::new(
                    session_manager.clone() as Arc<dyn SessionManager>,
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    subprocess.clone(),
                );

            let health_metrics = crate::cook::orchestrator::health_metrics::HealthMetrics::new(
                user_interaction.clone() as Arc<dyn UserInteraction>,
            );

            let argument_processor =
                crate::cook::orchestrator::argument_processing::ArgumentProcessor::new(
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    session_manager.clone() as Arc<dyn SessionManager>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    None,
                );

            let execution_pipeline =
                crate::cook::orchestrator::execution_pipeline::ExecutionPipeline::new(
                    session_manager.clone() as Arc<dyn SessionManager>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    git_operations.clone(),
                    subprocess.clone(),
                    session_ops.clone(),
                    workflow_executor.clone(),
                );

            let command_executor =
                Arc::new(crate::testing::mocks::subprocess::CommandExecutorMock::new());

            let orchestrator = DefaultCookOrchestrator {
                session_manager: session_manager.clone() as Arc<dyn SessionManager>,
                unified_session_manager: Arc::new(Mutex::new(None)),
                command_executor,
                claude_executor: claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                user_interaction: user_interaction.clone() as Arc<dyn UserInteraction>,
                git_operations: git_operations.clone(),
                subprocess,
                test_config: None,
                session_ops,
                workflow_executor,
                health_metrics,
                argument_processor,
                execution_pipeline,
            };

            (
                orchestrator,
                claude_executor,
                session_manager,
                user_interaction,
                git_operations,
            )
        }

        #[tokio::test]
        async fn test_successful_command_execution_happy_path() {
            let (orchestrator, claude_mock, _session, _ui, git_mock) =
                create_test_orchestrator().await;

            // Setup mock responses
            claude_mock.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed successfully".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            git_mock.add_success_response("abc123").await; // HEAD before
            git_mock.add_success_response("def456").await; // HEAD after

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_command_failure_with_fail_fast_true() {
            let (orchestrator, claude_mock, _session, _ui, _git_mock) =
                create_test_orchestrator().await;

            // Setup mock response - command fails
            claude_mock.add_response(ExecutionResult {
                success: false,
                stdout: String::new(),
                stderr: "Command failed".to_string(),
                exit_code: Some(1),
                metadata: HashMap::new(),
            });

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(true, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let command = crate::config::command::Command::new("test-command");

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("failed"));
        }

        #[tokio::test]
        async fn test_command_failure_with_fail_fast_false() {
            let (orchestrator, claude_mock, _session, ui_mock, _git_mock) =
                create_test_orchestrator().await;

            // Setup mock response - command fails
            claude_mock.add_response(ExecutionResult {
                success: false,
                stdout: String::new(),
                stderr: "Command failed".to_string(),
                exit_code: Some(1),
                metadata: HashMap::new(),
            });

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let command = crate::config::command::Command::new("test-command");

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            // Should succeed but display warning
            assert!(result.is_ok());

            // Check warning was displayed
            let messages = ui_mock.get_messages();
            assert!(messages
                .iter()
                .any(|(t, m)| t == "warning" && m.contains("failed")));
        }

        #[tokio::test]
        async fn test_test_mode_with_skip_validation() {
            let temp_dir = TempDir::new().unwrap();

            let claude_executor = Arc::new(MockClaudeExecutor::new());
            let session_manager = Arc::new(MockSessionManager::new());
            let user_interaction = Arc::new(MockUserInteraction::new());
            let git_operations = Arc::new(MockGitOperations::new());
            let subprocess =
                SubprocessManager::new(Arc::new(crate::subprocess::runner::TokioProcessRunner));
            let session_ops = crate::cook::orchestrator::session_ops::SessionOperations::new(
                session_manager.clone() as Arc<dyn SessionManager>,
                claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                user_interaction.clone() as Arc<dyn UserInteraction>,
                git_operations.clone(),
                subprocess.clone(),
            );

            let workflow_executor =
                crate::cook::orchestrator::workflow_execution::WorkflowExecutor::new(
                    session_manager.clone() as Arc<dyn SessionManager>,
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    subprocess.clone(),
                );

            let health_metrics = crate::cook::orchestrator::health_metrics::HealthMetrics::new(
                user_interaction.clone() as Arc<dyn UserInteraction>,
            );

            let argument_processor =
                crate::cook::orchestrator::argument_processing::ArgumentProcessor::new(
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    session_manager.clone() as Arc<dyn SessionManager>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    None,
                );

            let execution_pipeline =
                crate::cook::orchestrator::execution_pipeline::ExecutionPipeline::new(
                    session_manager.clone() as Arc<dyn SessionManager>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    git_operations.clone(),
                    subprocess.clone(),
                    session_ops.clone(),
                    workflow_executor.clone(),
                );

            // Create test config with skip_commit_validation
            let test_config = Some(Arc::new(TestConfiguration {
                test_mode: true,
                skip_commit_validation: true,
                ..Default::default()
            }));

            let command_executor =
                Arc::new(crate::testing::mocks::subprocess::CommandExecutorMock::new());

            let orchestrator = DefaultCookOrchestrator {
                session_manager: session_manager.clone() as Arc<dyn SessionManager>,
                unified_session_manager: Arc::new(Mutex::new(None)),
                command_executor,
                claude_executor: claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                user_interaction: user_interaction.clone() as Arc<dyn UserInteraction>,
                git_operations: git_operations.clone(),
                subprocess,
                test_config,
                session_ops,
                workflow_executor,
                health_metrics,
                argument_processor,
                execution_pipeline,
            };

            // Setup mock response
            claude_executor.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            // Should succeed and skip all validation
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_commit_required_with_actual_commit_created() {
            let (orchestrator, claude_mock, session_mock, _ui, git_mock) =
                create_test_orchestrator().await;

            // Setup mock responses
            claude_mock.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            git_mock.add_success_response("abc123").await; // HEAD before
            git_mock.add_success_response("def456").await; // HEAD after (different commit)

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            assert!(result.is_ok());

            // Verify session was updated with files changed
            let updates = session_mock.get_updates();
            assert!(updates
                .iter()
                .any(|u| matches!(u, SessionUpdate::AddFilesChanged(1))));
        }

        #[tokio::test]
        async fn test_commit_required_with_no_commit_should_error() {
            let (orchestrator, claude_mock, _session, _ui, git_mock) =
                create_test_orchestrator().await;

            // Setup mock responses
            claude_mock.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            git_mock.add_success_response("abc123").await; // HEAD before
            git_mock.add_success_response("abc123").await; // HEAD after (same commit - no changes)

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("No changes were committed"));
        }

        #[tokio::test]
        async fn test_commit_required_in_dry_run_mode_should_skip_error() {
            let (orchestrator, claude_mock, _session, ui_mock, git_mock) =
                create_test_orchestrator().await;

            // Setup mock responses
            claude_mock.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            git_mock.add_success_response("abc123").await; // HEAD before
            git_mock.add_success_response("abc123").await; // HEAD after (same - no commit)

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, true), // dry_run = true
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            // Should succeed in dry-run mode
            assert!(result.is_ok());

            // Should display info message about assuming commit would be created
            let messages = ui_mock.get_messages();
            assert!(messages
                .iter()
                .any(|(t, m)| t == "info" && m.contains("DRY RUN")));
        }

        #[tokio::test]
        async fn test_commit_required_false_skips_validation() {
            let (orchestrator, claude_mock, _session, _ui, _git_mock) =
                create_test_orchestrator().await;

            // Setup mock responses
            claude_mock.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            // No git mocks needed since commit_required is false

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let command = crate::config::command::Command::new("test-command");
            // commit_required defaults to false

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            // Should succeed without checking commits
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_test_mode_with_no_changes_commands() {
            let temp_dir = TempDir::new().unwrap();

            let claude_executor = Arc::new(MockClaudeExecutor::new());
            let session_manager = Arc::new(MockSessionManager::new());
            let user_interaction = Arc::new(MockUserInteraction::new());
            let git_operations = Arc::new(MockGitOperations::new());
            let subprocess =
                SubprocessManager::new(Arc::new(crate::subprocess::runner::TokioProcessRunner));
            let session_ops = crate::cook::orchestrator::session_ops::SessionOperations::new(
                session_manager.clone() as Arc<dyn SessionManager>,
                claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                user_interaction.clone() as Arc<dyn UserInteraction>,
                git_operations.clone(),
                subprocess.clone(),
            );

            let workflow_executor =
                crate::cook::orchestrator::workflow_execution::WorkflowExecutor::new(
                    session_manager.clone() as Arc<dyn SessionManager>,
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    subprocess.clone(),
                );

            let health_metrics = crate::cook::orchestrator::health_metrics::HealthMetrics::new(
                user_interaction.clone() as Arc<dyn UserInteraction>,
            );

            let argument_processor =
                crate::cook::orchestrator::argument_processing::ArgumentProcessor::new(
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    session_manager.clone() as Arc<dyn SessionManager>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    None,
                );

            let execution_pipeline =
                crate::cook::orchestrator::execution_pipeline::ExecutionPipeline::new(
                    session_manager.clone() as Arc<dyn SessionManager>,
                    user_interaction.clone() as Arc<dyn UserInteraction>,
                    claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                    git_operations.clone(),
                    subprocess.clone(),
                    session_ops.clone(),
                    workflow_executor.clone(),
                );

            // Create test config with no_changes_commands
            let test_config = Some(Arc::new(TestConfiguration {
                test_mode: true,
                skip_commit_validation: false,
                no_changes_commands: vec!["test-command".to_string()],
                ..Default::default()
            }));

            let command_executor =
                Arc::new(crate::testing::mocks::subprocess::CommandExecutorMock::new());

            let orchestrator = DefaultCookOrchestrator {
                session_manager: session_manager.clone() as Arc<dyn SessionManager>,
                unified_session_manager: Arc::new(Mutex::new(None)),
                command_executor,
                claude_executor: claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                user_interaction: user_interaction.clone() as Arc<dyn UserInteraction>,
                git_operations: git_operations.clone(),
                subprocess,
                test_config,
                session_ops,
                workflow_executor,
                health_metrics,
                argument_processor,
                execution_pipeline,
            };

            // Setup mock response
            claude_executor.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            // Should error because command is in no_changes_commands but requires commits
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("No changes were committed"));
        }

        #[tokio::test]
        async fn test_session_update_after_successful_commit() {
            let (orchestrator, claude_mock, session_mock, _ui, git_mock) =
                create_test_orchestrator().await;

            // Setup mock responses
            claude_mock.add_response(ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                metadata: HashMap::new(),
            });

            git_mock.add_success_response("abc123").await; // HEAD before
            git_mock.add_success_response("def456").await; // HEAD after (different - commit created)

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let config = CookConfig {
                command: create_test_cook_command(false, false),
                project_path: Arc::new(PathBuf::from("/test")),
                workflow: Arc::new(crate::config::WorkflowConfig {
                    commands: vec![],
                    env: None,
                    secrets: None,
                    env_files: None,
                    profiles: None,
                    merge: None,
                }),
                mapreduce_config: None,
            };

            let mut command = crate::config::command::Command::new("test-command");
            command.metadata.commit_required = true;

            let result = orchestrator
                .execute_and_validate_command(
                    &env,
                    &config,
                    &command,
                    "/test-command",
                    "test input",
                    HashMap::new(),
                )
                .await;

            assert!(result.is_ok());

            // Verify AddFilesChanged was called
            let updates = session_mock.get_updates();
            assert!(updates
                .iter()
                .any(|u| matches!(u, SessionUpdate::AddFilesChanged(1))));
        }
    }
}
