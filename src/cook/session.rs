use serde::{Deserialize, Serialize};

/// Summary of a cooking session
///
/// Tracks the progress and results of a single `mmm cook` run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub iterations: usize,
    pub files_changed: usize,
}
