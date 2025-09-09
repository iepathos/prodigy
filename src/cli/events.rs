//! CLI commands for viewing and searching MapReduce events

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use clap::{Args, Subcommand};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Event viewer commands
#[derive(Debug, Args)]
pub struct EventsArgs {
    #[command(subcommand)]
    pub command: EventsCommand,
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
    /// List all events
    List {
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
    },

    /// Show event statistics
    Stats {
        /// Path to events file
        #[arg(long, default_value = ".prodigy/events/mapreduce_events.jsonl")]
        file: PathBuf,

        /// Group statistics by field (job_id, event_type, agent_id)
        #[arg(long, default_value = "event_type")]
        group_by: String,
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
}

/// Resolve event file path, checking both global and local storage
async fn resolve_event_path(mut file: PathBuf, job_id: Option<&str>) -> Result<PathBuf> {
    // If the provided path exists, use it
    if file.exists() {
        return Ok(file);
    }

    // Check if we should look in global storage
    if crate::storage::GlobalStorage::should_use_global() {
        // Try to determine repository path from current directory
        let current_dir = std::env::current_dir()?;

        // Extract repository name
        if let Ok(repo_name) = crate::storage::extract_repo_name(&current_dir) {
            let global_base = crate::storage::get_global_base_dir()?;

            // If a job_id is provided, look for that specific job's events
            if let Some(job_id) = job_id {
                let global_events_dir = global_base.join("events").join(&repo_name).join(job_id);

                if global_events_dir.exists() {
                    // Find the most recent event file in the directory
                    if let Ok(entries) = fs::read_dir(&global_events_dir) {
                        let mut event_files: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                e.path()
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .map(|ext| ext == "jsonl")
                                    .unwrap_or(false)
                            })
                            .collect();

                        // Sort by modification time (most recent first)
                        event_files.sort_by_key(|e| {
                            e.metadata()
                                .and_then(|m| m.modified())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        });
                        event_files.reverse();

                        if let Some(latest) = event_files.first() {
                            file = latest.path();
                            info!("Using global event file: {:?}", file);
                        }
                    }
                }
            } else {
                // Look for any events in the global repository directory
                let global_events_dir = global_base.join("events").join(&repo_name);

                if global_events_dir.exists() {
                    warn!("Global events directory exists at {:?}, but no job_id specified. Use --job-id to specify.", global_events_dir);
                }
            }
        }
    }

    // If file still doesn't exist, return the original path (will fail gracefully later)
    Ok(file)
}

/// Execute event viewer commands
pub async fn execute(args: EventsArgs) -> Result<()> {
    match args.command {
        EventsCommand::List {
            job_id,
            event_type,
            agent_id,
            since,
            limit,
            file,
        } => {
            let file = resolve_event_path(file, job_id.as_deref()).await?;
            list_events(file, job_id, event_type, agent_id, since, limit).await
        }

        EventsCommand::Stats { file, group_by } => {
            let file = resolve_event_path(file, None).await?;
            show_stats(file, group_by).await
        }

        EventsCommand::Search {
            pattern,
            file,
            fields,
        } => {
            let file = resolve_event_path(file, None).await?;
            search_events(file, pattern, fields).await
        }

        EventsCommand::Follow {
            file,
            job_id,
            event_type,
        } => {
            let file = resolve_event_path(file, job_id.as_deref()).await?;
            follow_events(file, job_id, event_type).await
        }

        EventsCommand::Export {
            file,
            format,
            output,
        } => {
            let file = resolve_event_path(file, None).await?;
            export_events(file, format, output).await
        }
    }
}

/// List events with optional filters
async fn list_events(
    file: PathBuf,
    job_id: Option<String>,
    event_type: Option<String>,
    agent_id: Option<String>,
    since: Option<u64>,
    limit: usize,
) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    let since_time = since.map(|minutes| Utc::now() - chrono::Duration::minutes(minutes as i64));

    let file = fs::File::open(file)?;
    let reader = BufReader::new(file);
    let mut count = 0;

    for line in reader.lines() {
        if count >= limit {
            break;
        }

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

        if let Some(ref aid) = agent_id {
            if !event_matches_field(&event, "agent_id", aid) {
                continue;
            }
        }

        if let Some(since_time) = since_time {
            if !event_is_recent(&event, since_time) {
                continue;
            }
        }

        // Format and display event
        display_event(&event);
        count += 1;
    }

    println!("\nDisplayed {} events", count);
    Ok(())
}

/// Show event statistics
async fn show_stats(file: PathBuf, group_by: String) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    use std::collections::HashMap;

    let file = fs::File::open(file)?;
    let reader = BufReader::new(file);
    let mut stats: HashMap<String, usize> = HashMap::new();
    let mut total = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: Value = serde_json::from_str(&line)?;
        total += 1;

        // Group by specified field
        let key = match group_by.as_str() {
            "event_type" => get_event_type(&event),
            "job_id" => event
                .get("job_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            "agent_id" => event
                .get("agent_id")
                .and_then(|v| v.as_str())
                .unwrap_or("n/a")
                .to_string(),
            _ => "unknown".to_string(),
        };

        *stats.entry(key).or_insert(0) += 1;
    }

    // Display statistics
    println!("Event Statistics (grouped by {})", group_by);
    println!("{}", "=".repeat(50));

    let mut sorted_stats: Vec<_> = stats.iter().collect();
    sorted_stats.sort_by(|a, b| b.1.cmp(a.1));

    for (key, count) in sorted_stats {
        let percentage = (*count as f64 / total as f64) * 100.0;
        println!("{:<30} {:>6} ({:>5.1}%)", key, count, percentage);
    }

    println!("{}", "=".repeat(50));
    println!("Total events: {}", total);

    Ok(())
}

