//! Core types for agent execution
//!
//! This module contains the fundamental data structures and traits used for
//! managing agent lifecycle, execution, and results in the MapReduce framework.

use crate::cook::workflow::WorkflowStep;
use crate::worktree::WorktreeSession;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Status of an agent's execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    /// Agent is waiting to start
    Pending,
    /// Agent is currently running
    Running,
    /// Agent completed successfully
    Success,
    /// Agent execution failed
    Failed(String),
    /// Agent execution timed out
    Timeout,
    /// Agent is retrying after failure
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
    /// Path to Claude JSON log file for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_log_location: Option<String>,
}

impl AgentResult {
    /// Create a new successful agent result
    pub fn success(item_id: String, output: Option<String>, duration: Duration) -> Self {
        Self {
            item_id,
            status: AgentStatus::Success,
            output,
            commits: Vec::new(),
            files_modified: Vec::new(),
            duration,
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
        }
    }

    /// Create a new failed agent result
    pub fn failed(item_id: String, error: String, duration: Duration) -> Self {
        Self {
            item_id,
            status: AgentStatus::Failed(error.clone()),
            output: None,
            commits: Vec::new(),
            files_modified: Vec::new(),
            duration,
            error: Some(error),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
        }
    }

    /// Check if the agent result represents a successful execution
    pub fn is_success(&self) -> bool {
        matches!(self.status, AgentStatus::Success)
    }

    /// Check if the agent result represents a failed execution
    pub fn is_failure(&self) -> bool {
        matches!(self.status, AgentStatus::Failed(_) | AgentStatus::Timeout)
    }
}

/// Configuration for an agent execution
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Unique identifier for the agent
    pub id: String,
    /// ID of the work item being processed
    pub item_id: String,
    /// Branch name for the agent's worktree
    pub branch_name: String,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Timeout for agent execution
    pub timeout: Duration,
    /// Index of this agent in the pool
    pub agent_index: usize,
    /// Total number of work items
    pub total_items: usize,
}

impl AgentConfig {
    /// Create a new agent configuration
    pub fn new(
        id: String,
        item_id: String,
        branch_name: String,
        max_retries: u32,
        timeout: Duration,
        agent_index: usize,
        total_items: usize,
    ) -> Self {
        Self {
            id,
            item_id,
            branch_name,
            max_retries,
            timeout,
            agent_index,
            total_items,
        }
    }
}

/// Handle for managing an agent's lifecycle
#[derive(Debug)]
pub struct AgentHandle {
    /// Configuration for the agent
    pub config: AgentConfig,
    /// Worktree session for isolated execution
    pub worktree_session: WorktreeSession,
    /// Current state of the agent
    pub state: Arc<RwLock<AgentState>>,
    /// Commands to be executed by the agent
    pub commands: Vec<WorkflowStep>,
}

impl AgentHandle {
    /// Create a new agent handle
    pub fn new(
        config: AgentConfig,
        worktree_session: WorktreeSession,
        commands: Vec<WorkflowStep>,
    ) -> Self {
        Self {
            config,
            worktree_session,
            state: Arc::new(RwLock::new(AgentState::default())),
            commands,
        }
    }

    /// Get the agent's unique identifier
    pub fn id(&self) -> &str {
        &self.config.id
    }

    /// Get the work item ID being processed
    pub fn item_id(&self) -> &str {
        &self.config.item_id
    }

    /// Get the worktree path for this agent
    pub fn worktree_path(&self) -> &std::path::Path {
        &self.worktree_session.path
    }
}

/// State tracking for an agent during execution
#[derive(Debug, Clone, Default)]
pub struct AgentState {
    /// Current status of the agent
    pub status: AgentStateStatus,
    /// Current operation being performed
    pub current_operation: Option<String>,
    /// Number of retry attempts made
    pub retry_count: u32,
    /// Progress through the command list (current command index)
    pub command_progress: usize,
    /// Total number of commands
    pub total_commands: usize,
    /// Start time of current operation
    pub operation_start: Option<std::time::Instant>,
}

