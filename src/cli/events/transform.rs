//! Pure transformation functions for event data processing
//!
//! This module contains all pure functions for transforming, filtering, and
//! analyzing event data. These functions have no side effects and are highly testable.

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

/// Pure function to build event filter criteria
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub job_id: Option<String>,
    pub event_type: Option<String>,
    pub agent_id: Option<String>,
    pub since_time: Option<DateTime<Utc>>,
}

impl EventFilter {
    pub fn new(
        job_id: Option<String>,
        event_type: Option<String>,
        agent_id: Option<String>,
        since: Option<u64>,
    ) -> Self {
        let since_time =
            since.map(|minutes| Utc::now() - chrono::Duration::minutes(minutes as i64));
        Self {
            job_id,
            event_type,
            agent_id,
            since_time,
        }
    }

    pub fn matches_event(&self, event: &Value) -> bool {
        if let Some(ref jid) = self.job_id {
            if !event_matches_field(event, "job_id", jid) {
                return false;
            }
        }

        if let Some(ref etype) = self.event_type {
            if !event_matches_type(event, etype) {
                return false;
            }
        }

        if let Some(ref aid) = self.agent_id {
            if !event_matches_field(event, "agent_id", aid) {
                return false;
            }
        }

        if let Some(since_time) = self.since_time {
            if !event_is_recent(event, since_time) {
                return false;
            }
        }

        true
    }
}

/// Pure function to transform events into statistics
pub fn calculate_event_statistics(
    events: impl Iterator<Item = Value>,
    group_by: &str,
) -> (HashMap<String, usize>, usize) {
    let mut stats = HashMap::new();
    let mut total = 0;

    for event in events {
        total += 1;

        let key = match group_by {
            "event_type" => get_event_type(&event),
            "job_id" => event
                .get("job_id")
                .or_else(|| extract_nested_field(&event, "job_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            "agent_id" => event
                .get("agent_id")
                .or_else(|| extract_nested_field(&event, "agent_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("n/a")
                .to_string(),
            _ => "unknown".to_string(),
        };

        *stats.entry(key).or_insert(0) += 1;
    }

    (stats, total)
}

/// Pure function to sort statistics by count
pub fn sort_statistics_by_count(stats: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut sorted_stats: Vec<_> = stats.into_iter().collect();
    sorted_stats.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_stats
}

/// Pure function to check if event matches search pattern
pub fn event_matches_search(
    event: &Value,
    pattern: &regex::Regex,
    fields: Option<&[String]>,
) -> bool {
    if let Some(fields) = fields {
        fields.iter().any(|field| {
            event
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| pattern.is_match(s))
                .unwrap_or(false)
        })
    } else {
        search_in_value(event, pattern)
    }
}

/// Pure function to process a single event line into a JSON Value
pub fn parse_event_line(line: &str) -> Option<Value> {
    if line.trim().is_empty() {
        return None;
    }
    serde_json::from_str(line).ok()
}

/// Pure function to convert duration string to days
pub fn convert_duration_to_days(duration_str: &str) -> anyhow::Result<u32> {
    let duration_str = duration_str.trim().to_lowercase();

    if duration_str.is_empty() {
        return Err(anyhow::anyhow!("Empty duration string"));
    }

    let (number_part, unit_part) =
        if let Some(unit_pos) = duration_str.chars().position(|c| c.is_alphabetic()) {
            let (num, unit) = duration_str.split_at(unit_pos);
            (num, unit)
        } else {
            // If no unit specified, assume days
            (duration_str.as_str(), "d")
        };

    let number: f64 = number_part
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid number in duration: '{}'", number_part))?;

    let days = match unit_part {
        "d" | "day" | "days" => number,
        "w" | "week" | "weeks" => number * 7.0,
        "h" | "hour" | "hours" => number / 24.0,
        "m" | "min" | "minute" | "minutes" => number / (24.0 * 60.0),
        "s" | "sec" | "second" | "seconds" => number / (24.0 * 60.0 * 60.0),
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid duration unit: '{}'. Use d/day, w/week, h/hour, m/min, s/sec",
                unit_part
            ))
        }
    };

    // Convert to u32, ensuring at least 0 days
    Ok(days.max(0.0).ceil() as u32)
}

/// Pure function to convert size string to bytes
pub fn convert_size_to_bytes(size_str: &str) -> anyhow::Result<u64> {
    let size_str = size_str.trim().to_uppercase();

    if size_str.is_empty() {
        return Err(anyhow::anyhow!("Empty size string"));
    }

    let (number_part, unit_part) =
        if let Some(unit_pos) = size_str.chars().position(|c| c.is_alphabetic()) {
            let (num, unit) = size_str.split_at(unit_pos);
            (num, unit)
        } else {
            // If no unit specified, assume bytes
            (size_str.as_str(), "B")
        };

    let number: f64 = number_part
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid number in size: '{}'", number_part))?;

    let bytes = match unit_part {
        "B" | "BYTE" | "BYTES" => number,
        "KB" | "K" => number * 1024.0,
        "MB" | "M" => number * 1024.0 * 1024.0,
        "GB" | "G" => number * 1024.0 * 1024.0 * 1024.0,
        "TB" | "T" => number * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid size unit: '{}'. Use B/byte, KB/K, MB/M, GB/G, TB/T",
                unit_part
            ))
        }
    };

    // Convert to u64, ensuring at least 0 bytes
    Ok(bytes.max(0.0) as u64)
}

