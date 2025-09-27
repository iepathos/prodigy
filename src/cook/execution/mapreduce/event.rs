//! Event tracking for MapReduce execution

use std::path::PathBuf;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Event logger for MapReduce job tracking
pub struct EventLogger {
    project_root: PathBuf,
    job_id: String,
    session_id: Option<String>,
}

impl EventLogger {
    /// Create a new event logger
    pub fn new(project_root: PathBuf, job_id: String, session_id: Option<String>) -> Self {
        Self {
            project_root,
            job_id,
            session_id,
        }
    }

    /// Log an event
    pub async fn log_event(&self, event: MapReduceEvent) -> Result<()> {
        // For now, just log to tracing
        tracing::info!("MapReduce event: {:?}", event);
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
    },
    /// Agent failed processing
    AgentFailed {
        agent_id: String,
        item_id: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Reduce phase started
    ReducePhaseStarted {
        timestamp: DateTime<Utc>,
    },
    /// Reduce phase completed
    ReducePhaseCompleted {
        timestamp: DateTime<Utc>,
    },
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
    pub fn agent_completed(agent_id: String, item_id: String, duration: Duration) -> Self {
        Self::AgentCompleted {
            agent_id,
            item_id,
            duration,
            timestamp: Utc::now(),
        }
    }

    /// Create agent failed event
    pub fn agent_failed(agent_id: String, item_id: String, error: String) -> Self {
        Self::AgentFailed {
            agent_id,
            item_id,
            error,
            timestamp: Utc::now(),
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