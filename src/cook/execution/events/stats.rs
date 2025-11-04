//! Event statistics aggregation
//!
//! This module provides data structures and functions for aggregating event statistics:
//! - Event statistics data structure
//! - Time range calculations
//! - Success/failure counting
//!
//! Statistics are computed from event data for monitoring and reporting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub job_id: String,
    pub total_events: usize,
    pub event_counts: HashMap<String, usize>,
    pub success_count: usize,
    pub failure_count: usize,
    pub duration_ms: Option<i64>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}
