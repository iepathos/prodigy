//! Claude JSON logs command implementation
//!
//! Provides functionality to view, search, and analyze Claude JSON logs.

use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

/// Run the logs command
pub async fn run_logs_command(
    session_id: Option<String>,
    latest: bool,
    tail: bool,
    summary: bool,
) -> Result<()> {
    let log_dir = get_claude_log_dir()?;

    if latest {
        handle_latest_log(&log_dir, tail, summary)?;
    } else if let Some(sid) = session_id {
        handle_specific_session(&log_dir, &sid, tail, summary)?;
    } else {
        list_recent_logs(&log_dir)?;
    }

    Ok(())
}

/// Get the Claude log directory
fn get_claude_log_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".claude/projects"))
}

/// Handle viewing the latest log
fn handle_latest_log(log_dir: &Path, tail: bool, summary: bool) -> Result<()> {
    let latest_log = find_latest_log(log_dir)?;

    println!("📋 Latest Claude log: {}", latest_log.display());

    if tail {
        tail_file(&latest_log)?;
    } else if summary {
        display_log_summary(&latest_log)?;
    } else {
        println!("\nUse:");
        println!("  cat {}  # View complete log", latest_log.display());
        println!(
            "  tail -f {}  # Watch live (if in progress)",
            latest_log.display()
        );
        println!("  cat {} | jq  # Pretty-print JSON", latest_log.display());
    }

    Ok(())
}

/// Handle viewing a specific session log
fn handle_specific_session(
    log_dir: &Path,
    session_id: &str,
    tail: bool,
    summary: bool,
) -> Result<()> {
    // Try to find a log file matching the session ID
    let log_file = find_log_for_session(log_dir, session_id)?;

    println!("📋 Claude log: {}", log_file.display());

    if tail {
        tail_file(&log_file)?;
    } else if summary {
        display_log_summary(&log_file)?;
    } else {
        println!("\nUse:");
        println!("  cat {}  # View complete log", log_file.display());
        println!("  cat {} | jq  # Pretty-print JSON", log_file.display());
    }

    Ok(())
}

/// Find the most recent Claude log file
fn find_latest_log(log_dir: &Path) -> Result<PathBuf> {
    if !log_dir.exists() {
        anyhow::bail!(
            "Claude log directory not found: {}\nNo Claude commands have been executed yet.",
            log_dir.display()
        );
    }

    // Recursively search for .jsonl files in the log directory
    let mut log_files = Vec::new();
    collect_log_files(log_dir, &mut log_files)?;

    if log_files.is_empty() {
        anyhow::bail!(
            "No Claude logs found in {}\nRun a Claude command first to generate logs.",
            log_dir.display()
        );
    }

    // Sort by modification time
    log_files.sort_by_key(|(_, modified)| *modified);

    log_files
        .last()
        .map(|(path, _)| path.clone())
        .context("No log files found")
}

/// Recursively collect all .jsonl log files
/// Returns (PathBuf, SystemTime) tuples to avoid holding DirEntry file descriptors
fn collect_log_files(dir: &Path, files: &mut Vec<(PathBuf, SystemTime)>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    // Collect all entries first, then process them
    // This ensures read_dir's file descriptor is closed before recursing
    let entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;

    for entry in entries {
        let path = entry.path();

        if path.is_dir() {
            // Recursively search subdirectories
            collect_log_files(&path, files)?;
        } else if is_log_file(&path) {
            // Extract metadata immediately and drop the DirEntry
            // This prevents accumulating thousands of open file descriptors
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            files.push((path, modified));
        }
    }

    Ok(())
}

/// Check if a file is a Claude log file (.jsonl or .json)
fn is_log_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "jsonl" || ext == "json")
}

