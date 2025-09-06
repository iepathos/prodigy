//! Event logging and audit trail for MapReduce execution

mod event_logger;
mod event_store;
#[cfg(test)]
mod event_tests;
mod event_types;
mod event_writer;

pub use event_logger::{EventLogger, EventRecord};
pub use event_store::{EventFilter, EventIndex, EventStats, EventStore, FileOffset};
pub use event_types::MapReduceEvent;
pub use event_writer::{EventWriter, FileEventWriter, JsonlEventWriter};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Event severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Event category for filtering
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    JobLifecycle,
    AgentLifecycle,
    Checkpoint,
    Worktree,
    Performance,
}

impl MapReduceEvent {
    /// Get the severity level of this event
    pub fn severity(&self) -> EventSeverity {
        use MapReduceEvent::*;
        match self {
            JobFailed { .. } | AgentFailed { .. } | CheckpointFailed { .. } => EventSeverity::Error,
            MemoryPressure { .. } => EventSeverity::Warning,
            JobStarted { .. } | JobCompleted { .. } | AgentCompleted { .. } => EventSeverity::Info,
            _ => EventSeverity::Debug,
        }
    }

    /// Get the category of this event
    pub fn category(&self) -> EventCategory {
        use MapReduceEvent::*;
        match self {
            JobStarted { .. }
            | JobCompleted { .. }
            | JobFailed { .. }
            | JobPaused { .. }
            | JobResumed { .. } => EventCategory::JobLifecycle,
            AgentStarted { .. }
            | AgentProgress { .. }
            | AgentCompleted { .. }
            | AgentFailed { .. }
            | AgentRetrying { .. } => EventCategory::AgentLifecycle,
            CheckpointCreated { .. } | CheckpointLoaded { .. } | CheckpointFailed { .. } => {
                EventCategory::Checkpoint
            }
            WorktreeCreated { .. } | WorktreeMerged { .. } | WorktreeCleaned { .. } => {
                EventCategory::Worktree
            }
            QueueDepthChanged { .. } | MemoryPressure { .. } => EventCategory::Performance,
        }
    }
}
