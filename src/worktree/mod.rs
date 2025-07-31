use chrono::{DateTime, Utc};
use std::path::PathBuf;

pub mod manager;
pub mod state;
#[cfg(test)]
mod test_state;
#[cfg(test)]
mod tests;

pub use manager::WorktreeManager;
pub use state::{
    Checkpoint, CommandType, InterruptionType, IterationInfo, WorktreeState, WorktreeStats,
    WorktreeStatus,
};

#[derive(Debug, Clone)]
pub struct WorktreeSession {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub focus: Option<String>,
}

impl WorktreeSession {
    #[must_use]
    pub fn new(name: String, branch: String, path: PathBuf, focus: Option<String>) -> Self {
        Self {
            name,
            branch,
            path,
            created_at: Utc::now(),
            focus,
        }
    }
}