/// Find log file for a specific session ID
fn find_log_for_session(log_dir: &Path, session_id: &str) -> Result<PathBuf> {
    let mut log_files = Vec::new();
    collect_log_files(log_dir, &mut log_files)?;

    // Look for a file containing the session ID
    for (path, _modified) in log_files {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if filename.contains(session_id) {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "No log file found for session: {}\nAvailable sessions can be listed with: prodigy logs",
        session_id
    )
}

/// List recent Claude logs
fn list_recent_logs(log_dir: &Path) -> Result<()> {
    if !log_dir.exists() {
        println!("No Claude logs found.");
        println!("Claude log directory: {}", log_dir.display());
        println!("\nRun a Claude command to generate logs.");
        return Ok(());
    }

    let mut log_files = Vec::new();
    collect_log_files(log_dir, &mut log_files)?;

    if log_files.is_empty() {
        println!("No Claude logs found in {}", log_dir.display());
        return Ok(());
    }

    // Sort by modification time (most recent first)
    log_files.sort_by_key(|(_, modified)| *modified);
    log_files.reverse();

    println!("Recent Claude logs (showing up to 20 most recent):\n");

    for (i, (path, modified)) in log_files.iter().take(20).enumerate() {
        // Get file size - reopen file only when needed for display
        let size = fs::metadata(path).ok().map(|m| m.len()).unwrap_or(0);
        let size_kb = size / 1024;

        let modified_str = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|d| {
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        println!(
            "{:2}. {} ({} KB) - {}",
            i + 1,
            path.display(),
            size_kb,
            modified_str
        );
    }

    println!("\nUse 'prodigy logs --latest' to view the most recent log");
    println!("Use 'prodigy logs --latest --tail' to follow the latest log");

    Ok(())
}

/// Display a summary of a log file
fn display_log_summary(log_path: &Path) -> Result<()> {
    let is_jsonl = log_path.extension().and_then(|ext| ext.to_str()) == Some("jsonl");

    if is_jsonl {
        display_jsonl_summary(log_path)?;
    } else {
        display_json_summary(log_path)?;
    }

    Ok(())
}

/// Display summary for JSONL format (streaming logs)
fn display_jsonl_summary(log_path: &Path) -> Result<()> {
    let file = fs::File::open(log_path)?;
    let reader = BufReader::new(file);

    let mut message_count = 0;
    let mut tool_use_count = 0;
    let mut total_tokens = None;

    for line in reader.lines() {
        let line = line?;
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
            // Count messages
            if let Some(msg_type) = obj.get("type").and_then(|v| v.as_str()) {
                if msg_type == "user" || msg_type == "assistant" {
                    message_count += 1;
                }
            }

            // Extract token usage
            if let Some(usage) = obj.get("usage") {
                if let Some(total) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
                    total_tokens = Some(total);
                }
            }

            // Count tool uses
            if let Some(content) = obj.get("content").and_then(|v| v.as_array()) {
                for item in content {
                    if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        tool_use_count += 1;
                    }
                }
            }
        }
    }

    println!("\nLog Summary:");
    println!("  Format: JSONL (streaming)");
    println!("  Messages: {}", message_count);
    println!("  Tool uses: {}", tool_use_count);
    if let Some(tokens) = total_tokens {
        println!("  Tokens: {}", tokens);
    }

    Ok(())
}

/// Display summary for JSON format (legacy logs)
fn display_json_summary(log_path: &Path) -> Result<()> {
    let content = fs::read_to_string(log_path)?;
    let log: serde_json::Value = serde_json::from_str(&content)?;

    println!("\nLog Summary:");
    println!("  Format: JSON (legacy)");

    if let Some(messages) = log.get("messages").and_then(|v| v.as_array()) {
        println!("  Messages: {}", messages.len());
    }

    if let Some(usage) = log.get("usage").and_then(|v| v.as_object()) {
        if let Some(total) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
            println!("  Tokens: {}", total);
        }
    }

    Ok(())
}

/// Tail a log file (follow mode)
fn tail_file(log_path: &Path) -> Result<()> {
    println!("\nFollowing log file (Ctrl+C to exit)...\n");

    // Use system `tail -f` command for following
    let status = Command::new("tail")
        .arg("-f")
        .arg(log_path)
        .status()
        .context("Failed to execute tail command")?;

    if !status.success() {
        anyhow::bail!("tail command failed");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_log_file() {
        assert!(is_log_file(Path::new("session-abc.jsonl")));
        assert!(is_log_file(Path::new("session-abc.json")));
        assert!(!is_log_file(Path::new("session-abc.txt")));
        assert!(!is_log_file(Path::new("README.md")));
    }

    #[test]
    fn test_get_claude_log_dir() {
        let dir = get_claude_log_dir();
        assert!(dir.is_ok());
        let dir_path = dir.unwrap();
        assert!(dir_path.to_string_lossy().contains(".claude/projects"));
    }
}
