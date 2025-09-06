//! Event types for MapReduce execution

use crate::cook::execution::mapreduce::MapReduceConfig;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All possible events during MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum MapReduceEvent {
    // Job lifecycle events
    JobStarted {
        job_id: String,
        config: MapReduceConfig,
        total_items: usize,
        timestamp: DateTime<Utc>,
    },
    JobCompleted {
        job_id: String,
        duration: Duration,
        success_count: usize,
        failure_count: usize,
    },
    JobFailed {
        job_id: String,
        error: String,
        partial_results: usize,
    },
    JobPaused {
        job_id: String,
        checkpoint_version: u32,
    },
    JobResumed {
        job_id: String,
        checkpoint_version: u32,
        pending_items: usize,
    },

    // Agent lifecycle events
    AgentStarted {
        job_id: String,
        agent_id: String,
        item_id: String,
        worktree: String,
        attempt: u32,
    },
    AgentProgress {
        job_id: String,
        agent_id: String,
        step: String,
        progress_pct: f32,
    },
    AgentCompleted {
        job_id: String,
        agent_id: String,
        duration: Duration,
        commits: Vec<String>,
    },
    AgentFailed {
        job_id: String,
        agent_id: String,
        error: String,
        retry_eligible: bool,
    },
    AgentRetrying {
        job_id: String,
        agent_id: String,
        attempt: u32,
        backoff_ms: u64,
    },

    // Checkpoint events
    CheckpointCreated {
        job_id: String,
        version: u32,
        agents_completed: usize,
    },
    CheckpointLoaded {
        job_id: String,
        version: u32,
    },
    CheckpointFailed {
        job_id: String,
        error: String,
    },

    // Worktree events
    WorktreeCreated {
        job_id: String,
        agent_id: String,
        worktree_name: String,
        branch: String,
    },
    WorktreeMerged {
        job_id: String,
        agent_id: String,
        target_branch: String,
    },
    WorktreeCleaned {
        job_id: String,
        agent_id: String,
        worktree_name: String,
    },

    // Performance events
    QueueDepthChanged {
        job_id: String,
        pending: usize,
        active: usize,
        completed: usize,
    },
    MemoryPressure {
        job_id: String,
        used_mb: usize,
        limit_mb: usize,
    },
}

impl MapReduceEvent {
    /// Get the job ID associated with this event
    pub fn job_id(&self) -> &str {
        use MapReduceEvent::*;
        match self {
            JobStarted { job_id, .. }
            | JobCompleted { job_id, .. }
            | JobFailed { job_id, .. }
            | JobPaused { job_id, .. }
            | JobResumed { job_id, .. }
            | AgentStarted { job_id, .. }
            | AgentProgress { job_id, .. }
            | AgentCompleted { job_id, .. }
            | AgentFailed { job_id, .. }
            | AgentRetrying { job_id, .. }
            | CheckpointCreated { job_id, .. }
            | CheckpointLoaded { job_id, .. }
            | CheckpointFailed { job_id, .. }
            | WorktreeCreated { job_id, .. }
            | WorktreeMerged { job_id, .. }
            | WorktreeCleaned { job_id, .. }
            | QueueDepthChanged { job_id, .. }
            | MemoryPressure { job_id, .. } => job_id,
        }
    }

    /// Get the agent ID if this event is agent-specific
    pub fn agent_id(&self) -> Option<&str> {
        use MapReduceEvent::*;
        match self {
            AgentStarted { agent_id, .. }
            | AgentProgress { agent_id, .. }
            | AgentCompleted { agent_id, .. }
            | AgentFailed { agent_id, .. }
            | AgentRetrying { agent_id, .. }
            | WorktreeCreated { agent_id, .. }
            | WorktreeMerged { agent_id, .. }
            | WorktreeCleaned { agent_id, .. } => Some(agent_id),
            _ => None,
        }
    }

    /// Get a human-readable name for this event type
    pub fn event_name(&self) -> &'static str {
        use MapReduceEvent::*;
        match self {
            JobStarted { .. } => "job_started",
            JobCompleted { .. } => "job_completed",
            JobFailed { .. } => "job_failed",
            JobPaused { .. } => "job_paused",
            JobResumed { .. } => "job_resumed",
            AgentStarted { .. } => "agent_started",
            AgentProgress { .. } => "agent_progress",
            AgentCompleted { .. } => "agent_completed",
            AgentFailed { .. } => "agent_failed",
            AgentRetrying { .. } => "agent_retrying",
            CheckpointCreated { .. } => "checkpoint_created",
            CheckpointLoaded { .. } => "checkpoint_loaded",
            CheckpointFailed { .. } => "checkpoint_failed",
            WorktreeCreated { .. } => "worktree_created",
            WorktreeMerged { .. } => "worktree_merged",
            WorktreeCleaned { .. } => "worktree_cleaned",
            QueueDepthChanged { .. } => "queue_depth_changed",
            MemoryPressure { .. } => "memory_pressure",
        }
    }
}
