use serde::{Deserialize, Serialize};

/// Summary of an improvement session
///
/// Tracks the progress and results of a single `mmm improve` run,
/// including score changes and the number of iterations performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub initial_score: f32,
    pub final_score: f32,
    pub iterations: usize,
    pub files_changed: usize,
}
