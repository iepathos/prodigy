//! Output formatting functions for event display
//!
//! This module contains all functions for formatting and displaying events in various
//! formats (human-readable, JSON, YAML, table, CSV, markdown).

use anyhow::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use serde_json::Value;

use super::transform;

/// Information about available jobs in the global storage
#[derive(Debug)]
pub struct JobInfo {
    pub id: String,
    pub status: JobStatus,
    pub start_time: Option<DateTime<Local>>,
    pub end_time: Option<DateTime<Local>>,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug)]
pub enum JobStatus {
    InProgress,
    Completed,
    Failed,
    Unknown,
}

/// Pure function to format statistics as human-readable string
pub fn format_statistics_human(
    sorted_stats: &[(String, usize)],
    total: usize,
    group_by: &str,
) -> String {
    let mut output = format!("Event Statistics (grouped by {})\n", group_by);
    output.push_str(&"=".repeat(50));
    output.push('\n');

    for (key, count) in sorted_stats {
        let percentage = (*count as f64 / total as f64) * 100.0;
        output.push_str(&format!(
            "{:<30} {:>6} ({:>5.1}%)\n",
            key, count, percentage
        ));
    }

    output.push_str(&"=".repeat(50));
    output.push('\n');
    output.push_str(&format!("Total events: {}\n", total));
    output
}

/// Pure function to format job info for display
pub fn create_job_display_info(job: &JobInfo) -> String {
    match job.status {
        JobStatus::Completed => {
            let duration = calculate_duration(job.start_time, job.end_time);
            format!(
                "{} [✓ COMPLETED{} - Success: {}, Failed: {}]",
                job.id, duration, job.success_count, job.failure_count
            )
        }
        JobStatus::Failed => {
            format!("{} [✗ FAILED]", job.id)
        }
        JobStatus::InProgress => {
            let elapsed = calculate_elapsed(job.start_time);
            format!("{} [⟳ IN PROGRESS{}]", job.id, elapsed)
        }
        JobStatus::Unknown => {
            format!("{} [? UNKNOWN]", job.id)
        }
    }
}

/// Pure function to format statistics as JSON
pub fn format_statistics_json(
    sorted_stats: &[(String, usize)],
    total: usize,
    group_by: &str,
) -> Result<String> {
    #[derive(Serialize)]
    struct StatsOutput {
        group_by: String,
        stats: Vec<StatEntry>,
        total: usize,
    }

    #[derive(Serialize)]
    struct StatEntry {
        key: String,
        count: usize,
        percentage: f64,
    }

    let entries: Vec<StatEntry> = sorted_stats
        .iter()
        .map(|(key, count)| StatEntry {
            key: key.clone(),
            count: *count,
            percentage: (*count as f64 / total as f64) * 100.0,
        })
        .collect();

    let output = StatsOutput {
        group_by: group_by.to_string(),
        stats: entries,
        total,
    };

    Ok(serde_json::to_string_pretty(&output)?)
}

/// Pure function to format statistics as YAML
pub fn format_statistics_yaml(
    sorted_stats: &[(String, usize)],
    total: usize,
    group_by: &str,
) -> Result<String> {
    #[derive(Serialize)]
    struct StatsOutput {
        group_by: String,
        stats: Vec<StatEntry>,
        total: usize,
    }

    #[derive(Serialize)]
    struct StatEntry {
        key: String,
        count: usize,
        percentage: f64,
    }

    let entries: Vec<StatEntry> = sorted_stats
        .iter()
        .map(|(key, count)| StatEntry {
            key: key.clone(),
            count: *count,
            percentage: (*count as f64 / total as f64) * 100.0,
        })
        .collect();

    let output = StatsOutput {
        group_by: group_by.to_string(),
        stats: entries,
        total,
    };

    Ok(serde_yaml::to_string(&output)?)
}

/// Pure function to format cleanup summary message
pub fn create_cleanup_summary_message(total_cleaned: usize, dry_run: bool) -> String {
    if total_cleaned == 0 {
        "No events matched the cleanup criteria.".to_string()
    } else if dry_run {
        format!("Would clean {} events", total_cleaned)
    } else {
        format!("Cleaned {} events", total_cleaned)
    }
}

/// Pure function to create cleanup summary JSON
pub fn create_cleanup_summary_json(
    total_cleaned: usize,
    total_archived: usize,
    dry_run: bool,
) -> Result<String> {
    #[derive(Serialize)]
    struct CleanSummary {
        dry_run: bool,
        events_cleaned: usize,
        events_archived: usize,
        message: String,
    }

    let summary = CleanSummary {
        dry_run,
        events_cleaned: total_cleaned,
        events_archived: total_archived,
        message: create_cleanup_summary_message(total_cleaned, dry_run),
    };

    Ok(serde_json::to_string_pretty(&summary)?)
}

