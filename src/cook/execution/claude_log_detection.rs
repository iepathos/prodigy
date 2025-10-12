//! Claude JSON log location detection
//!
//! Detects the location of Claude's JSON log file after command execution.
//! Claude automatically saves streaming JSON logs to ~/.claude/projects/.

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// Detects the location of Claude's JSON log file after command execution.
///
/// Claude automatically saves streaming JSON logs to ~/.claude/projects/.
/// This function attempts to detect the log file location using multiple strategies:
/// 1. Parse log location from Claude CLI output
/// 2. Search for recently created .jsonl files
///
/// # Arguments
/// * `project_path` - Working directory where Claude command was executed
/// * `cli_output` - Standard output from Claude CLI
/// * `execution_start` - Timestamp when command execution started
///
/// # Returns
/// * `Some(PathBuf)` - Location of JSON log file if detected
/// * `None` - If log location could not be determined
pub async fn detect_json_log_location(
    project_path: &Path,
    cli_output: &str,
    execution_start: SystemTime,
) -> Option<PathBuf> {
    // Try parsing CLI output first
    if let Some(path) = parse_log_location_from_output(cli_output) {
        if path.exists() {
            tracing::debug!("Found Claude JSON log via CLI output: {}", path.display());
            return Some(path);
        }
    }

    // Try searching for recent files
    if let Some(path) = find_recent_log(execution_start).await {
        tracing::debug!("Found Claude JSON log via file search: {}", path.display());
        return Some(path);
    }

    // Try inferring from project path (fallback)
    if let Some(path) = infer_log_location(project_path) {
        if path.exists() {
            tracing::debug!(
                "Found Claude JSON log via path inference: {}",
                path.display()
            );
            return Some(path);
        }
    }

    // Log detection failure
    tracing::debug!("Could not detect Claude JSON log location");
    None
}

/// Parse log location from Claude CLI output
///
/// Looks for patterns like:
/// - "Session log: /Users/glen/.claude/projects/.../session.jsonl"
/// - "Log saved to: /path/to/log.jsonl"
fn parse_log_location_from_output(output: &str) -> Option<PathBuf> {
    // Try different patterns that Claude might use
    let patterns = [
        "Session log: ",
        "Log saved to: ",
        "JSON log: ",
        "Saving to: ",
    ];

    for pattern in &patterns {
        if let Some(start) = output.find(pattern) {
            let path_start = start + pattern.len();
            // Extract path until end of line
            if let Some(end) = output[path_start..].find('\n') {
                let path_str = output[path_start..path_start + end].trim();
                return Some(PathBuf::from(path_str));
            } else {
                // Path goes to end of output
                let path_str = output[path_start..].trim();
                return Some(PathBuf::from(path_str));
            }
        }
    }

    None
}

/// Infer log location from project path
///
/// Claude creates project directories based on working directory
/// Format: ~/.claude/projects/{sanitized-project-path}/{session-id}.jsonl
fn infer_log_location(project_path: &Path) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let sanitized = sanitize_project_path(project_path);

    let projects_dir = PathBuf::from(home).join(".claude/projects").join(sanitized);

    // If the directory exists, find the most recent .jsonl file
    if projects_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
                .collect();

            // Sort by modification time (most recent first)
            files.sort_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH)
            });

            if let Some(most_recent) = files.last() {
                return Some(most_recent.path());
            }
        }
    }

    None
}

/// Sanitize project path for Claude's directory structure
///
/// Claude sanitizes paths by replacing '/' with '-'
pub fn sanitize_project_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "-")
        .trim_start_matches('-')
        .to_string()
}

/// Search for recently created .jsonl files in ~/.claude/projects/
///
/// Matches by modification time (within last N seconds of execution start)
async fn find_recent_log(since: SystemTime) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let projects_dir = PathBuf::from(home).join(".claude/projects");

    if !projects_dir.exists() {
        return None;
    }

    // Search for .jsonl files modified after execution start
    // Allow a small buffer (1 second before) to account for timing differences
    let search_start = since.checked_sub(std::time::Duration::from_secs(1))?;

    let mut candidates: Vec<_> = WalkDir::new(&projects_dir)
        .max_depth(2) // Project dir + log files
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            if modified >= search_start {
                Some((e.path().to_path_buf(), modified))
            } else {
                None
            }
        })
        .collect();

    // Sort by modification time (most recent first)
    candidates.sort_by_key(|(_, modified)| *modified);

    // Return the most recent file
    candidates.last().map(|(path, _)| path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_location_from_output() {
        let output = "Session log: /Users/test/.claude/projects/test/abc.jsonl\n";
        let result = parse_log_location_from_output(output);
        assert_eq!(
            result,
            Some(PathBuf::from("/Users/test/.claude/projects/test/abc.jsonl"))
        );
    }

    #[test]
    fn test_parse_log_location_no_newline() {
        let output = "Log saved to: /Users/test/log.jsonl";
        let result = parse_log_location_from_output(output);
        assert_eq!(result, Some(PathBuf::from("/Users/test/log.jsonl")));
    }

    #[test]
    fn test_parse_log_location_not_found() {
        let output = "Some other output without log location";
        let result = parse_log_location_from_output(output);
        assert_eq!(result, None);
    }

    #[test]
    fn test_sanitize_project_path() {
        assert_eq!(
            sanitize_project_path(&PathBuf::from("/Users/glen/prodigy")),
            "Users-glen-prodigy"
        );
    }

    #[test]
    fn test_sanitize_project_path_no_leading_slash() {
        assert_eq!(
            sanitize_project_path(&PathBuf::from("Users/glen/prodigy")),
            "Users-glen-prodigy"
        );
    }

    #[test]
    fn test_execution_result_with_json_log() {
        use crate::cook::execution::ExecutionResult;
        use std::collections::HashMap;

        let result = ExecutionResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        }
        .with_json_log_location(PathBuf::from("/test/log.jsonl"));

        assert_eq!(result.json_log_location(), Some("/test/log.jsonl"));
    }
}
