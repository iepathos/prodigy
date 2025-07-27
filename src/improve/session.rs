use serde::{Deserialize, Serialize};

/// Summary of an improvement session
///
/// Tracks the progress and results of a single `mmm improve` run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub iterations: usize,
    pub files_changed: usize,
}
