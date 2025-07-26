use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub initial_score: f32,
    pub final_score: f32,
    pub iterations: usize,
    pub files_changed: usize,
}
