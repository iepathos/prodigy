use super::CommandOutput;
use crate::{Error, Result};
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

pub struct CommandHistory {
    memory: VecDeque<HistoryEntry>,
    max_memory_size: usize,
    history_file: Option<PathBuf>,
}

#[derive(Clone)]
struct HistoryEntry {
    command: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    success: Option<bool>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            memory: VecDeque::new(),
            max_memory_size: 1000,
            history_file: None,
        }
    }

    pub async fn with_file(history_file: PathBuf) -> Result<Self> {
        let mut history = Self {
            memory: VecDeque::new(),
            max_memory_size: 1000,
            history_file: Some(history_file.clone()),
        };

        history.load_from_file().await?;
        Ok(history)
    }

    pub async fn add_command(&mut self, command: String) -> Result<()> {
        let entry = HistoryEntry {
            command: command.clone(),
            timestamp: chrono::Utc::now(),
            success: None,
        };

        self.memory.push_back(entry);

        if self.memory.len() > self.max_memory_size {
            self.memory.pop_front();
        }

        if let Some(file) = &self.history_file {
            self.append_to_file(&command).await?;
        }

        Ok(())
    }

    pub async fn add_result(&mut self, command: String, output: &CommandOutput) -> Result<()> {
        if let Some(entry) = self.memory.iter_mut().rev().find(|e| e.command == command) {
            entry.success = Some(output.success);
        }
        Ok(())
    }

    pub async fn get_command(&self, index: usize) -> Result<Option<String>> {
        Ok(self.memory.get(index).map(|e| e.command.clone()))
    }

    pub async fn get_recent(&self, limit: usize) -> Result<Vec<String>> {
        let commands: Vec<String> = self
            .memory
            .iter()
            .rev()
            .take(limit)
            .map(|e| e.command.clone())
            .collect();

        Ok(commands)
    }

    pub async fn search(&self, pattern: &str) -> Result<Vec<String>> {
        let regex = regex::Regex::new(pattern)
            .map_err(|e| Error::Command(format!("Invalid search pattern: {}", e)))?;

        let matches: Vec<String> = self
            .memory
            .iter()
            .filter(|e| regex.is_match(&e.command))
            .map(|e| e.command.clone())
            .collect();

        Ok(matches)
    }

    async fn load_from_file(&mut self) -> Result<()> {
        if let Some(file) = &self.history_file {
            if file.exists() {
                let file_handle = fs::File::open(file).await?;
                let reader = tokio::io::BufReader::new(file_handle);
                let mut lines = reader.lines();

                while let Some(line) = lines.next_line().await? {
                    if !line.trim().is_empty() {
                        self.memory.push_back(HistoryEntry {
                            command: line,
                            timestamp: chrono::Utc::now(),
                            success: None,
                        });

                        if self.memory.len() > self.max_memory_size {
                            self.memory.pop_front();
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn append_to_file(&self, command: &str) -> Result<()> {
        if let Some(file) = &self.history_file {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).await?;
            }

            let mut file_handle = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file)
                .await?;

            file_handle
                .write_all(format!("{}\n", command).as_bytes())
                .await?;
            file_handle.flush().await?;
        }

        Ok(())
    }

    pub fn clear(&mut self) {
        self.memory.clear();
    }
}
