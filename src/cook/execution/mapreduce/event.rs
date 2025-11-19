//! Event tracking for MapReduce execution

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::agent::types::CleanupStatus;

/// Failure reasons for agent execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureReason {
    /// Agent execution timed out
    Timeout,
    /// Command failed with exit code
    CommandFailed { exit_code: i32 },
    /// Commit validation failed - required commit was not created
    CommitValidationFailed { command: String },
    /// Merge conflict or merge failure
    MergeConflict,
    /// Worktree creation or management error
    WorktreeError,
    /// Unknown or unclassified failure
    Unknown,
}

/// Event logger for MapReduce job tracking
pub struct EventLogger {
    _project_root: PathBuf,
    _job_id: String,
    _session_id: Option<String>,
    verbosity: u8,
}

impl EventLogger {
    /// Create a new event logger
    pub fn new(
        project_root: PathBuf,
        job_id: String,
        session_id: Option<String>,
        verbosity: u8,
    ) -> Self {
        Self {
            _project_root: project_root,
            _job_id: job_id,
            _session_id: session_id,
            verbosity,
        }
    }

    /// Log an event with verbosity control
    pub async fn log_event(&self, event: MapReduceEvent) -> Result<()> {
        // Always log errors regardless of verbosity
        if matches!(event, MapReduceEvent::AgentFailed { .. }) {
            tracing::info!("MapReduce event: {:?}", event);
            return Ok(());
        }

        // Only log verbose events in verbose mode (verbosity >= 1)
        if self.verbosity >= 1 {
            match &event {
                MapReduceEvent::AgentCompleted {
                    agent_id,
                    item_id,
                    duration,
                    cleanup_status,
                    commits,
                    json_log_location,
                    ..
                } => {
                    // Truncate commit list if it's too long (unless verbosity >= 2)
                    let commits_display = if commits.len() > 10 && self.verbosity < 2 {
                        format!(
                            "[{:?}, {:?}, ... ({} more)]",
                            commits.first().unwrap_or(&String::new()),
                            commits.get(1).unwrap_or(&String::new()),
                            commits.len() - 2
                        )
                    } else {
                        format!("{:?}", commits)
                    };

                    tracing::info!(
                        "MapReduce event: AgentCompleted {{ agent_id: {:?}, item_id: {:?}, duration: {:?}, cleanup_status: {:?}, commits: {}, json_log_location: {:?} }}",
                        agent_id,
                        item_id,
                        duration,
                        cleanup_status,
                        commits_display,
                        json_log_location
                    );
                }
                _ => {
                    tracing::info!("MapReduce event: {:?}", event);
                }
            }
        } else {
            // In default mode, only log phase-level events
            match event {
                MapReduceEvent::MapPhaseStarted { .. }
                | MapReduceEvent::MapPhaseCompleted { .. }
                | MapReduceEvent::ReducePhaseStarted { .. }
                | MapReduceEvent::ReducePhaseCompleted { .. } => {
                    tracing::info!("MapReduce event: {:?}", event);
                }
                // Suppress AgentStarted, AgentCompleted, and AgentFailed in default mode
                // (AgentFailed is already handled above and always shown)
                MapReduceEvent::AgentStarted { .. }
                | MapReduceEvent::AgentCompleted { .. }
                | MapReduceEvent::AgentFailed { .. } => {}
            }
        }

        Ok(())
    }
}

/// MapReduce execution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapReduceEvent {
    /// Map phase started
    MapPhaseStarted {
        total_items: usize,
        timestamp: DateTime<Utc>,
    },
    /// Map phase completed
    MapPhaseCompleted {
        successful: usize,
        failed: usize,
        timestamp: DateTime<Utc>,
    },
    /// Agent started processing an item
    AgentStarted {
        agent_id: String,
        item_id: String,
        timestamp: DateTime<Utc>,
    },
    /// Agent completed processing
    AgentCompleted {
        agent_id: String,
        item_id: String,
        duration: Duration,
        timestamp: DateTime<Utc>,
        cleanup_status: Option<CleanupStatus>,
        commits: Vec<String>,
        json_log_location: Option<String>,
    },
    /// Agent failed processing
    AgentFailed {
        agent_id: String,
        item_id: String,
        error: String,
        timestamp: DateTime<Utc>,
        failure_reason: FailureReason,
        json_log_location: Option<String>,
    },
    /// Reduce phase started
    ReducePhaseStarted { timestamp: DateTime<Utc> },
    /// Reduce phase completed
    ReducePhaseCompleted { timestamp: DateTime<Utc> },
}

impl MapReduceEvent {
    /// Create map phase started event
    pub fn map_phase_started(total_items: usize) -> Self {
        Self::MapPhaseStarted {
            total_items,
            timestamp: Utc::now(),
        }
    }

    /// Create map phase completed event
    pub fn map_phase_completed(successful: usize, failed: usize) -> Self {
        Self::MapPhaseCompleted {
            successful,
            failed,
            timestamp: Utc::now(),
        }
    }

    /// Create agent started event
    pub fn agent_started(agent_id: String, item_id: String) -> Self {
        Self::AgentStarted {
            agent_id,
            item_id,
            timestamp: Utc::now(),
        }
    }

    /// Create agent completed event
    pub fn agent_completed(
        agent_id: String,
        item_id: String,
        duration: Duration,
        cleanup_status: Option<CleanupStatus>,
        commits: Vec<String>,
        json_log_location: Option<String>,
    ) -> Self {
        Self::AgentCompleted {
            agent_id,
            item_id,
            duration,
            timestamp: Utc::now(),
            cleanup_status,
            commits,
            json_log_location,
        }
    }

    /// Create agent failed event
    pub fn agent_failed(
        agent_id: String,
        item_id: String,
        error: String,
        failure_reason: FailureReason,
        json_log_location: Option<String>,
    ) -> Self {
        Self::AgentFailed {
            agent_id,
            item_id,
            error,
            timestamp: Utc::now(),
            failure_reason,
            json_log_location,
        }
    }

    /// Create reduce phase started event
    pub fn reduce_phase_started() -> Self {
        Self::ReducePhaseStarted {
            timestamp: Utc::now(),
        }
    }

    /// Create reduce phase completed event
    pub fn reduce_phase_completed() -> Self {
        Self::ReducePhaseCompleted {
            timestamp: Utc::now(),
        }
    }
}
