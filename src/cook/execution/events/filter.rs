//! Event filtering functionality
//!
//! This module provides filtering logic for querying events:
//! - Event filter criteria definition
//! - Pure filter matching functions
//!
//! All functions in this module are pure predicates for testability.

use super::EventRecord;
use chrono::{DateTime, Utc};

/// Event filter for querying events
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub job_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_types: Vec<String>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub correlation_id: Option<String>,
    pub limit: Option<usize>,
}

/// Apply filter to an event (pure predicate function)
///
/// # Arguments
/// * `event` - The event record to test
/// * `filter` - The filter criteria to apply
///
/// # Returns
/// `true` if the event matches all filter criteria, `false` otherwise
pub fn matches_filter(event: &EventRecord, filter: &EventFilter) -> bool {
    // Check job ID
    if let Some(ref job_id) = filter.job_id {
        if event.event.job_id() != job_id {
            return false;
        }
    }

    // Check agent ID
    if let Some(ref agent_id) = filter.agent_id {
        if event.event.agent_id() != Some(agent_id.as_str()) {
            return false;
        }
    }

    // Check event types
    if !filter.event_types.is_empty()
        && !filter
            .event_types
            .contains(&event.event.event_name().to_string())
    {
        return false;
    }

    // Check time range
    if let Some((start, end)) = filter.time_range {
        if event.timestamp < start || event.timestamp > end {
            return false;
        }
    }

    // Check correlation ID
    if let Some(ref correlation_id) = filter.correlation_id {
        if event.correlation_id != *correlation_id {
            return false;
        }
    }

    true
}