/// Status of an agent's state
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AgentStateStatus {
    #[default]
    /// Agent is idle/waiting
    Idle,
    /// Agent is initializing
    Initializing,
    /// Agent is executing commands
    Executing,
    /// Agent is retrying after failure
    Retrying(u32),
    /// Agent has completed successfully
    Completed,
    /// Agent has failed
    Failed(String),
    /// Agent execution timed out
    TimedOut,
}

impl AgentState {
    /// Update the current operation
    pub fn set_operation(&mut self, operation: String) {
        self.current_operation = Some(operation);
        self.operation_start = Some(std::time::Instant::now());
    }

    /// Clear the current operation
    pub fn clear_operation(&mut self) {
        self.current_operation = None;
        self.operation_start = None;
    }

    /// Update command progress
    pub fn update_progress(&mut self, current: usize, total: usize) {
        self.command_progress = current;
        self.total_commands = total;
    }

    /// Mark as retrying
    pub fn mark_retrying(&mut self, attempt: u32) {
        self.status = AgentStateStatus::Retrying(attempt);
        self.retry_count = attempt;
    }

    /// Mark as completed
    pub fn mark_completed(&mut self) {
        self.status = AgentStateStatus::Completed;
        self.clear_operation();
    }

    /// Mark as failed
    pub fn mark_failed(&mut self, error: String) {
        self.status = AgentStateStatus::Failed(error);
        self.clear_operation();
    }

    /// Mark as timed out
    pub fn mark_timed_out(&mut self) {
        self.status = AgentStateStatus::TimedOut;
        self.clear_operation();
    }
}

/// Agent operation types for detailed status display
#[derive(Debug, Clone)]
pub enum AgentOperation {
    /// Agent is idle
    Idle,
    /// Agent is performing setup
    Setup(String),
    /// Agent is executing Claude command
    Claude(String),
    /// Agent is executing shell command
    Shell(String),
    /// Agent is running tests
    Test(String),
    /// Agent is executing error handler
    Handler(String),
    /// Agent is retrying an operation
    Retrying(String, u32),
    /// Agent has completed all operations
    Complete,
}

impl AgentOperation {
    /// Get a display string for the operation
    pub fn display(&self) -> String {
        match self {
            Self::Idle => "Idle".to_string(),
            Self::Setup(msg) => format!("Setup: {}", msg),
            Self::Claude(cmd) => format!("Claude: {}", cmd),
            Self::Shell(cmd) => format!("Shell: {}", cmd),
            Self::Test(cmd) => format!("Test: {}", cmd),
            Self::Handler(cmd) => format!("Handler: {}", cmd),
            Self::Retrying(msg, attempt) => format!("Retrying ({}): {}", attempt, msg),
            Self::Complete => "Complete".to_string(),
        }
    }
}

// ============================================================================
// State Machine Types (Pure State Model)
// ============================================================================

/// Pure agent state enum for explicit state machine transitions
#[derive(Debug, Clone, PartialEq)]
pub enum AgentLifecycleState {
    /// Agent created but not started
    Created { agent_id: String, work_item: Value },
    /// Agent is currently executing
    Running {
        agent_id: String,
        started_at: Instant,
        worktree_path: PathBuf,
    },
    /// Agent completed successfully
    Completed {
        agent_id: String,
        output: Option<String>,
        commits: Vec<String>,
        duration: Duration,
    },
    /// Agent failed with error
    Failed {
        agent_id: String,
        error: String,
        duration: Duration,
        json_log_location: Option<String>,
    },
}

/// Transitions between agent states
#[derive(Debug, Clone)]
pub enum AgentTransition {
    /// Transition from Created to Running
    Start { worktree_path: PathBuf },
    /// Transition from Running to Completed
    Complete {
        output: Option<String>,
        commits: Vec<String>,
    },
    /// Transition from Running to Failed
    Fail {
        error: String,
        json_log_location: Option<String>,
    },
}
