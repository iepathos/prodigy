use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub mod engine;
pub mod validator;
pub mod validators;

pub use engine::GoalSeekEngine;
pub use validator::{ValidationResult, Validator};
pub use validators::{SpecCoverageValidator, TestPassValidator, OutputQualityValidator};

/// Simplified goal-seek configuration with single command
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalSeekConfig {
    /// Human-readable goal description
    pub goal: String,
    
    /// Single command that handles both initial attempt and refinement
    /// Command gets validation context automatically via ${validation.gaps}
    pub command: String,
    
    /// Command to validate the attempt (returns score 0-100)
    pub validate: String,
    
    /// Success threshold (0-100)
    pub threshold: u32,
    
    /// Maximum attempts before giving up
    pub max_attempts: u32,
    
    /// Optional timeout for entire operation
    pub timeout_seconds: Option<u64>,
    
    /// Whether to fail workflow on incomplete
    pub fail_on_incomplete: Option<bool>,
}

/// Result of goal-seeking operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalSeekResult {
    /// Goal achieved within threshold
    Success {
        attempts: u32,
        final_score: u32,
        execution_time: Duration,
    },
    
    /// Max attempts reached without success
    MaxAttemptsReached {
        attempts: u32,
        best_score: u32,
        last_output: String,
    },
    
    /// Operation timed out
    Timeout {
        attempts: u32,
        best_score: u32,
        elapsed: Duration,
    },
    
    /// Converged (no improvement)
    Converged {
        attempts: u32,
        final_score: u32,
        reason: String,
    },
    
    /// Failed due to error
    Failed {
        attempts: u32,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct AttemptRecord {
    pub attempt: u32,
    pub score: u32,
    pub output: String,
    pub timestamp: Instant,
}