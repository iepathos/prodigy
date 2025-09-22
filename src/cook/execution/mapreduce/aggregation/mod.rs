//! Result aggregation and reduction module
//!
//! This module provides functionality for aggregating and reducing results
//! from MapReduce agent execution, following functional programming principles.

pub mod collector;
pub mod formatter;
pub mod reducer;

// Re-export main types for convenience
pub use collector::{CollectionStrategy, ResultCollector};
pub use formatter::{FormatType, OutputFormatter};
pub use reducer::{ReductionStrategy, ResultReducer};

use crate::cook::execution::mapreduce::{AgentResult, AgentStatus};
use serde::{Deserialize, Serialize};

/// Summary statistics for aggregated results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationSummary {
    /// Number of successful operations
    pub successful: usize,
    /// Number of failed operations
    pub failed: usize,
    /// Total number of operations
    pub total: usize,
    /// Average duration in seconds
    pub avg_duration_secs: f64,
    /// Total duration in seconds
    pub total_duration_secs: f64,
}

impl AggregationSummary {
    /// Create a summary from agent results
    pub fn from_results(results: &[AgentResult]) -> Self {
        let successful = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();

        let failed = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_) | AgentStatus::Timeout))
            .count();

        let total_duration: f64 = results.iter().map(|r| r.duration.as_secs_f64()).sum();

        let avg_duration = if results.is_empty() {
            0.0
        } else {
            total_duration / results.len() as f64
        };

        Self {
            successful,
            failed,
            total: results.len(),
            avg_duration_secs: avg_duration,
            total_duration_secs: total_duration,
        }
    }
}
