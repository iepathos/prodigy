//! CLI commands for viewing and searching MapReduce events

use crate::cook::interaction::prompts::{UserPrompter, UserPrompterImpl};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tracing::info;

/// Event viewer commands
#[derive(Debug, Args)]
pub struct EventsArgs {
    #[command(subcommand)]
    pub command: EventsCommand,
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
    /// List all events
    #[command(alias = "list")]
    Ls {
        /// Filter by job ID
        #[arg(long)]
        job_id: Option<String>,

        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,

        /// Filter by agent ID
        #[arg(long)]
        agent_id: Option<String>,

        /// Show only events from the last N minutes
        #[arg(long)]
        since: Option<u64>,

        /// Limit number of events shown
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Output format (human, json, yaml, table)
        #[arg(long, default_value = "human")]
        output_format: String,
    },

    /// Show event statistics
    Stats {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Group statistics by field (job_id, event_type, agent_id)
        #[arg(long, default_value = "event_type")]
        group_by: String,

        /// Output format (human, json, yaml, table)
        #[arg(long, default_value = "human")]
        output_format: String,
    },

    /// Search events by pattern
    Search {
        /// Search pattern (regex supported)
        pattern: String,

        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Search in specific fields only
        #[arg(long)]
        fields: Option<Vec<String>>,
    },

    /// Follow events in real-time (tail -f style)
    Follow {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Filter by job ID
        #[arg(long)]
        job_id: Option<String>,

        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,
    },

    /// Export events to different format
    Export {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Output format (json, csv, markdown)
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Clean up old events based on retention policy
    Clean {
        /// Clean events older than this duration (e.g., "7d", "30d", "1h", "2w")
        #[arg(long)]
        older_than: Option<String>,

        /// Maximum number of events to keep
        #[arg(long)]
        max_events: Option<usize>,

        /// Maximum file size to maintain (e.g., "100MB", "1GB")
        #[arg(long)]
        max_size: Option<String>,

        /// Show what would be cleaned without actually cleaning
        #[arg(long)]
        dry_run: bool,

        /// Archive events instead of deleting them
        #[arg(long)]
        archive: bool,

        /// Path to archive directory (defaults to .prodigy/events/archive)
        #[arg(long)]
        archive_path: Option<PathBuf>,

        /// Apply to all jobs (global storage)
        #[arg(long)]
        all_jobs: bool,

        /// Specific job ID to clean
        #[arg(long)]
        job_id: Option<String>,

        /// Specific event file to clean (for testing)
        #[arg(long)]
        file: Option<PathBuf>,

        /// Output format (human, json, yaml, table)
        #[arg(long, default_value = "human")]
        output_format: String,
    },
}

/// Information about available jobs in the global storage
#[derive(Debug)]
struct JobInfo {
    id: String,
    status: JobStatus,
    start_time: Option<DateTime<Local>>,
    end_time: Option<DateTime<Local>>,
    success_count: u64,
    failure_count: u64,
}

#[derive(Debug)]
enum JobStatus {
    InProgress,
    Completed,
    Failed,
    Unknown,
}

/// Get list of available jobs from global storage
fn get_available_jobs() -> Result<Vec<JobInfo>> {
    let current_dir = std::env::current_dir()?;
    let repo_name = crate::storage::extract_repo_name(&current_dir)?;
    let global_base = crate::storage::get_default_storage_dir()?;
    let global_events_dir = global_base.join("events").join(&repo_name);

    if !global_events_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&global_events_dir)?;

    let jobs: Vec<JobInfo> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|entry| {
            let job_id = entry.file_name().to_string_lossy().to_string();
            read_job_status(&global_events_dir.join(&job_id)).unwrap_or_else(|_| JobInfo {
                id: job_id.clone(),
                status: JobStatus::Unknown,
                start_time: None,
                end_time: None,
                success_count: 0,
                failure_count: 0,
            })
        })
        .collect();

    Ok(jobs)
}

/// Read job status from event files
fn read_job_status(job_events_dir: &Path) -> Result<JobInfo> {
    let job_id = job_events_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut status = JobStatus::Unknown;
    let mut start_time = None;
    let mut end_time = None;
    let mut success_count = 0;
    let mut failure_count = 0;

    // Find and read event files
    let event_files = find_event_files(job_events_dir)?;

    for file in event_files {
        let content = fs::read_to_string(&file)?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<Value>(line) {
                process_event_for_status(
                    &event,
                    &mut status,
                    &mut start_time,
                    &mut end_time,
                    &mut success_count,
                    &mut failure_count,
                );
            }
        }
    }

    Ok(JobInfo {
        id: job_id,
        status,
        start_time,
        end_time,
        success_count,
        failure_count,
    })
}

/// Process a single event to extract status information
fn process_event_for_status(
    event: &Value,
    status: &mut JobStatus,
    start_time: &mut Option<DateTime<Local>>,
    end_time: &mut Option<DateTime<Local>>,
    success_count: &mut u64,
    failure_count: &mut u64,
) {
    if event.get("JobStarted").is_some() {
        *status = JobStatus::InProgress;
        if let Some(ts) = extract_timestamp(event) {
            *start_time = Some(ts.with_timezone(&Local));
        }
    } else if let Some(completed) = event.get("JobCompleted") {
        *status = JobStatus::Completed;
        if let Some(ts) = extract_timestamp(event) {
            *end_time = Some(ts.with_timezone(&Local));
        }
        if let Some(s) = completed.get("success_count").and_then(|v| v.as_u64()) {
            *success_count = s;
        }
        if let Some(f) = completed.get("failure_count").and_then(|v| v.as_u64()) {
            *failure_count = f;
        }
    } else if event.get("JobFailed").is_some() {
        *status = JobStatus::Failed;
        if let Some(ts) = extract_timestamp(event) {
            *end_time = Some(ts.with_timezone(&Local));
        }
    }
}

/// Find all event files in a directory, sorted by name
fn find_event_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "jsonl")
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();

    files.sort();
    Ok(files)
}

