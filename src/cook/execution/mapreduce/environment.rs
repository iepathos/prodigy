//! Environment types for MapReduce effect execution
//!
//! This module defines environment types used with Stillwater's Effect pattern
//! for dependency injection in MapReduce operations.

use crate::cook::execution::mapreduce::agent_command_executor::AgentCommandExecutor;
use crate::cook::execution::mapreduce::checkpoint::storage::CheckpointStorage;
use crate::cook::workflow::WorkflowStep;
use crate::worktree::WorktreeManager;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Environment for map phase operations
///
/// Provides dependencies needed for executing agents including worktree
/// management, command execution, and checkpoint storage.
#[derive(Clone)]
pub struct MapEnv {
    /// Worktree manager for git operations
    pub worktree_manager: Arc<WorktreeManager>,
    /// Command executor for agent commands
    pub command_executor: Arc<AgentCommandExecutor>,
    /// Checkpoint storage for state persistence
    pub storage: Arc<dyn CheckpointStorage>,
    /// Agent template commands
    pub agent_template: Vec<WorkflowStep>,
    /// Job ID for tracking
    pub job_id: String,
    /// Maximum parallel agents
    pub max_parallel: usize,
    /// Workflow environment variables
    pub workflow_env: HashMap<String, Value>,
    /// Additional configuration
    pub config: HashMap<String, Value>,
}

/// Environment for phase operations (setup/reduce)
///
/// Provides dependencies for non-agent phases including command execution
/// and state management.
#[derive(Clone)]
pub struct PhaseEnv {
    /// Command executor for phase commands
    pub command_executor: Arc<AgentCommandExecutor>,
    /// Checkpoint storage for state persistence
    pub storage: Arc<dyn CheckpointStorage>,
    /// Variables from workflow and previous phases
    pub variables: HashMap<String, Value>,
    /// Workflow environment variables
    pub workflow_env: HashMap<String, Value>,
}

/// Parameters for constructing MapEnv
pub struct MapEnvParams {
    pub worktree_manager: Arc<WorktreeManager>,
    pub command_executor: Arc<AgentCommandExecutor>,
    pub storage: Arc<dyn CheckpointStorage>,
    pub agent_template: Vec<WorkflowStep>,
    pub job_id: String,
    pub max_parallel: usize,
    pub workflow_env: HashMap<String, Value>,
    pub config: HashMap<String, Value>,
}

impl MapEnv {
    /// Create a new map environment
    pub fn new(params: MapEnvParams) -> Self {
        let worktree_manager = params.worktree_manager;
        let command_executor = params.command_executor;
        let storage = params.storage;
        let agent_template = params.agent_template;
        let job_id = params.job_id;
        let max_parallel = params.max_parallel;
        let workflow_env = params.workflow_env;
        let config = params.config;

        Self {
            worktree_manager,
            command_executor,
            storage,
            agent_template,
            job_id,
            max_parallel,
            workflow_env,
            config,
        }
    }
}

impl PhaseEnv {
    /// Create a new phase environment
    pub fn new(
        command_executor: Arc<AgentCommandExecutor>,
        storage: Arc<dyn CheckpointStorage>,
        variables: HashMap<String, Value>,
        workflow_env: HashMap<String, Value>,
    ) -> Self {
        Self {
            command_executor,
            storage,
            variables,
            workflow_env,
        }
    }
}
