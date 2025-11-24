//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using pure planning and effect composition.
//!
//! # Architecture
//!
//! The orchestrator follows the "pure core, imperative shell" pattern:
//! - Pure planning via `plan_execution()` from `core::orchestration`
//! - Effect composition via `effects` module
//! - I/O at the boundaries only
//!
//! The refactored orchestrator:
//! - Uses pure `ExecutionPlan` to drive all decisions
//! - Delegates to specialized executors (MapReduce, Standard, etc.)
//! - Keeps only I/O coordination logic (~400 LOC)

use crate::abstractions::git::GitOperations;
use crate::config::{WorkflowCommand, WorkflowConfig};
use crate::core::orchestration::{plan_execution, ExecutionMode, ExecutionPlan};
use crate::testing::config::TestConfiguration;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::cook::command::CookCommand;
use crate::cook::execution::{ClaudeExecutor, CommandExecutor};
use crate::cook::interaction::UserInteraction;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{ExtendedWorkflowConfig, WorkflowStep};

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
///
/// Uses pure planning from `core::orchestration` to drive execution decisions.
pub struct DefaultCookOrchestrator {
    session_manager: Arc<dyn SessionManager>,
    unified_session_manager: Arc<Mutex<Option<Arc<crate::unified_session::SessionManager>>>>,
    #[allow(dead_code)]
    command_executor: Arc<dyn CommandExecutor>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
    #[allow(dead_code)]
    git_operations: Arc<dyn GitOperations>,
    subprocess: crate::subprocess::SubprocessManager,
    #[allow(dead_code)]
    test_config: Option<Arc<TestConfiguration>>,
    session_ops: super::session_ops::SessionOperations,
    #[allow(dead_code)]
    workflow_executor: super::workflow_execution::WorkflowExecutor,
    argument_processor: super::argument_processing::ArgumentProcessor,
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
        Self::from_builder(
            session_manager,
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
            None,
        )
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
            argument_processor,
            execution_pipeline,
        }
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
        Self::from_builder(
            session_manager,
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
            Some(test_config),
        )
    }

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

    /// Classify workflow type using pure function
    pub(crate) fn classify_workflow_type(config: &CookConfig) -> WorkflowType {
        super::workflow_classifier::classify_workflow_type(config)
    }

    /// Convert a workflow command to a workflow step
    fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
        super::workflow_execution::WorkflowExecutor::convert_command_to_step(cmd)
    }

    // --- I/O Operations (Thin Layer) ---

    async fn get_unified_session_manager(
        &self,
    ) -> Result<Arc<crate::unified_session::SessionManager>> {
        if let Some(m) = self
            .unified_session_manager
            .lock()
            .map_err(|e| anyhow!("Lock: {}", e))?
            .as_ref()
        {
            return Ok(Arc::clone(m));
        }
        let storage = crate::storage::GlobalStorage::new().context("Create storage")?;
        let manager = Arc::new(
            crate::unified_session::SessionManager::new(storage)
                .await
                .context("Create manager")?,
        );
        *self
            .unified_session_manager
            .lock()
            .map_err(|e| anyhow!("Lock: {}", e))? = Some(Arc::clone(&manager));
        Ok(manager)
    }

    async fn update_unified_session_status(&self, session_id: &str, success: bool) {
        let Ok(manager) = self.get_unified_session_manager().await else {
            return;
        };
        let id = crate::unified_session::SessionId::from_string(session_id.to_string());
        if manager.load_session(&id).await.is_ok() {
            let _ = manager.complete_session(&id, success).await;
        }
    }

    async fn create_unified_session(&self, config: &CookConfig) -> Result<String> {
        let manager = self.get_unified_session_manager().await?;
        let wf_id = super::construction::generate_workflow_id();
        let cfg = super::construction::build_session_config(
            wf_id.clone(),
            config.workflow.name.clone(),
            super::construction::create_session_metadata(config.workflow.commands.len()),
        );
        let id = manager.create_session(cfg).await?;
        manager.start_session(&id).await?;
        log::info!("Created session: {} (workflow: {})", id, wf_id);
        Ok(id.to_string())
    }

    async fn create_worktree(
        &self,
        config: &CookConfig,
        session_id: &str,
    ) -> Result<(Arc<PathBuf>, Option<Arc<str>>)> {
        let manager = WorktreeManager::with_config(
            config.project_path.to_path_buf(),
            self.subprocess.clone(),
            config.command.verbosity,
            super::construction::extract_merge_config(&config.workflow, &config.mapreduce_config),
            super::construction::extract_workflow_env(&config.workflow),
        )?;
        let session = manager.create_session_with_id(session_id).await?;
        self.user_interaction
            .display_info(&format!("Created worktree at: {}", session.path.display()));
        Ok((
            Arc::new(session.path.clone()),
            Some(Arc::from(session.name.as_ref())),
        ))
    }

    async fn cleanup_worktree(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        worktree: &str,
    ) -> Result<()> {
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        let merge_config =
            super::construction::extract_merge_config(&config.workflow, &config.mapreduce_config);
        let workflow_env = super::construction::extract_workflow_env(&config.workflow);

        let manager = WorktreeManager::with_config(
            env.project_dir.to_path_buf(),
            self.subprocess.clone(),
            config.command.verbosity,
            merge_config,
            workflow_env,
        )?;

        let should_merge =
            match super::construction::should_merge_worktree(test_mode, config.command.auto_accept)
            {
                Some(decision) => decision,
                None => {
                    let target = manager
                        .get_merge_target(worktree)
                        .await
                        .unwrap_or_else(|_| "master".to_string());
                    self.user_interaction
                        .prompt_yes_no(&format!("Merge {} to {}", worktree, target))
                        .await?
                }
            };

        if should_merge {
            manager.merge_session(worktree).await?;
            self.user_interaction
                .display_success("Worktree changes merged successfully!");
        }
        Ok(())
    }

    // --- Execution Dispatch (Uses Pure Plan) ---

    async fn execute_by_mode(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        plan: &ExecutionPlan,
    ) -> Result<()> {
        // DryRun mode with mapreduce_config should still go through MapReduce path
        let is_mapreduce = config.mapreduce_config.is_some();
        let effective_mode = if plan.mode == ExecutionMode::DryRun && is_mapreduce {
            ExecutionMode::MapReduce
        } else {
            plan.mode
        };

        match effective_mode {
            ExecutionMode::MapReduce => {
                let mr_config = config.mapreduce_config.as_ref().ok_or_else(|| {
                    anyhow!("MapReduce workflow requires mapreduce configuration")
                })?;
                self.execution_pipeline
                    .execute_mapreduce_workflow_with_executor(
                        env,
                        config,
                        mr_config,
                        self.create_workflow_executor_internal(config)
                            .with_dry_run(config.command.dry_run),
                    )
                    .await
            }
            ExecutionMode::Iterative => {
                self.user_interaction
                    .display_info("Processing workflow with arguments or file patterns");
                self.argument_processor
                    .execute_workflow_with_args(env, config)
                    .await
            }
            // DryRun uses standard workflow path with dry_run=true on executor
            ExecutionMode::DryRun | ExecutionMode::Standard => {
                self.execute_standard_workflow(env, config).await
            }
        }
    }

    async fn execute_standard_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        if Self::classify_workflow_type(config) == WorkflowType::StructuredWithOutputs {
            return self
                .execution_pipeline
                .execute_structured_workflow(env, config)
                .await;
        }

        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .map(Self::convert_command_to_step)
            .collect();
        let extended = ExtendedWorkflowConfig {
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
        };

        let checkpoint_mgr = Arc::new(crate::cook::workflow::CheckpointManager::with_storage(
            crate::cook::workflow::CheckpointStorage::Session {
                session_id: env.session_id.to_string(),
            },
        ));
        let mut executor = self
            .create_workflow_executor_internal(config)
            .with_checkpoint_manager(
                checkpoint_mgr,
                format!("workflow-{}", chrono::Utc::now().timestamp_millis()),
            )
            .with_dry_run(config.command.dry_run);

        if config.workflow.env.is_some()
            || config.workflow.secrets.is_some()
            || config.workflow.env_files.is_some()
            || config.workflow.profiles.is_some()
        {
            executor = executor.with_environment_config(super::construction::create_env_config(
                &config.workflow,
            ))?;
        }
        executor.execute(&extended, env).await
    }
}

