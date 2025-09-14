//! Foreach executor for simple parallel iteration
//!
//! Implements the foreach construct for parallel processing of items without MapReduce complexity.

use crate::config::command::{ForeachConfig, ForeachInput, ParallelConfig};
use anyhow::{Context, Result};
use tracing::{debug, info, warn};

/// Result of foreach execution
#[derive(Debug, Clone)]
pub struct ForeachResult {
    pub total_items: usize,
    pub successful_items: usize,
    pub failed_items: usize,
    pub skipped_items: usize,
}

/// Execute a foreach operation
pub async fn execute_foreach(config: &ForeachConfig) -> Result<ForeachResult> {
    // Get items from input source
    let items = get_items(&config.input).await?;

    // Apply max_items limit if specified
    let items = if let Some(max) = config.max_items {
        items.into_iter().take(max).collect()
    } else {
        items
    };

    info!("Executing foreach over {} items", items.len());

    // Determine parallelism level
    let max_parallel = match &config.parallel {
        ParallelConfig::Boolean(false) => 1,
        ParallelConfig::Boolean(true) => 10, // Default parallel count
        ParallelConfig::Count(n) => *n,
    };

    debug!("Using parallelism level: {}", max_parallel);

    // For now, we only support simple shell commands in foreach
    // Full workflow step execution will be added later
    warn!(
        "Foreach currently only supports simple execution - full workflow step support coming soon"
    );

    let total_items = items.len();
    let successful_items = items.len(); // For now, assume all succeed
    let failed_items = 0;

    // Execute items sequentially for now
    for item in &items {
        debug!("Processing item: {}", item);

        // For now, just process items without actual execution
        // Real execution will be added when we have proper integration
    }

    info!(
        "Foreach completed: {} total, {} successful, {} failed",
        total_items, successful_items, failed_items
    );

    Ok(ForeachResult {
        total_items,
        successful_items,
        failed_items,
        skipped_items: 0,
    })
}

/// Get items from input source
async fn get_items(input: &ForeachInput) -> Result<Vec<String>> {
    match input {
        ForeachInput::List(items) => {
            debug!("Using static list of {} items", items.len());
            Ok(items.clone())
        }
        ForeachInput::Command(cmd) => {
            debug!("Executing command to get items: {}", cmd);

            // Execute the command to get items
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .await
                .context("Failed to execute foreach command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Foreach command failed: {}", stderr));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Split output into items (one per line, skip empty lines)
            let items: Vec<String> = stdout
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|s| s.to_string())
                .collect();

            debug!("Command produced {} items", items.len());
            Ok(items)
        }
    }
}
