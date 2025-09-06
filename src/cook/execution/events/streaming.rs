//! Simplified real-time event streaming support

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Simple event streamer that watches a file for changes
pub struct SimpleEventStreamer {
    events_file: PathBuf,
}

impl SimpleEventStreamer {
    /// Create a new event streamer
    pub fn new(events_file: PathBuf) -> Self {
        Self { events_file }
    }

    /// Start streaming events with a callback
    pub async fn stream_events<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        let events_file = self.events_file.clone();

        // Track last position in file
        let mut last_pos = 0u64;

        // Set up file watcher
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;

        // Watch the events file or its parent directory
        let watch_path = if events_file.exists() {
            events_file.clone()
        } else {
            events_file
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid events file path"))?
                .to_path_buf()
        };

        watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

        // Process file changes
        loop {
            match rx.recv() {
                Ok(_) => {
                    // File changed, read new events
                    if events_file.exists() {
                        last_pos = self.read_new_events(last_pos, &callback).await?;
                    }
                }
                Err(e) => {
                    log::error!("File watcher error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Read new events from the file
    async fn read_new_events<F>(&self, last_pos: u64, callback: &F) -> Result<u64>
    where
        F: Fn(String),
    {
        use tokio::io::AsyncSeekExt;

        let mut file = fs::File::open(&self.events_file).await?;
        file.seek(tokio::io::SeekFrom::Start(last_pos)).await?;

        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut new_pos = last_pos;

        while reader.read_line(&mut line).await? > 0 {
            if !line.trim().is_empty() {
                callback(line.trim().to_string());
            }
            new_pos += line.len() as u64;
            line.clear();
        }

        Ok(new_pos)
    }
}