/// Resolve event file path for a specific job
fn resolve_job_event_file(job_id: &str) -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    let repo_name = crate::storage::extract_repo_name(&current_dir)?;
    let global_base = crate::storage::get_default_storage_dir()?;
    let job_events_dir = global_base.join("events").join(&repo_name).join(job_id);

    if !job_events_dir.exists() {
        return Err(anyhow::anyhow!("Job '{}' not found", job_id));
    }

    // Find the most recent event file
    let event_files = find_event_files(&job_events_dir)?;

    event_files
        .into_iter()
        .next_back()
        .ok_or_else(|| anyhow::anyhow!("No event files found for job '{}'", job_id))
}

/// Resolve event file path, with fallback to local file if it exists
fn resolve_event_file_with_fallback(file: PathBuf, job_id: Option<&str>) -> Result<PathBuf> {
    // If the provided file exists, use it directly
    if file.exists() {
        return Ok(file);
    }

    // If a job_id is provided, resolve it from global storage
    if let Some(job_id) = job_id {
        if let Ok(resolved) = resolve_job_event_file(job_id) {
            info!("Using global event file: {:?}", resolved);
            return Ok(resolved);
        }
    }

    // Return the original path (will fail gracefully if it doesn't exist)
    Ok(file)
}

/// Get all event files from global storage for aggregate operations
fn get_all_event_files() -> Result<Vec<PathBuf>> {
    // Always use global storage for event file aggregation

    let current_dir = std::env::current_dir()?;
    let repo_name = crate::storage::extract_repo_name(&current_dir)?;
    let global_base = crate::storage::get_default_storage_dir()?;
    let global_events_dir = global_base.join("events").join(&repo_name);

    if !global_events_dir.exists() {
        return Ok(Vec::new());
    }

    let mut all_files = Vec::new();

    // Iterate through all job directories
    for entry in fs::read_dir(&global_events_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let event_files = find_event_files(&entry.path())?;
            all_files.extend(event_files);
        }
    }

    all_files.sort();
    Ok(all_files)
}

// =============================================================================
// Pure Data Transformation Functions
// =============================================================================

/// Pure function to build event filter criteria
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub job_id: Option<String>,
    pub event_type: Option<String>,
    pub agent_id: Option<String>,
    pub since_time: Option<DateTime<Utc>>,
}

