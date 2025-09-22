//! Input validation utilities
//!
//! This module provides validation functions for CLI arguments and user inputs.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Validate a workflow file path exists and is readable
pub fn validate_workflow_file(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        return Err(anyhow::anyhow!(
            "Workflow file '{}' does not exist",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(anyhow::anyhow!("Path '{}' is not a file", path.display()));
    }

    // Check if file is readable
    std::fs::File::open(path)
        .with_context(|| format!("Cannot read workflow file '{}'", path.display()))?;

    Ok(())
}

/// Validate a directory path exists and is accessible
pub fn validate_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow::anyhow!(
            "Directory '{}' does not exist",
            path.display()
        ));
    }

    if !path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path '{}' is not a directory",
            path.display()
        ));
    }

    Ok(())
}

/// Validate timeout value is reasonable
pub fn validate_timeout(timeout: u64) -> Result<()> {
    if timeout == 0 {
        return Err(anyhow::anyhow!("Timeout must be greater than 0"));
    }

    if timeout > 86400 {
        // 24 hours
        return Err(anyhow::anyhow!(
            "Timeout of {} seconds is unreasonably large (max: 86400)",
            timeout
        ));
    }

    Ok(())
}

/// Validate threshold value is within valid range
pub fn validate_threshold(threshold: u32) -> Result<()> {
    if threshold > 100 {
        return Err(anyhow::anyhow!(
            "Threshold must be between 0 and 100, got {}",
            threshold
        ));
    }

    Ok(())
}

/// Validate parallel worker count is reasonable
pub fn validate_parallel_count(parallel: usize) -> Result<()> {
    if parallel == 0 {
        return Err(anyhow::anyhow!("Parallel count must be greater than 0"));
    }

    if parallel > 100 {
        return Err(anyhow::anyhow!(
            "Parallel count of {} is too high (max: 100)",
            parallel
        ));
    }

    Ok(())
}