/// Pure function to create human-readable cleanup summary
pub fn create_cleanup_summary_human(
    total_cleaned: usize,
    total_archived: usize,
    dry_run: bool,
) -> String {
    let mut summary = String::new();

    if dry_run {
        summary.push_str(&format!(
            "Summary (dry run): {} events would be cleaned\n",
            total_cleaned
        ));
        if total_archived > 0 {
            summary.push_str(&format!("  {} events would be archived\n", total_archived));
        }
    } else {
        summary.push_str(&format!("Summary: {} events cleaned\n", total_cleaned));
        if total_archived > 0 {
            summary.push_str(&format!("  {} events archived\n", total_archived));
        }
    }

    if total_cleaned == 0 {
        summary.push_str("No events matched the cleanup criteria.\n");
    }

    summary
}

/// Calculate duration between start and end times
pub fn calculate_duration(start: Option<DateTime<Local>>, end: Option<DateTime<Local>>) -> String {
    match (start, end) {
        (Some(start), Some(end)) => {
            let diff = end.signed_duration_since(start);
            format!(" in {}m{}s", diff.num_minutes(), diff.num_seconds() % 60)
        }
        _ => String::new(),
    }
}

/// Calculate elapsed time from start
pub fn calculate_elapsed(start: Option<DateTime<Local>>) -> String {
    match start {
        Some(start) => {
            let diff = Local::now().signed_duration_since(start);
            format!(" - running for {}m", diff.num_minutes())
        }
        None => String::new(),
    }
}

/// Pure function to display statistics in the specified format
pub fn display_statistics_with_format(
    sorted_stats: &[(String, usize)],
    total: usize,
    group_by: &str,
    output_format: &str,
    is_aggregated: bool,
) -> Result<()> {
    match output_format {
        "json" => {
            let json_output = format_statistics_json(sorted_stats, total, group_by)?;
            println!("{}", json_output);
        }
        "yaml" => {
            let yaml_output = format_statistics_yaml(sorted_stats, total, group_by)?;
            println!("{}", yaml_output);
        }
        _ => {
            let title_suffix = if is_aggregated { " - All Jobs" } else { "" };
            println!("Event Statistics (grouped by {}){}", group_by, title_suffix);
            let human_output = format_statistics_human(sorted_stats, total, group_by);
            print!("{}", human_output);
        }
    }

    Ok(())
}

/// Pure function to display events in the specified format
pub fn display_events_with_format(events: &[Value], output_format: &str) -> Result<()> {
    match output_format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(events)?);
        }
        "yaml" => {
            println!("{}", serde_yaml::to_string(events)?);
        }
        "table" => {
            display_events_as_table(events)?;
        }
        _ => {
            // Default to human-readable output
            for event in events {
                display_event(event);
            }
            println!("\nDisplayed {} events", events.len());
        }
    }
    Ok(())
}

/// Pure function to display search results
pub fn display_search_results(matching_events: &[Value], is_aggregated: bool) -> Result<()> {
    for event in matching_events {
        display_event(event);
    }

    let suffix = if is_aggregated {
        " across all jobs"
    } else {
        ""
    };
    println!(
        "\nFound {} matching events{}",
        matching_events.len(),
        suffix
    );
    Ok(())
}

/// Display JobStarted event
pub fn display_job_started(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
    if let Some(data) = event.get("JobStarted") {
        let total_items = data
            .get("total_items")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        println!(
            "[{}] {} - Job: {} - Started with {} items",
            time_str, event_type, job_id, total_items
        );
    }
}

