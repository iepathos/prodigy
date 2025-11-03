//! Job status analysis and event aggregation
//!
//! This module handles job status tracking, retention analysis, and event aggregation
//! operations across multiple event files and jobs.

use super::{format, io, transform};
use anyhow::Result;
use chrono::{DateTime, Local};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

// =============================================================================
// Job Status Functions
// =============================================================================

/// Get list of all available jobs from global storage
///
/// Scans the global events directory and collects status information
/// for all jobs found in the repository.
///
/// # Returns
/// A vector of JobInfo structs containing status details for each job
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::get_available_jobs;
///
/// let jobs = get_available_jobs()?;
/// for job in jobs {
///     println!("{}: {:?}", job.id, job.status);
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn get_available_jobs() -> Result<Vec<format::JobInfo>> {
    let current_dir = std::env::current_dir()?;
    let repo_name = crate::storage::extract_repo_name(&current_dir)?;
    let global_base = crate::storage::get_default_storage_dir()?;
    let global_events_dir = global_base.join("events").join(&repo_name);

    if !global_events_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&global_events_dir)?;

    let jobs: Vec<format::JobInfo> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|entry| {
            let job_id = entry.file_name().to_string_lossy().to_string();
            read_job_status(&global_events_dir.join(&job_id)).unwrap_or_else(|_| format::JobInfo {
                id: job_id.clone(),
                status: format::JobStatus::Unknown,
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
///
/// Processes all event files in a job directory to extract status information
/// including start/end times, success/failure counts, and current state.
///
/// # Arguments
/// * `job_events_dir` - Path to the job's events directory
///
/// # Returns
/// A JobInfo struct with the aggregated status information
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::read_job_status;
/// use std::path::Path;
///
/// let job_info = read_job_status(Path::new(".prodigy/events/mapreduce-123"))?;
/// println!("Job {} is {:?}", job_info.id, job_info.status);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn read_job_status(job_events_dir: &Path) -> Result<format::JobInfo> {
    let job_id = job_events_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut status = format::JobStatus::Unknown;
    let mut start_time = None;
    let mut end_time = None;
    let mut success_count = 0;
    let mut failure_count = 0;

    // Find and read event files
    let event_files = io::find_event_files(job_events_dir)?;

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

    Ok(format::JobInfo {
        id: job_id,
        status,
        start_time,
        end_time,
        success_count,
        failure_count,
    })
}

/// Process a single event to extract status information
///
/// Updates the provided status variables based on the event type and contents.
/// This is a pure function that performs no I/O.
///
/// # Arguments
/// * `event` - The event JSON to process
/// * `status` - Mutable reference to current job status
/// * `start_time` - Mutable reference to job start time
/// * `end_time` - Mutable reference to job end time
/// * `success_count` - Mutable reference to success counter
/// * `failure_count` - Mutable reference to failure counter
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::process_event_for_status;
/// use prodigy::cli::events::format::JobStatus;
/// use serde_json::json;
///
/// let event = json!({"JobStarted": {"job_id": "test"}});
/// let mut status = JobStatus::Unknown;
/// let mut start_time = None;
/// let mut end_time = None;
/// let mut success_count = 0;
/// let mut failure_count = 0;
///
/// process_event_for_status(
///     &event,
///     &mut status,
///     &mut start_time,
///     &mut end_time,
///     &mut success_count,
///     &mut failure_count,
/// );
/// ```
pub fn process_event_for_status(
    event: &Value,
    status: &mut format::JobStatus,
    start_time: &mut Option<DateTime<Local>>,
    end_time: &mut Option<DateTime<Local>>,
    success_count: &mut u64,
    failure_count: &mut u64,
) {
    if event.get("JobStarted").is_some() {
        *status = format::JobStatus::InProgress;
        if let Some(ts) = transform::extract_timestamp(event) {
            *start_time = Some(ts.with_timezone(&Local));
        }
    } else if let Some(completed) = event.get("JobCompleted") {
        *status = format::JobStatus::Completed;
        if let Some(ts) = transform::extract_timestamp(event) {
            *end_time = Some(ts.with_timezone(&Local));
        }
        if let Some(s) = completed.get("success_count").and_then(|v| v.as_u64()) {
            *success_count = s;
        }
        if let Some(f) = completed.get("failure_count").and_then(|v| v.as_u64()) {
            *failure_count = f;
        }
    } else if event.get("JobFailed").is_some() {
        *status = format::JobStatus::Failed;
        if let Some(ts) = transform::extract_timestamp(event) {
            *end_time = Some(ts.with_timezone(&Local));
        }
    }
}

// =============================================================================
// Event Aggregation Functions
// =============================================================================

/// Show aggregated statistics from all global event files
///
/// Reads events from all job directories in global storage and calculates
/// aggregated statistics grouped by the specified field.
///
/// # Arguments
/// * `group_by` - Field to group statistics by (e.g., "event_type", "job_id")
/// * `output_format` - Output format ("human", "json", "yaml", "table")
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::show_aggregated_stats;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// show_aggregated_stats("event_type".to_string(), "human".to_string()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn show_aggregated_stats(group_by: String, output_format: String) -> Result<()> {
    let event_files = io::get_all_event_files()?;

    if event_files.is_empty() {
        println!("No events found in global storage.");
        return Ok(());
    }

    // Read all events and calculate statistics using pure functions
    let all_events = io::read_events_from_files(&event_files)?;
    let (stats, total) = transform::calculate_event_statistics(all_events.into_iter(), &group_by);
    let sorted_stats = transform::sort_statistics_by_count(stats);

    // Display statistics using pure functions
    format::display_statistics_with_format(&sorted_stats, total, &group_by, &output_format, true)
}

/// Search aggregated events from all global event files
///
/// Reads events from all job directories and searches for matches using
/// the provided pattern and optional field filters.
///
/// # Arguments
/// * `pattern` - Search pattern (regex supported)
/// * `fields` - Optional list of fields to search within
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::search_aggregated_events;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// search_aggregated_events(
///     "error".to_string(),
///     Some(vec!["message".to_string()])
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn search_aggregated_events(pattern: String, fields: Option<Vec<String>>) -> Result<()> {
    let event_files = io::get_all_event_files()?;

    if event_files.is_empty() {
        println!("No events found in global storage.");
        return Ok(());
    }

    // Read all events and search using pure functions
    let all_events = io::read_events_from_files(&event_files)?;
    let matching_events =
        transform::search_events_with_pattern(&all_events, &pattern, fields.as_deref())?;

    // Display results
    format::display_search_results(&matching_events, true)
}

/// Export aggregated events from all global event files
///
/// Collects events from all job directories and exports them in the
/// specified format to stdout or a file.
///
/// # Arguments
/// * `format` - Export format ("json", "csv", "markdown")
/// * `output` - Optional output file path (stdout if None)
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::export_aggregated_events;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// export_aggregated_events(
///     "json".to_string(),
///     Some(PathBuf::from("all-events.json"))
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn export_aggregated_events(format: String, output: Option<PathBuf>) -> Result<()> {
    let event_files = io::get_all_event_files()?;

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
        "json" => format::export_as_json(&events)?,
        "csv" => format::export_as_csv(&events)?,
        "markdown" => format::export_as_markdown(&events)?,
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

// =============================================================================
// Retention Analysis Functions
// =============================================================================

/// Pure function: Determine if global storage should be analyzed
///
/// Returns true if either all_jobs flag is set or a specific job_id is provided.
///
/// # Arguments
/// * `all_jobs` - Whether to analyze all jobs
/// * `job_id` - Optional specific job ID to analyze
///
/// # Example
/// ```
/// use prodigy::cli::events::analysis::should_analyze_global_storage;
///
/// assert!(should_analyze_global_storage(true, None));
/// assert!(should_analyze_global_storage(false, Some("job-123")));
/// assert!(!should_analyze_global_storage(false, None));
/// ```
pub fn should_analyze_global_storage(all_jobs: bool, job_id: Option<&str>) -> bool {
    all_jobs || job_id.is_some()
}

/// Analyze retention targets and calculate what will be cleaned
///
/// Determines which events will be removed based on the retention policy,
/// analyzing either all jobs, a specific job, or local events.
///
/// # Arguments
/// * `all_jobs` - Whether to analyze all jobs in global storage
/// * `job_id` - Optional specific job ID to analyze
/// * `policy` - Retention policy to apply
///
/// # Returns
/// RetentionAnalysis struct with counts of events to remove/archive and space to save
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::analyze_retention_targets;
/// use prodigy::cook::execution::events::retention::RetentionPolicy;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// let policy = RetentionPolicy::default();
/// let analysis = analyze_retention_targets(true, None, &policy).await?;
/// println!("Will remove {} events", analysis.events_to_remove);
/// # Ok(())
/// # }
/// ```
pub async fn analyze_retention_targets(
    all_jobs: bool,
    job_id: Option<&str>,
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
) -> Result<crate::cook::execution::events::retention::RetentionAnalysis> {
    use crate::cook::execution::events::retention::{RetentionAnalysis, RetentionManager};

    let mut analysis_total = RetentionAnalysis::default();

    if should_analyze_global_storage(all_jobs, job_id) {
        let current_dir = std::env::current_dir()?;
        let repo_name = crate::storage::extract_repo_name(&current_dir)?;
        let global_events_dir = io::build_global_events_path(&repo_name)?;

        if global_events_dir.exists() {
            let job_dirs = get_job_directories(&global_events_dir, job_id)?;
            analysis_total = aggregate_job_retention(job_dirs, policy).await?;
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

/// Aggregate retention analysis across job directories
///
/// Processes each job directory's event files and aggregates the retention
/// analysis results (events to remove, space to save, events to archive).
///
/// # Arguments
/// * `job_dirs` - Vector of job directory paths to analyze
/// * `policy` - Retention policy to apply
///
/// # Returns
/// Aggregated RetentionAnalysis across all provided job directories
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::{aggregate_job_retention, get_job_directories};
/// use prodigy::cook::execution::events::retention::RetentionPolicy;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), anyhow::Error> {
/// let global_events_dir = Path::new(".prodigy/events");
/// let policy = RetentionPolicy::default();
/// let job_dirs = get_job_directories(&global_events_dir, None)?;
/// let analysis = aggregate_job_retention(job_dirs, &policy).await?;
/// # Ok(())
/// # }
/// ```
pub async fn aggregate_job_retention(
    job_dirs: Vec<PathBuf>,
    policy: &crate::cook::execution::events::retention::RetentionPolicy,
) -> Result<crate::cook::execution::events::retention::RetentionAnalysis> {
    use crate::cook::execution::events::retention::{RetentionAnalysis, RetentionManager};

    let mut analysis_total = RetentionAnalysis::default();

    for job_dir in job_dirs {
        let event_files = io::find_event_files(&job_dir)?;
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

    Ok(analysis_total)
}

/// Get job directories to process
///
/// Returns a list of job directories to process based on whether a specific
/// job ID is provided or all jobs should be processed.
///
/// # Arguments
/// * `global_events_dir` - Path to the global events directory
/// * `job_id` - Optional specific job ID to filter
///
/// # Returns
/// Vector of PathBuf for each job directory to process
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::analysis::get_job_directories;
/// use std::path::Path;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let global_events_dir = Path::new(".prodigy/events");
/// // Get all job directories
/// let all_jobs = get_job_directories(&global_events_dir, None)?;
///
/// // Get specific job directory
/// let specific = get_job_directories(&global_events_dir, Some("mapreduce-123"))?;
/// # Ok(())
/// # }
/// ```
pub fn get_job_directories(global_events_dir: &Path, job_id: Option<&str>) -> Result<Vec<PathBuf>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_analyze_global_storage_with_all_jobs() {
        assert!(should_analyze_global_storage(true, None));
    }

    #[test]
    fn test_should_analyze_global_storage_with_job_id() {
        assert!(should_analyze_global_storage(false, Some("job-123")));
    }

    #[test]
    fn test_should_analyze_global_storage_neither_flag() {
        assert!(!should_analyze_global_storage(false, None));
    }

    #[test]
    fn test_process_event_for_status_job_started() {
        use serde_json::json;

        let event = json!({
            "JobStarted": {
                "job_id": "test-123",
                "timestamp": "2024-01-01T00:00:00Z"
            }
        });

        let mut status = format::JobStatus::Unknown;
        let mut start_time = None;
        let mut end_time = None;
        let mut success_count = 0;
        let mut failure_count = 0;

        process_event_for_status(
            &event,
            &mut status,
            &mut start_time,
            &mut end_time,
            &mut success_count,
            &mut failure_count,
        );

        assert!(matches!(status, format::JobStatus::InProgress));
        assert!(start_time.is_some());
    }

    #[test]
    fn test_process_event_for_status_job_completed() {
        use serde_json::json;

        let event = json!({
            "JobCompleted": {
                "job_id": "test-123",
                "success_count": 5,
                "failure_count": 2,
                "timestamp": "2024-01-01T01:00:00Z"
            }
        });

        let mut status = format::JobStatus::InProgress;
        let mut start_time = None;
        let mut end_time = None;
        let mut success_count = 0;
        let mut failure_count = 0;

        process_event_for_status(
            &event,
            &mut status,
            &mut start_time,
            &mut end_time,
            &mut success_count,
            &mut failure_count,
        );

        assert!(matches!(status, format::JobStatus::Completed));
        assert!(end_time.is_some());
        assert_eq!(success_count, 5);
        assert_eq!(failure_count, 2);
    }

    #[test]
    fn test_process_event_for_status_job_failed() {
        use serde_json::json;

        let event = json!({
            "JobFailed": {
                "job_id": "test-123",
                "error": "Something went wrong",
                "timestamp": "2024-01-01T01:00:00Z"
            }
        });

        let mut status = format::JobStatus::InProgress;
        let mut start_time = None;
        let mut end_time = None;
        let mut success_count = 0;
        let mut failure_count = 0;

        process_event_for_status(
            &event,
            &mut status,
            &mut start_time,
            &mut end_time,
            &mut success_count,
            &mut failure_count,
        );

        assert!(matches!(status, format::JobStatus::Failed));
        assert!(end_time.is_some());
    }
}
