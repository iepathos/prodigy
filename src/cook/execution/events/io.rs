//! I/O operations for event storage
//!
//! This module provides async I/O functions for reading and writing event data:
//! - Reading events from JSONL files
//! - Saving indices to disk
//! - File system operations with proper error handling
//!
//! All functions in this module perform I/O operations and return Results.

use super::index::EventIndex;
use super::EventRecord;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

/// Save index to file
pub async fn save_index(index: &EventIndex, index_path: &Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create index directory: {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(index).context("Failed to serialize index to JSON")?;

    fs::write(index_path, json)
        .await
        .with_context(|| format!("Failed to write index to {}", index_path.display()))?;

    Ok(())
}

/// Read and parse events from a file with byte offsets
///
/// This is an I/O operation that reads a JSONL file and parses each line as an EventRecord.
/// Malformed lines are skipped with a debug log message.
///
/// # Arguments
/// * `file_path` - Path to the JSONL file to read
///
/// # Returns
/// A vector of tuples containing (EventRecord, byte_offset, line_number) for each valid event
///
/// # Errors
/// Returns an error if:
/// - The file cannot be opened
/// - There is an error reading from the file
pub async fn read_events_from_file_with_offsets(
    file_path: &PathBuf,
) -> Result<Vec<(EventRecord, u64, usize)>> {
    let file = File::open(file_path)
        .await
        .with_context(|| format!("Failed to open event file: {}", file_path.display()))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut line_number = 0;
    let mut byte_offset = 0u64;
    let mut events = Vec::new();

    while let Some(line) = lines.next_line().await? {
        line_number += 1;

        // Try to parse the event
        if let Ok(event) = serde_json::from_str::<EventRecord>(&line) {
            events.push((event, byte_offset, line_number));
        } else {
            // Log warning but continue processing
            debug!(
                "Skipping malformed event at {}:{}: {}",
                file_path.display(),
                line_number,
                &line[..line.len().min(100)]
            );
        }

        byte_offset += line.len() as u64 + 1; // +1 for newline
    }

    Ok(events)
}