impl EventFilter {
    fn new(
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

    fn matches_event(&self, event: &Value) -> bool {
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
fn calculate_event_statistics(
    events: impl Iterator<Item = Value>,
    group_by: &str,
) -> (std::collections::HashMap<String, usize>, usize) {
    use std::collections::HashMap;

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
fn sort_statistics_by_count(
    stats: std::collections::HashMap<String, usize>,
) -> Vec<(String, usize)> {
    let mut sorted_stats: Vec<_> = stats.into_iter().collect();
    sorted_stats.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_stats
}

/// Pure function to format statistics as human-readable string
fn format_statistics_human(
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

/// Pure function to check if event matches search pattern
fn event_matches_search(event: &Value, pattern: &regex::Regex, fields: Option<&[String]>) -> bool {
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
fn parse_event_line(line: &str) -> Option<Value> {
    if line.trim().is_empty() {
        return None;
    }
    serde_json::from_str(line).ok()
}

/// Pure function to convert duration string to days
fn convert_duration_to_days(duration_str: &str) -> Result<u32> {
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
fn convert_size_to_bytes(size_str: &str) -> Result<u64> {
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
fn validate_retention_policy(
    older_than: &Option<String>,
    max_events: &Option<usize>,
    max_size: &Option<String>,
) -> Result<()> {
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

// =============================================================================
// Pure Formatting Functions
// =============================================================================

/// Pure function to format job info for display
fn create_job_display_info(job: &JobInfo) -> String {
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
fn format_statistics_json(
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
fn format_statistics_yaml(
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
fn create_cleanup_summary_message(total_cleaned: usize, dry_run: bool) -> String {
    if total_cleaned == 0 {
        "No events matched the cleanup criteria.".to_string()
    } else if dry_run {
        format!("Would clean {} events", total_cleaned)
    } else {
        format!("Cleaned {} events", total_cleaned)
    }
}

/// Pure function to create cleanup summary JSON
fn create_cleanup_summary_json(
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
fn create_cleanup_summary_human(
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

// =============================================================================
// Command Execution Functions
// =============================================================================

/// Execute event viewer commands
pub async fn execute(args: EventsArgs) -> Result<()> {
    match args.command {
        EventsCommand::Ls {
            job_id,
            event_type,
            agent_id,
            since,
            limit,
            file,
            output_format,
        } => {
            // If no job_id provided and no explicit file, show available jobs
            if job_id.is_none() && !file.exists() {
                display_available_jobs()?;
                Ok(())
            } else {
                // Resolve the event file and list events
                let resolved_file = resolve_event_file_with_fallback(file, job_id.as_deref())?;
                list_events(
                    resolved_file,
                    job_id,
                    event_type,
                    agent_id,
                    since,
                    limit,
                    output_format,
                )
                .await
            }
        }

        EventsCommand::Stats {
            file,
            group_by,
            output_format,
        } => {
            // If no explicit file, aggregate all events from global storage
            if !file.exists() {
                show_aggregated_stats(group_by, output_format).await
            } else {
                let resolved_file = resolve_event_file_with_fallback(file, None)?;
                show_stats(resolved_file, group_by, output_format).await
            }
        }

        EventsCommand::Search {
            pattern,
            file,
            fields,
        } => {
            // If no explicit file, search all events from global storage
            if !file.exists() {
                search_aggregated_events(pattern, fields).await
            } else {
                let resolved_file = resolve_event_file_with_fallback(file, None)?;
                search_events(resolved_file, pattern, fields).await
            }
        }

        EventsCommand::Follow {
            file,
            job_id,
            event_type,
        } => {
            let resolved_file = resolve_event_file_with_fallback(file, job_id.as_deref())?;
            follow_events(resolved_file, job_id, event_type).await
        }

        EventsCommand::Export {
            file,
            format,
            output,
        } => {
            // If no explicit file, export all events from global storage
            if !file.exists() {
                export_aggregated_events(format, output).await
            } else {
                let resolved_file = resolve_event_file_with_fallback(file, None)?;
                export_events(resolved_file, format, output).await
            }
        }

        EventsCommand::Clean {
            older_than,
            max_events,
            max_size,
            dry_run,
            archive,
            archive_path,
            all_jobs,
            job_id,
            file,
            output_format,
        } => {
            clean_events(
                older_than,
                max_events,
                max_size,
                dry_run,
                archive,
                archive_path,
                all_jobs,
                job_id,
                file,
                output_format,
            )
            .await
        }
    }
}

/// Display available jobs with their status
fn display_available_jobs() -> Result<()> {
    let jobs = get_available_jobs()?;

    if jobs.is_empty() {
        println!("No MapReduce jobs found in global storage.");
        return Ok(());
    }

    println!("Available MapReduce jobs:");
    println!("{}", "=".repeat(50));

    for job in jobs {
        let job_info = format_job_info(&job);
        println!("  • {}", job_info);
    }

    println!("{}", "=".repeat(50));
    println!("\nTo view events for a specific job:");
    println!("  prodigy events list --job-id <JOB_ID>");
    println!("\nTo view all recent events across jobs:");
    println!("  prodigy events list --file .prodigy/events/mapreduce_events.jsonl");

    Ok(())
}

/// Format job information for display (now delegates to pure function)
fn format_job_info(job: &JobInfo) -> String {
    create_job_display_info(job)
}

/// Calculate duration between start and end times
fn calculate_duration(start: Option<DateTime<Local>>, end: Option<DateTime<Local>>) -> String {
    match (start, end) {
        (Some(start), Some(end)) => {
            let diff = end.signed_duration_since(start);
            format!(" in {}m{}s", diff.num_minutes(), diff.num_seconds() % 60)
        }
        _ => String::new(),
    }
}

/// Calculate elapsed time from start
fn calculate_elapsed(start: Option<DateTime<Local>>) -> String {
    match start {
        Some(start) => {
            let diff = Local::now().signed_duration_since(start);
            format!(" - running for {}m", diff.num_minutes())
        }
        None => String::new(),
    }
}

/// List events with optional filters (refactored to separate I/O from logic)
async fn list_events(
    file: PathBuf,
    job_id: Option<String>,
    event_type: Option<String>,
    agent_id: Option<String>,
    since: Option<u64>,
    limit: usize,
    output_format: String,
) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    // Create filter using pure function
    let filter = EventFilter::new(job_id, event_type, agent_id, since);

    // Read events from file
    let events = read_and_filter_events(&file, &filter, limit)?;

    // Display events using pure functions
    display_events_with_format(&events, &output_format)
}

/// Pure I/O function to read and filter events from file
fn read_and_filter_events(
    file: &PathBuf,
    filter: &EventFilter,
    limit: usize,
) -> Result<Vec<Value>> {
    let file_handle = fs::File::open(file)?;
    let reader = BufReader::new(file_handle);

    let events = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| parse_event_line(&line))
        .filter(|event| filter.matches_event(event))
        .take(limit)
        .collect();

    Ok(events)
}

/// Pure function to display events in the specified format
fn display_events_with_format(events: &[Value], output_format: &str) -> Result<()> {
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

/// Show aggregated statistics from all global event files (refactored)
async fn show_aggregated_stats(group_by: String, output_format: String) -> Result<()> {
    let event_files = get_all_event_files()?;

    if event_files.is_empty() {
        println!("No events found in global storage.");
        return Ok(());
    }

    // Read all events and calculate statistics using pure functions
    let all_events = read_events_from_files(&event_files)?;
    let (stats, total) = calculate_event_statistics(all_events.into_iter(), &group_by);
    let sorted_stats = sort_statistics_by_count(stats);

    // Display statistics using pure functions
    display_statistics_with_format(&sorted_stats, total, &group_by, &output_format, true)
}

/// Pure I/O function to read events from multiple files
fn read_events_from_files(event_files: &[PathBuf]) -> Result<Vec<Value>> {
    let mut all_events = Vec::new();

    for file in event_files {
        let content = fs::File::open(file)?;
        let reader = BufReader::new(content);

        for line in reader.lines() {
            let line = line?;
            if let Some(event) = parse_event_line(&line) {
                all_events.push(event);
            }
        }
    }

    Ok(all_events)
}

/// Pure function to display statistics in the specified format
fn display_statistics_with_format(
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

/// Show event statistics (refactored to use pure functions)
async fn show_stats(file: PathBuf, group_by: String, output_format: String) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    // Read events and calculate statistics using pure functions
    let events = read_events_from_single_file(&file)?;
    let (stats, total) = calculate_event_statistics(events.into_iter(), &group_by);
    let sorted_stats = sort_statistics_by_count(stats);

    // Display statistics using pure functions
    display_statistics_with_format(&sorted_stats, total, &group_by, &output_format, false)
}

/// Pure I/O function to read events from a single file
fn read_events_from_single_file(file: &PathBuf) -> Result<Vec<Value>> {
    let file_handle = fs::File::open(file)?;
    let reader = BufReader::new(file_handle);

    let events = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| parse_event_line(&line))
        .collect();

    Ok(events)
}

/// Search aggregated events from all global event files (refactored)
async fn search_aggregated_events(pattern: String, fields: Option<Vec<String>>) -> Result<()> {
    let event_files = get_all_event_files()?;

    if event_files.is_empty() {
        println!("No events found in global storage.");
        return Ok(());
    }

    // Read all events and search using pure functions
    let all_events = read_events_from_files(&event_files)?;
    let matching_events = search_events_with_pattern(&all_events, &pattern, fields.as_deref())?;

    // Display results
    display_search_results(&matching_events, true)
}

/// Pure function to search events with a pattern
fn search_events_with_pattern(
    events: &[Value],
    pattern: &str,
    fields: Option<&[String]>,
) -> Result<Vec<Value>> {
    use regex::Regex;
    let re = Regex::new(pattern)?;

    let matching_events = events
        .iter()
        .filter(|event| event_matches_search(event, &re, fields))
        .cloned()
        .collect();

    Ok(matching_events)
}

/// Pure function to display search results
fn display_search_results(matching_events: &[Value], is_aggregated: bool) -> Result<()> {
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

/// Search events by pattern (refactored to use pure functions)
async fn search_events(file: PathBuf, pattern: String, fields: Option<Vec<String>>) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    // Read events and search using pure functions
    let events = read_events_from_single_file(&file)?;
    let matching_events = search_events_with_pattern(&events, &pattern, fields.as_deref())?;

    // Display results
    display_search_results(&matching_events, false)
}

/// Follow events in real-time (refactored to smaller functions)
async fn follow_events(
    file: PathBuf,
    job_id: Option<String>,
    event_type: Option<String>,
) -> Result<()> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    setup_file_for_watching(&file)?;

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher = setup_file_watcher(tx)?;
    let watch_path = determine_watch_path(&file);
    watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

    println!("Following events (Ctrl+C to stop)...\n");

    if file.exists() {
        watch_existing_file(&file, &job_id, &event_type, rx).await
    } else {
        wait_for_file_creation(&file, &job_id, &event_type, rx).await
    }
}

/// Setup file and directory structure for watching
fn setup_file_for_watching(file: &PathBuf) -> Result<()> {
    if !file.exists() {
        println!("Waiting for events file to be created: {:?}", file);
        // Create parent directory if it doesn't exist
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Setup file watcher with event handling
fn setup_file_watcher(
    tx: std::sync::mpsc::Sender<notify::Event>,
) -> Result<notify::RecommendedWatcher> {
    let watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    Ok(watcher)
}

/// Determine the path to watch based on file existence
fn determine_watch_path(file: &Path) -> PathBuf {
    if file.exists() {
        file.to_path_buf()
    } else {
        file.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| file.to_path_buf())
    }
}

/// Watch existing file for new events
async fn watch_existing_file(
    file: &Path,
    job_id: &Option<String>,
    event_type: &Option<String>,
    rx: std::sync::mpsc::Receiver<notify::Event>,
) -> Result<()> {
    use std::time::Duration;

    let mut last_pos = display_existing_events(file, job_id, event_type)?;

    // Watch for new events
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) => {
                // File changed, read new events
                last_pos = display_new_events(file, last_pos, job_id, event_type)?;
            }
            Err(_) => {
                // Timeout, continue waiting
                continue;
            }
        }
    }
}

/// Wait for file to be created and then start monitoring
async fn wait_for_file_creation(
    file: &Path,
    job_id: &Option<String>,
    event_type: &Option<String>,
    rx: std::sync::mpsc::Receiver<notify::Event>,
) -> Result<()> {
    use std::time::Duration;

    // Wait for file to be created
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) => {
                if file.exists() {
                    let _ = display_existing_events(file, job_id, event_type)?;
                    break;
                }
            }
            Err(_) => continue,
        }
    }

    Ok(())
}

/// Export aggregated events from all global event files
async fn export_aggregated_events(format: String, output: Option<PathBuf>) -> Result<()> {
    let event_files = get_all_event_files()?;

    if event_files.is_empty() {
        println!("No events found in global storage.");
        return Ok(());
    }

    let mut events = Vec::new();

    // Collect all events from all files
    for file in event_files {
        let content = fs::File::open(&file)?;
        let reader = BufReader::new(content);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Value = serde_json::from_str(&line)?;
            events.push(event);
        }
    }

    let exported = match format.as_str() {
        "json" => export_as_json(&events)?,
        "csv" => export_as_csv(&events)?,
        "markdown" => export_as_markdown(&events)?,
        _ => return Err(anyhow::anyhow!("Unsupported format: {}", format)),
    };

    if let Some(output_path) = output {
        fs::write(output_path, exported)?;
        println!(
            "Events exported successfully ({} events from all jobs)",
            events.len()
        );
    } else {
        println!("{}", exported);
    }

    Ok(())
}

/// Export events to different format
async fn export_events(file: PathBuf, format: String, output: Option<PathBuf>) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    let file = fs::File::open(file)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(&line)?;
        events.push(event);
    }

    let exported = match format.as_str() {
        "json" => export_as_json(&events)?,
        "csv" => export_as_csv(&events)?,
        "markdown" => export_as_markdown(&events)?,
        _ => return Err(anyhow::anyhow!("Unsupported format: {}", format)),
    };

    if let Some(output_path) = output {
        fs::write(output_path, exported)?;
        println!("Events exported successfully");
    } else {
        println!("{}", exported);
    }

    Ok(())
}

/// Clean up old events based on retention policy
#[allow(clippy::too_many_arguments)]
async fn clean_events(
    older_than: Option<String>,
    max_events: Option<usize>,
    max_size: Option<String>,
    dry_run: bool,
    archive: bool,
    archive_path: Option<PathBuf>,
    all_jobs: bool,
    job_id: Option<String>,
    file: Option<PathBuf>,
    output_format: String,
) -> Result<()> {
    // Build retention policy from arguments
    let policy = build_retention_policy(older_than, max_events, max_size, archive, archive_path)?;

    // First perform dry-run analysis to show what will be cleaned
    let _analysis_total = if !dry_run {
        let analysis = analyze_retention_targets(all_jobs, job_id.as_deref(), &policy).await?;

        if !confirm_cleanup(&analysis).await? {
            return Ok(());
        }
        analysis
    } else {
        crate::cook::execution::events::retention::RetentionAnalysis::default()
    };

    display_retention_policy(&policy, dry_run);

    let (total_cleaned, total_archived) = if let Some(specific_file) = file {
        clean_specific_file(&specific_file, &policy, dry_run).await?
    } else if all_jobs || job_id.is_some() {
        clean_global_storage(job_id.as_deref(), &policy, dry_run, &output_format).await?
    } else {
        clean_local_storage(&policy, dry_run, &output_format).await?
    };

    display_cleanup_summary(total_cleaned, total_archived, dry_run, &output_format)?;

    Ok(())
}

/// Build retention policy from command arguments (refactored to use pure functions)
fn build_retention_policy(
    older_than: Option<String>,
    max_events: Option<usize>,
    max_size: Option<String>,
    archive: bool,
    archive_path: Option<PathBuf>,
) -> Result<crate::cook::execution::events::retention::RetentionPolicy> {
    use crate::cook::execution::events::retention::RetentionPolicy;

    // Validate parameters first using pure function
    validate_retention_policy(&older_than, &max_events, &max_size)?;

    let mut policy = RetentionPolicy::default();

    if let Some(duration_str) = older_than {
        let days = convert_duration_to_days(&duration_str)?;
        policy.max_age_days = Some(days);
    }

    if let Some(max_events) = max_events {
        policy.max_events = Some(max_events);
    }

    if let Some(size_str) = max_size {
        let bytes = convert_size_to_bytes(&size_str)?;
        policy.max_file_size_bytes = Some(bytes);
    }

    policy.archive_old_events = archive;
    if let Some(path) = archive_path {
        policy.archive_path = Some(path);
    }

    Ok(policy)
}

/// Analyze retention targets and calculate what will be cleaned
async fn analyze_retention_targets(
    all_jobs: bool,
    job_id: Option<&str>,
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
) -> Result<crate::cook::execution::events::retention::RetentionAnalysis> {
    use crate::cook::execution::events::retention::{RetentionAnalysis, RetentionManager};

    let mut analysis_total = RetentionAnalysis::default();

    if all_jobs || job_id.is_some() {
        let current_dir = std::env::current_dir()?;
        let repo_name = crate::storage::extract_repo_name(&current_dir)?;
        let global_base = crate::storage::get_default_storage_dir()?;
        let global_events_dir = global_base.join("events").join(&repo_name);

        if global_events_dir.exists() {
            let job_dirs = get_job_directories(&global_events_dir, job_id)?;

            for job_dir in job_dirs {
                let event_files = find_event_files(&job_dir)?;
                for event_file in event_files {
                    let retention = RetentionManager::new(policy.clone(), event_file);
                    let analysis = retention.analyze_retention().await?;
                    analysis_total.events_to_remove += analysis.events_to_remove;
                    analysis_total.space_to_save += analysis.space_to_save;
                    if policy.archive_old_events {
                        analysis_total.events_to_archive += analysis.events_to_archive;
                    }
                }
            }
        }
    } else {
        let local_file = PathBuf::from(".prodigy/events/mapreduce_events.jsonl");
        if local_file.exists() {
            let retention = RetentionManager::new(policy.clone(), local_file);
            analysis_total = retention.analyze_retention().await?;
        }
    }

    Ok(analysis_total)
}

/// Get job directories to process
fn get_job_directories(global_events_dir: &Path, job_id: Option<&str>) -> Result<Vec<PathBuf>> {
    if let Some(specific_job_id) = job_id {
        let specific_dir = global_events_dir.join(specific_job_id);
        if specific_dir.exists() {
            Ok(vec![specific_dir])
        } else {
            Ok(vec![])
        }
    } else {
        Ok(fs::read_dir(global_events_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect())
    }
}

/// Confirm cleanup with user if not in automation mode
async fn confirm_cleanup(
    analysis: &crate::cook::execution::events::retention::RetentionAnalysis,
) -> Result<bool> {
    if analysis.events_to_remove == 0 {
        println!("No events match the cleanup criteria.");
        return Ok(false);
    }

    println!("Events cleanup preview:");
    println!("  Events to remove: {}", analysis.events_to_remove);
    if analysis.events_to_archive > 0 {
        println!("  Events to archive: {}", analysis.events_to_archive);
    }
    println!(
        "  Space to save: {:.1} MB",
        analysis.space_to_save as f64 / (1024.0 * 1024.0)
    );
    println!();

    if std::env::var("PRODIGY_AUTOMATION").unwrap_or_default() != "true" {
        let prompter = UserPrompterImpl::new();
        let confirm = prompter
            .prompt_yes_no(&format!(
                "This will permanently remove {} events. Continue?",
                analysis.events_to_remove
            ))
            .await?;

        if !confirm {
            println!("Cleanup cancelled.");
            return Ok(false);
        }
    }

    Ok(true)
}

/// Display retention policy details
fn display_retention_policy(
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
    dry_run: bool,
) {
    let action = if dry_run { "Would clean" } else { "Cleaning" };
    println!("{} events with policy:", action);
    println!("  Max age: {:?} days", policy.max_age_days);
    println!("  Max events: {:?}", policy.max_events);
    println!("  Max file size: {:?} bytes", policy.max_file_size_bytes);
    println!("  Archive: {}", policy.archive_old_events);
    println!();
}

/// Clean a specific event file
async fn clean_specific_file(
    specific_file: &Path,
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
    dry_run: bool,
) -> Result<(usize, usize)> {
    use crate::cook::execution::events::retention::RetentionManager;

    if !specific_file.exists() {
        return Err(anyhow::anyhow!(
            "Event file not found: {}",
            specific_file.display()
        ));
    }

    let retention = RetentionManager::new(policy.clone(), specific_file.to_path_buf());

    if dry_run {
        let analysis = retention.analyze_retention().await?;
        println!(
            "[DRY RUN] Would remove {} events from {}",
            analysis.events_to_remove,
            specific_file.display()
        );
        if analysis.events_to_archive > 0 {
            println!(
                "[DRY RUN] Would archive {} events",
                analysis.events_to_archive
            );
        }
        println!(
            "[DRY RUN] Would save {:.1} MB",
            analysis.space_to_save as f64 / (1024.0 * 1024.0)
        );
        Ok((analysis.events_to_remove, analysis.events_to_archive))
    } else {
        let result = retention.apply_retention().await?;
        if result.events_removed > 0 {
            println!(
                "Cleaned {} events from {}",
                result.events_removed,
                specific_file.display()
            );
        }
        Ok((result.events_removed, result.events_archived))
    }
}

/// Clean events from global storage
async fn clean_global_storage(
    job_id: Option<&str>,
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
    dry_run: bool,
    output_format: &str,
) -> Result<(usize, usize)> {
    use crate::cook::execution::events::retention::RetentionManager;

    let current_dir = std::env::current_dir()?;
    let repo_name = crate::storage::extract_repo_name(&current_dir)?;
    let global_base = crate::storage::get_default_storage_dir()?;
    let global_events_dir = global_base.join("events").join(&repo_name);

    if !global_events_dir.exists() {
        println!(
            "No events found in global storage for repository: {}",
            repo_name
        );
        return Ok((0, 0));
    }

    let job_dirs = get_job_directories(&global_events_dir, job_id)?;
    if job_dirs.is_empty() {
        if let Some(id) = job_id {
            println!("Job '{}' not found", id);
        }
        return Ok((0, 0));
    }

    let mut total_cleaned = 0usize;
    let mut total_archived = 0usize;

    for job_dir in job_dirs {
        let job_id = job_dir
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|| "unknown".into());
        println!("Processing job: {}", job_id);

        let event_files = find_event_files(&job_dir)?;
        for event_file in event_files {
            let retention = RetentionManager::new(policy.clone(), event_file.clone());

            if dry_run {
                let analysis = retention.analyze_retention().await?;
                if output_format != "json" {
                    println!("  Analyzing: {:?}", event_file);
                    if analysis.events_to_remove > 0 {
                        println!("    Would remove {} events", analysis.events_to_remove);
                        println!("    Would save {} bytes", analysis.space_to_save);
                    } else {
                        println!("    No events to remove");
                    }
                }
                total_cleaned += analysis.events_to_remove;
                if policy.archive_old_events {
                    total_archived += analysis.events_to_archive;
                }
            } else {
                let stats = retention.apply_retention().await?;
                println!(
                    "  Cleaned: {} events, {:.1}% space saved",
                    stats.events_removed,
                    stats.space_saved_percentage()
                );
                total_cleaned += stats.events_removed;
                if policy.archive_old_events {
                    total_archived += stats.events_removed;
                }
            }
        }
    }

    Ok((total_cleaned, total_archived))
}

/// Clean events from local storage
async fn clean_local_storage(
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
    dry_run: bool,
    output_format: &str,
) -> Result<(usize, usize)> {
    use crate::cook::execution::events::retention::RetentionManager;

    let local_file = PathBuf::from(".prodigy/events/mapreduce_events.jsonl");
    if !local_file.exists() {
        println!("No local events file found: {:?}", local_file);
        return Ok((0, 0));
    }

    let retention = RetentionManager::new(policy.clone(), local_file.clone());

    if dry_run {
        let analysis = retention.analyze_retention().await?;
        if output_format == "json" {
            println!("{}", serde_json::to_string_pretty(&analysis)?);
        } else {
            analysis.display_human();
        }

        let archived = if policy.archive_old_events {
            analysis.events_to_archive
        } else {
            0
        };
        Ok((analysis.events_to_remove, archived))
    } else {
        let stats = retention.apply_retention().await?;
        println!(
            "Cleaned: {} events, {:.1}% space saved",
            stats.events_removed,
            stats.space_saved_percentage()
        );

        let archived = if policy.archive_old_events {
            stats.events_removed
        } else {
            0
        };
        Ok((stats.events_removed, archived))
    }
}

/// Display cleanup summary (refactored to use pure functions)
fn display_cleanup_summary(
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

// Helper functions

fn event_matches_field(event: &Value, field: &str, value: &str) -> bool {
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

fn event_matches_type(event: &Value, event_type: &str) -> bool {
    get_event_type(event) == event_type
}

fn get_event_type(event: &Value) -> String {
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

fn event_is_recent(event: &Value, since_time: DateTime<Utc>) -> bool {
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
fn extract_event_metadata(event: &Value) -> (String, String, String) {
    let event_type = get_event_type(event);
    let timestamp = extract_timestamp(event);
    let job_id = extract_job_id(event);

    let time_str = format_timestamp(timestamp);
    (event_type, time_str, job_id)
}

/// Extract job ID from event
fn extract_job_id(event: &Value) -> String {
    event
        .get("job_id")
        .or_else(|| extract_nested_field(event, "job_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("n/a")
        .to_string()
}

/// Format timestamp for display
fn format_timestamp(timestamp: Option<DateTime<Utc>>) -> String {
    timestamp
        .map(|ts| {
            ts.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "n/a".to_string())
}

/// Display JobStarted event
fn display_job_started(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
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
fn display_job_completed(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
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
fn display_agent_progress(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
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
fn display_generic_event(event: &Value, time_str: &str, event_type: &str, job_id: &str) {
    println!(
        "[{}] {} - Job: {} - {}",
        time_str,
        event_type,
        job_id,
        serde_json::to_string(event).unwrap_or_default()
    );
}

fn display_event(event: &Value) {
    let (event_type, time_str, job_id) = extract_event_metadata(event);

    match event_type.as_str() {
        "JobStarted" => display_job_started(event, &time_str, &event_type, &job_id),
        "JobCompleted" => display_job_completed(event, &time_str, &event_type, &job_id),
        "AgentProgress" => display_agent_progress(event, &time_str, &event_type, &job_id),
        _ => display_generic_event(event, &time_str, &event_type, &job_id),
    }
}

fn extract_timestamp(event: &Value) -> Option<DateTime<Utc>> {
    let timestamp_str = event
        .get("timestamp")
        .or_else(|| extract_nested_field(event, "timestamp"))
        .or_else(|| event.get("time"))
        .and_then(|v| v.as_str());

    timestamp_str
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn extract_nested_field<'a>(event: &'a Value, field: &str) -> Option<&'a Value> {
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

fn search_in_value(value: &Value, re: &regex::Regex) -> bool {
    match value {
        Value::String(s) => re.is_match(s),
        Value::Object(map) => map.values().any(|v| search_in_value(v, re)),
        Value::Array(arr) => arr.iter().any(|v| search_in_value(v, re)),
        _ => false,
    }
}

fn display_existing_events(
    file: &Path,
    job_id: &Option<String>,
    event_type: &Option<String>,
) -> Result<u64> {
    let file = fs::File::open(file)?;
    let mut reader = BufReader::new(file);

    for line in reader.by_ref().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: Value = serde_json::from_str(&line)?;

        // Apply filters
        if let Some(ref jid) = job_id {
            if !event_matches_field(&event, "job_id", jid) {
                continue;
            }
        }

        if let Some(ref etype) = event_type {
            if !event_matches_type(&event, etype) {
                continue;
            }
        }

        display_event(&event);
    }

    // Get current position
    let last_pos = reader.into_inner().stream_position()?;

    Ok(last_pos)
}

fn display_new_events(
    file: &Path,
    last_pos: u64,
    job_id: &Option<String>,
    event_type: &Option<String>,
) -> Result<u64> {
    let mut file = fs::File::open(file)?;
    file.seek(SeekFrom::Start(last_pos))?;
    let mut reader = BufReader::new(file);

    for line in reader.by_ref().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: Value = serde_json::from_str(&line)?;

        // Apply filters
        if let Some(ref jid) = job_id {
            if !event_matches_field(&event, "job_id", jid) {
                continue;
            }
        }

        if let Some(ref etype) = event_type {
            if !event_matches_type(&event, etype) {
                continue;
            }
        }

        display_event(&event);
    }

    // Update position
    let new_pos = reader.into_inner().stream_position()?;

    Ok(new_pos)
}

fn export_as_json(events: &[Value]) -> Result<String> {
    Ok(serde_json::to_string_pretty(&events)?)
}

fn export_as_csv(events: &[Value]) -> Result<String> {
    use std::fmt::Write;

    let mut csv = String::new();

    // Write header
    writeln!(&mut csv, "timestamp,event_type,job_id,agent_id,details")?;

    // Write rows
    for event in events {
        let timestamp = extract_timestamp(event)
            .map(|ts| ts.to_rfc3339())
            .unwrap_or_default();
        let event_type = get_event_type(event);
        let job_id = event
            .get("job_id")
            .or_else(|| extract_nested_field(event, "job_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let agent_id = event
            .get("agent_id")
            .or_else(|| extract_nested_field(event, "agent_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
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

fn export_as_markdown(events: &[Value]) -> Result<String> {
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
        let timestamp = extract_timestamp(event)
            .map(|ts| {
                ts.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|| "n/a".to_string());
        let event_type = get_event_type(event);
        let job_id = event
            .get("job_id")
            .or_else(|| extract_nested_field(event, "job_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");
        let agent_id = event
            .get("agent_id")
            .or_else(|| extract_nested_field(event, "agent_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");

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
fn print_table_header() {
    println!(
        "{:<20} {:<15} {:<20} {:<15} {:<30}",
        "Timestamp", "Event Type", "Job ID", "Agent ID", "Details"
    );
    println!("{}", "-".repeat(100));
}

/// Extract agent ID from event
fn extract_agent_id(event: &Value) -> String {
    event
        .get("agent_id")
        .or_else(|| extract_nested_field(event, "agent_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("n/a")
        .to_string()
}

/// Truncate string to fit in table column
fn truncate_field(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Extract table row data from event
fn extract_table_row_data(event: &Value) -> (String, String, String, String, String) {
    let timestamp = format_timestamp(extract_timestamp(event));
    let event_type = get_event_type(event);
    let job_id = extract_job_id(event);
    let agent_id = extract_agent_id(event);
    let details = format_event_details(event);

    (timestamp, event_type, job_id, agent_id, details)
}

/// Print a single event as a table row
fn print_event_row(event: &Value) {
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
fn display_events_as_table(events: &[Value]) -> Result<()> {
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

fn format_event_details(event: &Value) -> String {
    let event_type = get_event_type(event);

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn test_get_event_type_job_started() {
        let event = json!({
            "JobStarted": {
                "job_id": "test-123",
                "timestamp": "2024-01-01T00:00:00Z"
            }
        });
        assert_eq!(get_event_type(&event), "JobStarted");
    }

    #[test]
    fn test_get_event_type_job_completed() {
        let event = json!({
            "JobCompleted": {
                "job_id": "test-123",
                "success_count": 5,
                "failure_count": 0
            }
        });
        assert_eq!(get_event_type(&event), "JobCompleted");
    }

    #[test]
    fn test_get_event_type_job_failed() {
        let event = json!({
            "JobFailed": {
                "job_id": "test-123",
                "error": "Test error"
            }
        });
        assert_eq!(get_event_type(&event), "JobFailed");
    }

    #[test]
    fn test_get_event_type_job_paused() {
        let event = json!({
            "JobPaused": {
                "job_id": "test-123",
                "reason": "Manual pause"
            }
        });
        assert_eq!(get_event_type(&event), "JobPaused");
    }

    #[test]
    fn test_get_event_type_job_resumed() {
        let event = json!({
            "JobResumed": {
                "job_id": "test-123"
            }
        });
        assert_eq!(get_event_type(&event), "JobResumed");
    }

    #[test]
    fn test_build_retention_policy_with_all_options() {
        let policy = build_retention_policy(
            Some("30d".to_string()),
            Some(100),
            Some("1GB".to_string()),
            false,
            None,
        )
        .unwrap();
        assert_eq!(policy.max_age_days, Some(30));
        assert_eq!(policy.max_events, Some(100));
        assert_eq!(policy.max_file_size_bytes, Some(1_073_741_824));
        assert!(!policy.archive_old_events);
        // archive_path has a default value from RetentionPolicy::default()
        assert!(policy.archive_path.is_some());
    }

    #[test]
    fn test_convert_duration_to_days() {
        assert_eq!(convert_duration_to_days("30d").unwrap(), 30);
        assert_eq!(convert_duration_to_days("7d").unwrap(), 7);
        assert_eq!(convert_duration_to_days("1d").unwrap(), 1);
        assert_eq!(convert_duration_to_days("365d").unwrap(), 365);
        assert_eq!(convert_duration_to_days("30").unwrap(), 30); // Default to days
        assert!(convert_duration_to_days("invalid").is_err());
    }

    #[test]
    fn test_convert_size_to_bytes() {
        assert_eq!(convert_size_to_bytes("1KB").unwrap(), 1_024);
        assert_eq!(convert_size_to_bytes("1MB").unwrap(), 1_048_576);
        assert_eq!(convert_size_to_bytes("1GB").unwrap(), 1_073_741_824);
        assert_eq!(convert_size_to_bytes("500MB").unwrap(), 524_288_000);
        assert!(convert_size_to_bytes("invalid").is_err());
        // 100 without unit is now valid (defaults to bytes)
        assert_eq!(convert_size_to_bytes("100").unwrap(), 100);
    }

    #[test]
    fn test_format_job_info() {
        let job = JobInfo {
            id: "test-123".to_string(),
            status: JobStatus::Completed,
            start_time: Some(Local.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap()),
            end_time: Some(Local.with_ymd_and_hms(2024, 1, 1, 10, 30, 0).unwrap()),
            success_count: 95,
            failure_count: 5,
        };

        let formatted = format_job_info(&job);
        assert!(formatted.contains("test-123"));
        assert!(formatted.contains("COMPLETED"));
        assert!(formatted.contains("Success: 95"));
        assert!(formatted.contains("Failed: 5"));
        assert!(formatted.contains(" in 30m0s"));
    }

    #[test]
    fn test_calculate_duration() {
        let start = Some(Local.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap());
        let end = Some(Local.with_ymd_and_hms(2024, 1, 1, 10, 30, 45).unwrap());
        assert_eq!(calculate_duration(start, end), " in 30m45s");

        let end2 = Some(Local.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap());
        assert_eq!(calculate_duration(start, end2), " in 60m0s");

        assert_eq!(calculate_duration(None, end), "");
        assert_eq!(calculate_duration(start, None), "");
    }

    #[test]
    fn test_event_matches_field() {
        let event = json!({
            "JobStarted": {
                "job_id": "test-123",
                "workflow_name": "test-workflow"
            },
            "timestamp": "2024-01-01T00:00:00Z"
        });

        assert!(event_matches_field(&event, "job_id", "test-123"));
        assert!(event_matches_field(
            &event,
            "workflow_name",
            "test-workflow"
        ));
        assert!(!event_matches_field(&event, "job_id", "different-id"));
        assert!(!event_matches_field(&event, "nonexistent", "value"));
    }

    #[test]
    fn test_extract_nested_field() {
        let event = json!({
            "JobStarted": {
                "job_id": "test-123",
                "total_items": 10
            }
        });

        let result = extract_nested_field(&event, "job_id");
        assert_eq!(result, Some(&json!("test-123")));

        let result2 = extract_nested_field(&event, "total_items");
        assert_eq!(result2, Some(&json!(10)));

        let result3 = extract_nested_field(&event, "nonexistent");
        assert_eq!(result3, None);
    }

    #[test]
    fn test_get_event_type_agent_started() {
        let event = json!({
            "AgentStarted": {
                "agent_id": "agent-1",
                "work_item": {}
            }
        });
        assert_eq!(get_event_type(&event), "AgentStarted");
    }

    #[test]
    fn test_get_event_type_agent_progress() {
        let event = json!({
            "AgentProgress": {
                "agent_id": "agent-1",
                "step": "Running tests",
                "progress_pct": 50.0
            }
        });
        assert_eq!(get_event_type(&event), "AgentProgress");
    }

    #[test]
    fn test_get_event_type_agent_completed() {
        let event = json!({
            "AgentCompleted": {
                "agent_id": "agent-1",
                "result": "Success"
            }
        });
        assert_eq!(get_event_type(&event), "AgentCompleted");
    }

    #[test]
    fn test_get_event_type_agent_failed() {
        let event = json!({
            "AgentFailed": {
                "agent_id": "agent-1",
                "error": "Test failure"
            }
        });
        assert_eq!(get_event_type(&event), "AgentFailed");
    }

    #[test]
    fn test_get_event_type_pipeline_started() {
        let event = json!({
            "PipelineStarted": {
                "pipeline_id": "pipeline-1"
            }
        });
        assert_eq!(get_event_type(&event), "PipelineStarted");
    }

    #[test]
    fn test_get_event_type_pipeline_stage_completed() {
        let event = json!({
            "PipelineStageCompleted": {
                "pipeline_id": "pipeline-1",
                "stage": "build"
            }
        });
        assert_eq!(get_event_type(&event), "PipelineStageCompleted");
    }

    #[test]
    fn test_get_event_type_pipeline_completed() {
        let event = json!({
            "PipelineCompleted": {
                "pipeline_id": "pipeline-1",
                "duration_ms": 1000
            }
        });
        assert_eq!(get_event_type(&event), "PipelineCompleted");
    }

    #[test]
    fn test_get_event_type_metrics_snapshot() {
        let event = json!({
            "MetricsSnapshot": {
                "cpu_usage": 50.0,
                "memory_usage": 1024
            }
        });
        assert_eq!(get_event_type(&event), "MetricsSnapshot");
    }

    #[test]
    fn test_get_event_type_unknown() {
        let event = json!({
            "UnknownEvent": {
                "data": "test"
            }
        });
        assert_eq!(get_event_type(&event), "Unknown");
    }

    #[test]
    fn test_get_event_type_empty_object() {
        let event = json!({});
        assert_eq!(get_event_type(&event), "Unknown");
    }

    #[test]
    fn test_get_event_type_null_value() {
        let event = json!(null);
        assert_eq!(get_event_type(&event), "Unknown");
    }

    #[test]
    fn test_get_event_type_multiple_fields() {
        // When an event has multiple fields, the first matching one should be returned
        let event = json!({
            "JobStarted": {"job_id": "job-1"},
            "AgentStarted": {"agent_id": "agent-1"}
        });
        // JobStarted comes first in the EVENT_TYPES array
        assert_eq!(get_event_type(&event), "JobStarted");
    }

    #[test]
    fn test_get_event_type_nested_structure() {
        let event = json!({
            "JobCompleted": {
                "job_id": "test-123",
                "nested": {
                    "deep": {
                        "value": true
                    }
                }
            }
        });
        assert_eq!(get_event_type(&event), "JobCompleted");
    }

    #[test]
    fn test_event_matches_type() {
        let event = json!({
            "JobStarted": {"job_id": "test-123"}
        });
        assert!(event_matches_type(&event, "JobStarted"));
        assert!(!event_matches_type(&event, "JobCompleted"));
        assert!(!event_matches_type(&event, "Unknown"));
    }
}