#[async_trait]
impl CookOrchestrator for DefaultCookOrchestrator {
    async fn run(&self, config: CookConfig) -> Result<()> {
        // Handle resume
        if let Some(session_id) = config.command.resume.clone() {
            return self
                .execution_pipeline
                .resume_workflow(&session_id, config)
                .await;
        }

        // Pure planning - drives all decisions
        let plan = plan_execution(&config);
        log::debug!(
            "Execution plan: mode={:?}, phases={}",
            plan.mode,
            plan.phase_count()
        );

        // Check prerequisites
        self.session_ops
            .check_prerequisites_with_config(&config)
            .await?;

        // Setup environment (I/O)
        let env = self.setup_environment(&config).await?;

        // Initialize session metadata
        self.execution_pipeline
            .initialize_session_metadata(&env.session_id, &config)
            .await?;

        // Setup signal handlers
        let interrupt_handler = self.execution_pipeline.setup_signal_handlers(
            &config,
            &env.session_id,
            env.worktree_name.as_ref().map(Arc::clone),
        )?;

        // Execute by mode (determined by pure plan)
        let execution_result = self.execute_by_mode(&env, &config, &plan).await;

        interrupt_handler.abort();

        // Update session status
        self.update_unified_session_status(&env.session_id, execution_result.is_ok())
            .await;

        // Finalize
        self.execution_pipeline
            .finalize_session(&env, &config, execution_result, self.cleanup(&env, &config))
            .await
    }