/// Pure function to validate retention policy parameters
pub fn validate_retention_policy(
    older_than: &Option<String>,
    max_events: &Option<usize>,
    max_size: &Option<String>,
) -> anyhow::Result<()> {
    if let Some(ref duration) = older_than {
        convert_duration_to_days(duration)?;
    }

    if let Some(ref size) = max_size {
        convert_size_to_bytes(size)?;
    }

    if older_than.is_none() && max_events.is_none() && max_size.is_none() {
        return Err(anyhow::anyhow!(
            "At least one retention criteria must be specified (older_than, max_events, or max_size)"
        ));
    }

    Ok(())
}

/// Pure function to search events with a pattern
pub fn search_events_with_pattern(
    events: &[Value],
    pattern: &str,
    fields: Option<&[String]>,
) -> anyhow::Result<Vec<Value>> {
    use regex::Regex;
    let re = Regex::new(pattern)?;

    let matching_events = events
        .iter()
        .filter(|event| event_matches_search(event, &re, fields))
        .cloned()
        .collect();

    Ok(matching_events)
}

/// Pure function: Calculate archived count based on policy
pub fn calculate_archived_count(events_count: usize, archive_enabled: bool) -> usize {
    if archive_enabled {
        events_count
    } else {
        0
    }
}

/// Pure function: Aggregate statistics
pub fn aggregate_stats(
    (cleaned, archived): (usize, usize),
    new_cleaned: usize,
    new_archived: usize,
) -> (usize, usize) {
    (cleaned + new_cleaned, archived + new_archived)
}

/// Pure function: Extract job name from directory path
pub fn extract_job_name(job_dir: &std::path::Path) -> String {
    job_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// =============================================================================
// Helper Functions
// =============================================================================

pub fn event_matches_field(event: &Value, field: &str, value: &str) -> bool {
    // First try direct field access
    if let Some(v) = event.get(field) {
        if let Some(s) = v.as_str() {
            return s == value;
        }
    }

    // Then try nested field access
    if let Some(v) = extract_nested_field(event, field) {
        if let Some(s) = v.as_str() {
            return s == value;
        }
    }

    false
}

pub fn event_matches_type(event: &Value, event_type: &str) -> bool {
    get_event_type(event) == event_type
}

pub fn get_event_type(event: &Value) -> String {
    // Extract event type from the event structure using functional pattern
    const EVENT_TYPES: &[&str] = &[
        "JobStarted",
        "JobCompleted",
        "JobFailed",
        "JobPaused",
        "JobResumed",
        "AgentStarted",
        "AgentProgress",
        "AgentCompleted",
        "AgentFailed",
        "PipelineStarted",
        "PipelineStageCompleted",
        "PipelineCompleted",
        "MetricsSnapshot",
    ];

    EVENT_TYPES
        .iter()
        .find(|&&event_type| event.get(event_type).is_some())
        .map(|&s| s.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub fn event_is_recent(event: &Value, since_time: DateTime<Utc>) -> bool {
    // Look for timestamp in various possible locations
    let timestamp_str = event
        .get("timestamp")
        .or_else(|| event.get("JobStarted").and_then(|v| v.get("timestamp")))
        .or_else(|| event.get("time"))
        .and_then(|v| v.as_str());

    if let Some(ts) = timestamp_str {
        if let Ok(event_time) = DateTime::parse_from_rfc3339(ts) {
            return event_time.with_timezone(&Utc) >= since_time;
        }
    }

    false
}

/// Extract event metadata for display
pub fn extract_event_metadata(event: &Value) -> (String, String, String) {
    let event_type = get_event_type(event);
    let timestamp = extract_timestamp(event);
    let job_id = extract_job_id(event);

    let time_str = format_timestamp(timestamp);
    (event_type, time_str, job_id)
}

/// Extract job ID from event
pub fn extract_job_id(event: &Value) -> String {
    event
        .get("job_id")
        .or_else(|| extract_nested_field(event, "job_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("n/a")
        .to_string()
}

/// Extract agent ID from event
pub fn extract_agent_id(event: &Value) -> String {
    event
        .get("agent_id")
        .or_else(|| extract_nested_field(event, "agent_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("n/a")
        .to_string()
}

/// Format timestamp for display
pub fn format_timestamp(timestamp: Option<DateTime<Utc>>) -> String {
    timestamp
        .map(|ts| {
            ts.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "n/a".to_string())
}

pub fn extract_timestamp(event: &Value) -> Option<DateTime<Utc>> {
    let timestamp_str = event
        .get("timestamp")
        .or_else(|| extract_nested_field(event, "timestamp"))
        .or_else(|| event.get("time"))
        .and_then(|v| v.as_str());

    timestamp_str
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn extract_nested_field<'a>(event: &'a Value, field: &str) -> Option<&'a Value> {
    // Look for field in nested event structures
    for key in [
        "JobStarted",
        "JobCompleted",
        "JobFailed",
        "AgentStarted",
        "AgentProgress",
        "AgentCompleted",
        "AgentFailed",
    ] {
        if let Some(nested) = event.get(key) {
            if let Some(value) = nested.get(field) {
                return Some(value);
            }
        }
    }
    None
}

pub fn search_in_value(value: &Value, re: &regex::Regex) -> bool {
    match value {
        Value::String(s) => re.is_match(s),
        Value::Object(map) => map.values().any(|v| search_in_value(v, re)),
        Value::Array(arr) => arr.iter().any(|v| search_in_value(v, re)),
        _ => false,
    }
}
