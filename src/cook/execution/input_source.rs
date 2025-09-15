//! Input source handling for MapReduce workflows
//!
//! Supports both command execution and JSON file input sources.

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::subprocess::SubprocessManager;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info};

/// Type of input source
#[derive(Debug, Clone)]
pub enum InputSource {
    /// Shell command to execute
    Command(String),
    /// Path to JSON file
    JsonFile(String),
}

impl InputSource {
    /// Detect the input source type from a string
    ///
    /// If the input is a path to an existing .json file, it's treated as a JSON file.
    /// Otherwise, it's treated as a command to execute.
    pub fn detect(input: &str) -> Self {
        Self::detect_with_base(input, Path::new("."))
    }

    /// Detect the input source type from a string with a base path for resolution
    ///
    /// If the input is a path to an existing .json file, it's treated as a JSON file.
    /// Otherwise, it's treated as a command to execute.
    pub fn detect_with_base(input: &str, base_path: &Path) -> Self {
        let path = Path::new(input);

        // Resolve the path relative to the base if it's not absolute
        let resolved_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_path.join(path)
        };

        debug!("Checking for file at: {}", resolved_path.display());

        // Check if it's an existing JSON file
        if resolved_path.exists() && resolved_path.extension().and_then(|s| s.to_str()) == Some("json") {
            debug!("Detected JSON file input: {}", input);
            InputSource::JsonFile(input.to_string())
        } else if resolved_path.exists() && resolved_path.is_file() {
            // If it's another type of existing file, still treat as JSON file
            // This allows for flexibility in file naming
            debug!("Detected file input (non-.json extension): {}", input);
            InputSource::JsonFile(input.to_string())
        } else {
            // Treat as command to execute
            debug!("Detected command input: {}", input);
            InputSource::Command(input.to_string())
        }
    }

    /// Execute a command and return work items from its output
    pub async fn execute_command(
        command: &str,
        timeout: Duration,
        subprocess_manager: &SubprocessManager,
    ) -> MapReduceResult<Vec<Value>> {
        info!("Executing command for work items: {}", command);

        // Use subprocess manager for secure execution
        let output = subprocess_manager
            .run_with_timeout(command, timeout)
            .await
            .map_err(|e| MapReduceError::CommandExecutionFailed {
                command: command.to_string(),
                reason: format!("Command execution failed: {}", e),
                source: None, // ProcessError doesn't implement std::error::Error
            })?;

        if !output.status.success() {
            return Err(MapReduceError::CommandExecutionFailed {
                command: command.to_string(),
                reason: format!(
                    "Command exited with non-zero status: {}. Stderr: {}",
                    output.status.code().unwrap_or(-1),
                    output.stderr
                ),
                source: None,
            });
        }

        // Parse each line of output as a work item
        let items: Vec<Value> = output
            .stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                // Each line becomes a work item with the line as the "item" field
                serde_json::json!({
                    "item": line.trim()
                })
            })
            .collect();

        info!("Command produced {} work items", items.len());

        Ok(items)
    }

    /// Load work items from a JSON file
    pub async fn load_json_file(path: &str, project_root: &Path) -> MapReduceResult<Value> {
        let file_path = if Path::new(path).is_absolute() {
            Path::new(path).to_path_buf()
        } else {
            project_root.join(path)
        };

        debug!("Loading JSON from file: {}", file_path.display());

        // Check if file exists
        if !file_path.exists() {
            return Err(MapReduceError::WorkItemLoadFailed {
                path: file_path.clone(),
                reason: "File does not exist".to_string(),
                source: None,
            });
        }

        // Read and parse the JSON file
        let content = tokio::fs::read_to_string(&file_path).await.map_err(|e| {
            MapReduceError::WorkItemLoadFailed {
                path: file_path.clone(),
                reason: format!("Failed to read file: {}", e),
                source: Some(Box::new(e)),
            }
        })?;

        let json: Value =
            serde_json::from_str(&content).map_err(|e| MapReduceError::WorkItemLoadFailed {
                path: file_path.clone(),
                reason: "Failed to parse JSON".to_string(),
                source: Some(Box::new(e)),
            })?;

        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_json_file() {
        let temp_dir = TempDir::new().unwrap();
        let json_path = temp_dir.path().join("test.json");
        fs::write(&json_path, "{}").unwrap();

        let source = InputSource::detect(json_path.to_str().unwrap());
        match source {
            InputSource::JsonFile(_) => {} // Expected
            _ => panic!("Expected JSON file detection"),
        }
    }

    #[test]
    fn test_detect_command() {
        let source = InputSource::detect("ls -la");
        match source {
            InputSource::Command(_) => {} // Expected
            _ => panic!("Expected command detection"),
        }
    }

    #[test]
    fn test_detect_complex_command() {
        let source = InputSource::detect("find . -name '*.rs' | grep test");
        match source {
            InputSource::Command(_) => {} // Expected
            _ => panic!("Expected command detection"),
        }
    }

    #[tokio::test]
    async fn test_execute_command() {
        use crate::subprocess::SubprocessManager;

        let subprocess_manager = SubprocessManager::production();
        let result = InputSource::execute_command(
            "echo 'file1.txt' && echo 'file2.txt'",
            Duration::from_secs(5),
            &subprocess_manager,
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["item"], "file1.txt");
        assert_eq!(result[1]["item"], "file2.txt");
    }
}