    async fn check_prerequisites(&self) -> Result<()> {
        self.session_ops.check_prerequisites().await
    }

    async fn setup_environment(&self, config: &CookConfig) -> Result<ExecutionEnvironment> {
        let mut session_id = Arc::from(self.session_ops.generate_session_id().as_str());

        if super::construction::should_create_unified_session(
            config.mapreduce_config.is_some(),
            config.command.dry_run,
        ) {
            session_id = Arc::from(self.create_unified_session(config).await?.as_str());
        }

        let (working_dir, worktree_name) = if !config.command.dry_run {
            self.create_worktree(config, &session_id).await?
        } else {
            self.user_interaction
                .display_info("[DRY RUN] Would create worktree for isolated execution");
            (Arc::clone(&config.project_path), None)
        };

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
        // Use pure planning to determine mode
        let plan = plan_execution(config);
        self.execute_by_mode(env, config, &plan).await
    }

    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        let session_state_path = env.working_dir.join(".prodigy/session_state.json");
        self.session_manager.save_state(&session_state_path).await?;

        if let Some(ref worktree_name) = env.worktree_name {
            self.cleanup_worktree(env, config, worktree_name).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cook_command() -> CookCommand {
        CookCommand {
            playbook: PathBuf::from("test.yaml"),
            path: None,
            max_iterations: 1,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run: false,
            params: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_classify_workflow_type_standard() {
        let config = CookConfig {
            command: create_test_cook_command(),
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(WorkflowConfig {
                name: None,
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
    fn test_classify_workflow_type_with_arguments() {
        let mut command = create_test_cook_command();
        command.args = vec!["arg1".to_string()];

        let config = CookConfig {
            command,
            project_path: Arc::new(PathBuf::from("/test")),
            workflow: Arc::new(WorkflowConfig {
                name: None,
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
                name: None,
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
}
