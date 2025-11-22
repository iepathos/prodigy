//! Input validation utilities using stillwater for error accumulation
//!
//! This module provides validation functions for CLI arguments and user inputs.

use anyhow::Result;
use std::path::{Path, PathBuf};
use stillwater::Validation;

/// CLI validation errors
#[derive(Debug, Clone, PartialEq)]
pub enum CliValidationError {
    WorkflowFileNotFound(PathBuf),
    WorkflowPathNotFile(PathBuf),
    WorkflowFileNotReadable { path: PathBuf, reason: String },
    DirectoryNotFound(PathBuf),
    PathNotDirectory(PathBuf),
    TimeoutZero,
    TimeoutTooLarge(u64),
    ThresholdTooLarge(u32),
    ParallelCountZero,
    ParallelCountTooLarge(usize),
}

impl std::fmt::Display for CliValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WorkflowFileNotFound(p) => {
                write!(f, "Workflow file '{}' does not exist", p.display())
            }
            Self::WorkflowPathNotFile(p) => write!(f, "Path '{}' is not a file", p.display()),
            Self::WorkflowFileNotReadable { path, reason } => {
                write!(
                    f,
                    "Cannot read workflow file '{}': {}",
                    path.display(),
                    reason
                )
            }
            Self::DirectoryNotFound(p) => write!(f, "Directory '{}' does not exist", p.display()),
            Self::PathNotDirectory(p) => write!(f, "Path '{}' is not a directory", p.display()),
            Self::TimeoutZero => write!(f, "Timeout must be greater than 0"),
            Self::TimeoutTooLarge(timeout) => write!(
                f,
                "Timeout of {} seconds is unreasonably large (max: 86400)",
                timeout
            ),
            Self::ThresholdTooLarge(threshold) => {
                write!(f, "Threshold must be between 0 and 100, got {}", threshold)
            }
            Self::ParallelCountZero => write!(f, "Parallel count must be greater than 0"),
            Self::ParallelCountTooLarge(parallel) => {
                write!(f, "Parallel count of {} is too high (max: 100)", parallel)
            }
        }
    }
}

impl std::error::Error for CliValidationError {}

/// Validate a workflow file path exists and is readable
pub fn validate_workflow_file(path: &PathBuf) -> Result<()> {
    let validation = validate_workflow_file_internal(path);
    match validation.into_result() {
        Ok(_) => Ok(()),
        Err(errors) => Err(anyhow::anyhow!(
            "Workflow file validation failed:\n{}",
            errors
                .iter()
                .map(|e| format!("  - {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    }
}

fn validate_workflow_file_internal(path: &PathBuf) -> Validation<PathBuf, Vec<CliValidationError>> {
    let mut errors = Vec::new();

    if !path.exists() {
        errors.push(CliValidationError::WorkflowFileNotFound(path.clone()));
    }

    if !path.is_file() {
        errors.push(CliValidationError::WorkflowPathNotFile(path.clone()));
    }

    // Check if file is readable
    if let Err(e) = std::fs::File::open(path) {
        errors.push(CliValidationError::WorkflowFileNotReadable {
            path: path.clone(),
            reason: e.to_string(),
        });
    }

    if errors.is_empty() {
        Validation::success(path.clone())
    } else {
        Validation::failure(errors)
    }
}

/// Validate a directory path exists and is accessible
pub fn validate_directory(path: &Path) -> Result<()> {
    let validation = validate_directory_internal(path);
    match validation.into_result() {
        Ok(_) => Ok(()),
        Err(errors) => Err(anyhow::anyhow!(
            "Directory validation failed:\n{}",
            errors
                .iter()
                .map(|e| format!("  - {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    }
}

fn validate_directory_internal(path: &Path) -> Validation<PathBuf, Vec<CliValidationError>> {
    let mut errors = Vec::new();

    if !path.exists() {
        errors.push(CliValidationError::DirectoryNotFound(path.to_path_buf()));
    }

    if !path.is_dir() {
        errors.push(CliValidationError::PathNotDirectory(path.to_path_buf()));
    }

    if errors.is_empty() {
        Validation::success(path.to_path_buf())
    } else {
        Validation::failure(errors)
    }
}

/// Validate timeout value is reasonable
pub fn validate_timeout(timeout: u64) -> Result<()> {
    let validation = validate_timeout_internal(timeout);
    match validation.into_result() {
        Ok(_) => Ok(()),
        Err(error) => Err(anyhow::anyhow!("{}", error)),
    }
}

fn validate_timeout_internal(timeout: u64) -> Validation<u64, CliValidationError> {
    if timeout == 0 {
        Validation::failure(CliValidationError::TimeoutZero)
    } else if timeout > 86400 {
        // 24 hours
        Validation::failure(CliValidationError::TimeoutTooLarge(timeout))
    } else {
        Validation::success(timeout)
    }
}

/// Validate threshold value is within valid range
pub fn validate_threshold(threshold: u32) -> Result<()> {
    let validation = validate_threshold_internal(threshold);
    match validation.into_result() {
        Ok(_) => Ok(()),
        Err(error) => Err(anyhow::anyhow!("{}", error)),
    }
}

fn validate_threshold_internal(threshold: u32) -> Validation<u32, CliValidationError> {
    if threshold > 100 {
        Validation::failure(CliValidationError::ThresholdTooLarge(threshold))
    } else {
        Validation::success(threshold)
    }
}

/// Validate parallel worker count is reasonable
pub fn validate_parallel_count(parallel: usize) -> Result<()> {
    let validation = validate_parallel_count_internal(parallel);
    match validation.into_result() {
        Ok(_) => Ok(()),
        Err(error) => Err(anyhow::anyhow!("{}", error)),
    }
}

fn validate_parallel_count_internal(parallel: usize) -> Validation<usize, CliValidationError> {
    if parallel == 0 {
        Validation::failure(CliValidationError::ParallelCountZero)
    } else if parallel > 100 {
        Validation::failure(CliValidationError::ParallelCountTooLarge(parallel))
    } else {
        Validation::success(parallel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_validate_workflow_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("workflow.yml");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "name: test").unwrap();

        let result = validate_workflow_file(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_workflow_file_not_found() {
        let path = PathBuf::from("/nonexistent/workflow.yml");
        let result = validate_workflow_file(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_validate_directory_success() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_directory(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_directory_not_found() {
        let path = Path::new("/nonexistent/directory");
        let result = validate_directory(path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_validate_timeout() {
        assert!(validate_timeout(0).is_err());
        assert!(validate_timeout(60).is_ok());
        assert!(validate_timeout(86400).is_ok());
        assert!(validate_timeout(86401).is_err());
    }

    #[test]
    fn test_validate_threshold() {
        assert!(validate_threshold(0).is_ok());
        assert!(validate_threshold(50).is_ok());
        assert!(validate_threshold(100).is_ok());
        assert!(validate_threshold(101).is_err());
    }

    #[test]
    fn test_validate_parallel_count() {
        assert!(validate_parallel_count(0).is_err());
        assert!(validate_parallel_count(1).is_ok());
        assert!(validate_parallel_count(50).is_ok());
        assert!(validate_parallel_count(100).is_ok());
        assert!(validate_parallel_count(101).is_err());
    }
}
