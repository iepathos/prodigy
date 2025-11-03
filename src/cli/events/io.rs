//! File I/O operations for event data
//!
//! This module handles all file system operations for reading and writing event data,
//! including path resolution and file discovery.

use super::transform;
use super::EventFilter;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tracing::info;

// =============================================================================
// File Discovery
// =============================================================================

/// Find all JSONL event files in a directory
///
/// Returns a sorted list of .jsonl files found in the specified directory.
/// Returns an empty vector if the directory doesn't exist.
///
/// # Arguments
/// * `dir` - Directory to search for event files
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::find_event_files;
/// use std::path::Path;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let files = find_event_files(Path::new(".prodigy/events"))?;
/// # Ok(())
/// # }
/// ```
pub fn find_event_files(dir: &Path) -> Result<Vec<PathBuf>> {
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

/// Get all event files from global storage across all jobs
///
/// Searches the global events directory for all jobs in the current repository
/// and returns a sorted list of all event files found.
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::get_all_event_files;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let all_files = get_all_event_files()?;
/// println!("Found {} event files across all jobs", all_files.len());
/// # Ok(())
/// # }
/// ```
pub fn get_all_event_files() -> Result<Vec<PathBuf>> {
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
// Path Resolution
// =============================================================================

/// Resolve event file path for a specific job
///
/// Finds the most recent event file for a given job ID in global storage.
///
/// # Arguments
/// * `job_id` - The job ID to resolve
///
/// # Errors
/// Returns an error if the job doesn't exist or has no event files.
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::resolve_job_event_file;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let file = resolve_job_event_file("mapreduce-123")?;
/// # Ok(())
/// # }
/// ```
pub fn resolve_job_event_file(job_id: &str) -> Result<PathBuf> {
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

/// Resolve event file path with fallback to job lookup
///
/// First checks if the provided file exists. If not, attempts to resolve
/// the file from global storage using the job_id if provided.
///
/// # Arguments
/// * `file` - The file path to resolve
/// * `job_id` - Optional job ID for fallback resolution
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::resolve_event_file_with_fallback;
/// use std::path::PathBuf;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let file = resolve_event_file_with_fallback(
///     PathBuf::from(".prodigy/events/events.jsonl"),
///     Some("mapreduce-123")
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn resolve_event_file_with_fallback(file: PathBuf, job_id: Option<&str>) -> Result<PathBuf> {
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

/// Build global events directory path for a repository
///
/// Pure function that constructs the global events path from a repository name.
///
/// # Arguments
/// * `repo_name` - Name of the repository
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::build_global_events_path;
/// use std::path::PathBuf;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let path = build_global_events_path("my-repo")?;
/// assert!(path.ends_with("events/my-repo"));
/// # Ok(())
/// # }
/// ```
pub fn build_global_events_path(repo_name: &str) -> Result<PathBuf> {
    let global_base = crate::storage::get_default_storage_dir()?;
    Ok(global_base.join("events").join(repo_name))
}

/// Determine the path to watch for file changes
///
/// Returns the file path if it exists, otherwise returns the parent directory
/// to watch for the file to be created.
///
/// # Arguments
/// * `file` - The target file path
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::determine_watch_path;
/// use std::path::Path;
///
/// let watch_path = determine_watch_path(Path::new(".prodigy/events/events.jsonl"));
/// ```
pub fn determine_watch_path(file: &Path) -> PathBuf {
    if file.exists() {
        file.to_path_buf()
    } else {
        file.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| file.to_path_buf())
    }
}

// =============================================================================
// Event Reading
// =============================================================================

/// Read and parse events from multiple files
///
/// Reads all events from the provided files and parses them into JSON values.
/// Skips lines that fail to parse.
///
/// # Arguments
/// * `event_files` - Slice of file paths to read
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::read_events_from_files;
/// use std::path::PathBuf;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let files = vec![PathBuf::from("events1.jsonl"), PathBuf::from("events2.jsonl")];
/// let events = read_events_from_files(&files)?;
/// # Ok(())
/// # }
/// ```
pub fn read_events_from_files(event_files: &[PathBuf]) -> Result<Vec<Value>> {
    let mut all_events = Vec::new();

    for file in event_files {
        let content = fs::File::open(file)?;
        let reader = BufReader::new(content);

        for line in reader.lines() {
            let line = line?;
            if let Some(event) = transform::parse_event_line(&line) {
                all_events.push(event);
            }
        }
    }

    Ok(all_events)
}

/// Read and parse events from a single file
///
/// Reads all events from the provided file and parses them into JSON values.
/// Skips lines that fail to parse.
///
/// # Arguments
/// * `file` - Path to the file to read
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::read_events_from_single_file;
/// use std::path::PathBuf;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let events = read_events_from_single_file(&PathBuf::from("events.jsonl"))?;
/// # Ok(())
/// # }
/// ```
pub fn read_events_from_single_file(file: &PathBuf) -> Result<Vec<Value>> {
    let file_handle = fs::File::open(file)?;
    let reader = BufReader::new(file_handle);

    let events = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| transform::parse_event_line(&line))
        .collect();

    Ok(events)
}

/// Read events from a file and apply filtering
///
/// Reads events from the specified file, applies the provided filter,
/// and limits the results to the specified count.
///
/// # Arguments
/// * `file` - Path to the file to read
/// * `filter` - Filter to apply to events
/// * `limit` - Maximum number of events to return
///
/// # Example
/// ```no_run
/// use prodigy::cli::events::io::read_and_filter_events;
/// use prodigy::cli::events::EventFilter;
/// use std::path::PathBuf;
///
/// # fn example() -> Result<(), anyhow::Error> {
/// let file = PathBuf::from("events.jsonl");
/// let filter = EventFilter::new(
///     Some("mapreduce-123".to_string()),
///     Some("AgentCompleted".to_string()),
///     None,
///     None
/// );
/// let events = read_and_filter_events(&file, &filter, 100)?;
/// # Ok(())
/// # }
/// ```
pub fn read_and_filter_events(
    file: &PathBuf,
    filter: &EventFilter,
    limit: usize,
) -> Result<Vec<Value>> {
    let file_handle = fs::File::open(file)?;
    let reader = BufReader::new(file_handle);

    let events = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| transform::parse_event_line(&line))
        .filter(|event| filter.matches_event(event))
        .take(limit)
        .collect();

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_find_event_files_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let files = find_event_files(temp_dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_find_event_files_with_jsonl() {
        let temp_dir = TempDir::new().unwrap();

        // Create some test files
        fs::write(temp_dir.path().join("events1.jsonl"), "{}").unwrap();
        fs::write(temp_dir.path().join("events2.jsonl"), "{}").unwrap();
        fs::write(temp_dir.path().join("not-events.txt"), "{}").unwrap();

        let files = find_event_files(temp_dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("events1.jsonl"));
        assert!(files[1].ends_with("events2.jsonl"));
    }

    #[test]
    fn test_find_event_files_nonexistent_dir() {
        let files = find_event_files(Path::new("/nonexistent/path")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_determine_watch_path_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");
        fs::write(&file_path, "{}").unwrap();

        let watch_path = determine_watch_path(&file_path);
        assert_eq!(watch_path, file_path);
    }

    #[test]
    fn test_determine_watch_path_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.jsonl");

        let watch_path = determine_watch_path(&file_path);
        assert_eq!(watch_path, temp_dir.path());
    }

    #[test]
    fn test_read_events_from_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("events.jsonl");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, r#"{{"type":"test1"}}"#).unwrap();
        writeln!(file, r#"{{"type":"test2"}}"#).unwrap();

        let events = read_events_from_single_file(&file_path).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_read_events_from_files() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("events1.jsonl");
        let file2 = temp_dir.path().join("events2.jsonl");

        fs::write(&file1, r#"{"type":"test1"}"#).unwrap();
        fs::write(&file2, r#"{"type":"test2"}"#).unwrap();

        let events = read_events_from_files(&[file1, file2]).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_build_global_events_path() {
        let path = build_global_events_path("test-repo").unwrap();
        assert!(path.ends_with("events/test-repo"));
    }
}
