use chrono::{DateTime, Utc};
use std::path::PathBuf;

pub mod manager;
#[cfg(test)]
mod tests;

pub use manager::WorktreeManager;

#[derive(Debug, Clone)]
pub struct WorktreeSession {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub focus: Option<String>,
}

impl WorktreeSession {
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