/// Search events by pattern
async fn search_events(file: PathBuf, pattern: String, fields: Option<Vec<String>>) -> Result<()> {
    if !file.exists() {
        println!("No events found. Events file does not exist: {:?}", file);
        return Ok(());
    }

    use regex::Regex;
    let re = Regex::new(&pattern)?;

    let file = fs::File::open(file)?;
    let reader = BufReader::new(file);
    let mut count = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: Value = serde_json::from_str(&line)?;

        // Search in specified fields or all fields
        let matches = if let Some(ref fields) = fields {
            fields.iter().any(|field| {
                event
                    .get(field)
                    .and_then(|v| v.as_str())
                    .map(|s| re.is_match(s))
                    .unwrap_or(false)
            })
        } else {
            // Search in all string values
            search_in_value(&event, &re)
        };

        if matches {
            display_event(&event);
            count += 1;
        }
    }

    println!("\nFound {} matching events", count);
    Ok(())
}

/// Follow events in real-time
async fn follow_events(
    file: PathBuf,
    job_id: Option<String>,
    event_type: Option<String>,
) -> Result<()> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    if !file.exists() {
        println!("Waiting for events file to be created: {:?}", file);
        // Create parent directory if it doesn't exist
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    // Watch the events file
    let watch_path = if file.exists() {
        file.clone()
    } else {
        file.parent().unwrap().to_path_buf()
    };
    watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

    println!("Following events (Ctrl+C to stop)...\n");

    // Read existing content first
    if file.exists() {
        let mut last_pos = display_existing_events(&file, &job_id, &event_type)?;

        // Watch for new events
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(_) => {
                    // File changed, read new events
                    last_pos = display_new_events(&file, last_pos, &job_id, &event_type)?;
                }
                Err(_) => {
                    // Timeout, continue waiting
                    continue;
                }
            }
        }
    } else {
        // Wait for file to be created
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(_) => {
                    if file.exists() {
                        let _ = display_existing_events(&file, &job_id, &event_type)?;
                        break;
                    }
                }
                Err(_) => continue,
            }
        }
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

// Helper functions

fn event_matches_field(event: &Value, field: &str, value: &str) -> bool {
    event
        .get(field)
        .and_then(|v| v.as_str())
        .map(|s| s == value)
        .unwrap_or(false)
}

fn event_matches_type(event: &Value, event_type: &str) -> bool {
    get_event_type(event) == event_type
}

fn get_event_type(event: &Value) -> String {
    // Extract event type from the event structure
    if event.get("JobStarted").is_some() {
        "JobStarted".to_string()
    } else if event.get("JobCompleted").is_some() {
        "JobCompleted".to_string()
    } else if event.get("JobFailed").is_some() {
        "JobFailed".to_string()
    } else if event.get("JobPaused").is_some() {
        "JobPaused".to_string()
    } else if event.get("JobResumed").is_some() {
        "JobResumed".to_string()
    } else if event.get("AgentStarted").is_some() {
        "AgentStarted".to_string()
    } else if event.get("AgentProgress").is_some() {
        "AgentProgress".to_string()
    } else if event.get("AgentCompleted").is_some() {
        "AgentCompleted".to_string()
    } else if event.get("AgentFailed").is_some() {
        "AgentFailed".to_string()
    } else if event.get("PipelineStarted").is_some() {
        "PipelineStarted".to_string()
    } else if event.get("PipelineStageCompleted").is_some() {
        "PipelineStageCompleted".to_string()
    } else if event.get("PipelineCompleted").is_some() {
        "PipelineCompleted".to_string()
    } else if event.get("MetricsSnapshot").is_some() {
        "MetricsSnapshot".to_string()
    } else {
        "Unknown".to_string()
    }
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

fn display_event(event: &Value) {
    let event_type = get_event_type(event);
    let timestamp = extract_timestamp(event);
    let job_id = event
        .get("job_id")
        .or_else(|| extract_nested_field(event, "job_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("n/a");

    // Format timestamp for display
    let time_str = if let Some(ts) = timestamp {
        ts.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    } else {
        "n/a".to_string()
    };

    // Display based on event type
    match event_type.as_str() {
        "JobStarted" => {
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
                println!(
                    "[{}] {} - Job: {} - Success: {}, Failures: {}",
                    time_str, event_type, job_id, success, failure
                );
            }
        }
        "AgentProgress" => {
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
        _ => {
            // Generic display for other event types
            println!(
                "[{}] {} - Job: {} - {}",
                time_str,
                event_type,
                job_id,
                serde_json::to_string(event).unwrap_or_default()
            );
        }
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
