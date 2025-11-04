//! Pure index calculation and validation functions for event storage
//!
//! This module provides pure functions for working with event indices, including:
//! - Index building from event data
//! - Time range calculations
//! - Event count aggregation
//! - Index validation
//!
//! All functions in this module are pure (no I/O, no side effects) for testability.

use super::EventRecord;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;
use uuid::Uuid;

/// Event index for quick lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventIndex {
    pub job_id: String,
    pub event_counts: HashMap<String, usize>,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub file_offsets: Vec<FileOffset>,
    pub total_events: usize,
}

/// File offset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOffset {
    pub file_path: PathBuf,
    pub byte_offset: u64,
    pub line_number: usize,
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

/// Type alias for parsed event data from a file
pub type ParsedEventData = Vec<(EventRecord, u64, usize)>;

/// Type alias for file events data
pub type FileEventsData = Vec<(PathBuf, ParsedEventData)>;

/// Calculate the time range for a collection of events (pure function)
pub fn calculate_time_range(events: &[EventRecord]) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    if events.is_empty() {
        return None;
    }

    let mut start = events[0].timestamp;
    let mut end = events[0].timestamp;

    for event in events {
        if event.timestamp < start {
            start = event.timestamp;
        }
        if event.timestamp > end {
            end = event.timestamp;
        }
    }

    Some((start, end))
}

/// Build an index from file event data (pure function)
///
/// Takes parsed event data from files and constructs an EventIndex.
/// This is a pure function that performs no I/O operations.
///
/// # Arguments
/// * `job_id` - The job identifier
/// * `file_events` - Vector of (file_path, events) tuples
///
/// # Returns
/// An EventIndex with aggregated statistics
pub fn build_index_from_events(job_id: &str, file_events: FileEventsData) -> EventIndex {
    let mut index = EventIndex {
        job_id: job_id.to_string(),
        event_counts: HashMap::new(),
        time_range: (Utc::now(), Utc::now()),
        file_offsets: Vec::new(),
        total_events: 0,
    };

    let mut all_timestamps = Vec::new();

    for (file_path, events) in file_events {
        for (event, byte_offset, line_number) in events {
            index.total_events += 1;

            // Update event counts
            let event_name = event.event.event_name().to_string();
            increment_event_count(&mut index.event_counts, event_name);

            // Create file offset
            let file_offset = FileOffset {
                file_path: file_path.clone(),
                byte_offset,
                line_number,
                event_id: event.id,
                timestamp: event.timestamp,
            };
            index.file_offsets.push(file_offset);

            all_timestamps.push(event.timestamp);
        }
    }

    // Calculate time range from all timestamps
    if !all_timestamps.is_empty() {
        let min_time = all_timestamps
            .iter()
            .min()
            .copied()
            .unwrap_or_else(Utc::now);
        let max_time = all_timestamps
            .iter()
            .max()
            .copied()
            .unwrap_or_else(Utc::now);
        index.time_range = (min_time, max_time);
    }

    index
}

/// Update time range with a new event timestamp (pure function)
pub fn update_time_range(
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    event_time: DateTime<Utc>,
) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let new_start = match start {
        None => Some(event_time),
        Some(s) if event_time < s => Some(event_time),
        Some(s) => Some(s),
    };

    let new_end = match end {
        None => Some(event_time),
        Some(e) if event_time > e => Some(event_time),
        Some(e) => Some(e),
    };

    (new_start, new_end)
}

/// Increment event count for a given event type (pure function)
pub fn increment_event_count(counts: &mut HashMap<String, usize>, event_name: String) {
    *counts.entry(event_name).or_insert(0) += 1;
}

/// Validate job ID for proper format and content (pure function)
pub fn validate_job_id(job_id: &str) -> Result<()> {
    // Check for empty job_id
    if job_id.is_empty() {
        return Err(anyhow!("Job ID cannot be empty"));
    }

    // Check for invalid characters (only allow alphanumeric, dash, underscore)
    if !job_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!(
            "Job ID contains invalid characters. Only alphanumeric, dash, and underscore allowed: {}",
            job_id
        ));
    }

    // Check for reasonable length (max 255 characters)
    if job_id.len() > 255 {
        return Err(anyhow!(
            "Job ID is too long (max 255 characters): {} characters",
            job_id.len()
        ));
    }

    Ok(())
}

/// Validate index consistency before saving (pure function with side effect of fixing issues)
pub fn validate_index_consistency(index: &mut EventIndex) -> Result<()> {
    // Validate time range consistency
    let (start, end) = index.time_range;
    if start > end {
        // Fix the time range if it's inverted
        warn!(
            "Index time range was inverted (start: {}, end: {}). Correcting...",
            start, end
        );
        index.time_range = (end, start);
    }

    // Validate event counts match total
    let sum_of_counts: usize = index.event_counts.values().sum();
    if sum_of_counts != index.total_events {
        warn!(
            "Event count mismatch: sum of individual counts ({}) != total_events ({}). Using sum.",
            sum_of_counts, index.total_events
        );
        index.total_events = sum_of_counts;
    }

    // Validate file offsets count matches total events
    if index.file_offsets.len() != index.total_events {
        warn!(
            "File offset count ({}) doesn't match total events ({}). This may indicate partial indexing.",
            index.file_offsets.len(),
            index.total_events
        );
        // This is acceptable as we might have events without offsets
    }

    // Ensure job_id is not empty (redundant check but important)
    if index.job_id.is_empty() {
        return Err(anyhow!("Index has empty job_id"));
    }

    Ok(())
}