/// Display JobCompleted event
pub fn display_job_completed(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
    if let Some(data) = event.get("JobCompleted") {
        let success = data
            .get("success_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let failure = data
            .get("failure_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        println!(
            "[{}] {} - Job: {} - Success: {}, Failures: {}",
            time_str, event_type, job_id, success, failure
        );
    }
}

/// Display AgentProgress event
pub fn display_agent_progress(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
    if let Some(data) = event.get("AgentProgress") {
        let agent_id = data
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");
        let step = data.get("step").and_then(|v| v.as_str()).unwrap_or("n/a");
        let progress = data
            .get("progress_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        println!(
            "[{}] {} - Job: {} - Agent: {} - Step: {} ({:.1}%)",
            time_str, event_type, job_id, agent_id, step, progress
        );
    }
}

/// Display generic event
pub fn display_generic_event(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
    println!(
        "[{}] {} - Job: {} - {}",
        time_str,
        event_type,
        job_id,
        serde_json::to_string(event).unwrap_or_default()
    );
}

pub fn display_event(event: &Value) {
    let (event_type, time_str, job_id) = transform::extract_event_metadata(event);

    match event_type.as_str() {
        "JobStarted" => display_job_started(event, &time_str, &event_type, &job_id),
        "JobCompleted" => display_job_completed(event, &time_str, &event_type, &job_id),
        "AgentProgress" => display_agent_progress(event, &time_str, &event_type, &job_id),
        _ => display_generic_event(event, &time_str, &event_type, &job_id),
    }
}

pub fn export_as_json(events: &[Value]) -> Result<String> {
    Ok(serde_json::to_string_pretty(&events)?)
}

pub fn export_as_csv(events: &[Value]) -> Result<String> {
    use std::fmt::Write;

    let mut csv = String::new();

    // Write header
    writeln!(&mut csv, "timestamp,event_type,job_id,agent_id,details")?;

    // Write rows
    for event in events {
        let timestamp = transform::extract_timestamp(event)
            .map(|ts| ts.to_rfc3339())
            .unwrap_or_default();
        let event_type = transform::get_event_type(event);
        let job_id = transform::extract_job_id(event);
        let agent_id = transform::extract_agent_id(event);
        let details = serde_json::to_string(event)?
            .replace('"', "\"\"")
            .replace('\n', " ");

        writeln!(
            &mut csv,
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            timestamp, event_type, job_id, agent_id, details
        )?;
    }

    Ok(csv)
}

pub fn export_as_markdown(events: &[Value]) -> Result<String> {
    use std::fmt::Write;

    let mut md = String::new();

    writeln!(&mut md, "# MapReduce Events\n")?;
    writeln!(
        &mut md,
        "| Timestamp | Event Type | Job ID | Agent ID | Details |"
    )?;
    writeln!(
        &mut md,
        "|-----------|------------|--------|----------|---------|"
    )?;

    for event in events {
        let timestamp = transform::format_timestamp(transform::extract_timestamp(event));
        let event_type = transform::get_event_type(event);
        let job_id = transform::extract_job_id(event);
        let agent_id = transform::extract_agent_id(event);

        let details = format_event_details(event);

        writeln!(
            &mut md,
            "| {} | {} | {} | {} | {} |",
            timestamp, event_type, job_id, agent_id, details
        )?;
    }

    Ok(md)
}

/// Print table header for events display
pub fn print_table_header() {
    println!(
        "{:<20} {:<15} {:<20} {:<15} {:<30}",
        "Timestamp", "Event Type", "Job ID", "Agent ID", "Details"
    );
    println!("{}", "-".repeat(100));
}

/// Truncate string to fit in table column
pub fn truncate_field(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Extract table row data from event
pub fn extract_table_row_data(event: &Value) -> (String, String, String, String, String) {
    let timestamp = transform::format_timestamp(transform::extract_timestamp(event));
    let event_type = transform::get_event_type(event);
    let job_id = transform::extract_job_id(event);
    let agent_id = transform::extract_agent_id(event);
    let details = format_event_details(event);

    (timestamp, event_type, job_id, agent_id, details)
}

/// Print a single event as a table row
pub fn print_event_row(event: &Value) {
    let (timestamp, event_type, job_id, agent_id, details) = extract_table_row_data(event);

    println!(
        "{:<20} {:<15} {:<20} {:<15} {:<30}",
        truncate_field(&timestamp, 19),
        truncate_field(&event_type, 14),
        truncate_field(&job_id, 19),
        truncate_field(&agent_id, 14),
        truncate_field(&details, 29)
    );
}

/// Display events in a table format
pub fn display_events_as_table(events: &[Value]) -> Result<()> {
    if events.is_empty() {
        println!("No events to display.");
        return Ok(());
    }

    print_table_header();

    for event in events {
        print_event_row(event);
    }

    println!("\nTotal events: {}", events.len());
    Ok(())
}

pub fn format_event_details(event: &Value) -> String {
    let event_type = transform::get_event_type(event);

    match event_type.as_str() {
        "JobStarted" => {
            if let Some(data) = event.get("JobStarted") {
                let total = data
                    .get("total_items")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                format!("{} items", total)
            } else {
                "".to_string()
            }
        }
        "JobCompleted" => {
            if let Some(data) = event.get("JobCompleted") {
                let success = data
                    .get("success_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let failure = data
                    .get("failure_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                format!("✓ {} / ✗ {}", success, failure)
            } else {
                "".to_string()
            }
        }
        "AgentProgress" => {
            if let Some(data) = event.get("AgentProgress") {
                let step = data.get("step").and_then(|v| v.as_str()).unwrap_or("n/a");
                let progress = data
                    .get("progress_pct")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                format!("{} ({:.1}%)", step, progress)
            } else {
                "".to_string()
            }
        }
        _ => "".to_string(),
    }
}

/// Display cleanup summary (refactored to use pure functions)
pub fn display_cleanup_summary(
    total_cleaned: usize,
    total_archived: usize,
    dry_run: bool,
    output_format: &str,
) -> Result<()> {
    if output_format == "json" {
        let json_summary = create_cleanup_summary_json(total_cleaned, total_archived, dry_run)?;
        println!("{}", json_summary);
    } else {
        let human_summary = create_cleanup_summary_human(total_cleaned, total_archived, dry_run);
        print!("\n{}", human_summary);
    }
    Ok(())
}
