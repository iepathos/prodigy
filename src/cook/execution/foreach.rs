//! Foreach executor for simple parallel iteration
//!
//! Implements the foreach construct for parallel processing of items without MapReduce complexity.

use crate::config::command::{
    ForeachConfig, ForeachInput, ParallelConfig, WorkflowStepCommand,
};
use crate::cook::execution::interpolation::{InterpolationContext, InterpolationEngine};
use crate::subprocess::{ProcessCommand, SubprocessManager};
use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Result of foreach execution
#[derive(Debug, Clone)]
pub struct ForeachResult {
    pub total_items: usize,
    pub successful_items: usize,
    pub failed_items: usize,
    pub skipped_items: usize,
    pub errors: Vec<String>,
}

/// Context for executing foreach commands
struct ForeachExecutionContext {
    /// Item being processed
    item: String,
    /// Index of the item
    index: usize,
    /// Total number of items
    total: usize,
    /// Working directory
    working_dir: PathBuf,
    /// Environment variables
    env_vars: HashMap<String, String>,
    /// Subprocess manager
    subprocess_manager: Arc<SubprocessManager>,
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

    let total_items = items.len();
    if total_items == 0 {
        info!("No items to process in foreach");
        return Ok(ForeachResult {
            total_items: 0,
            successful_items: 0,
            failed_items: 0,
            skipped_items: 0,
            errors: vec![],
        });
    }

    info!("Executing foreach over {} items", total_items);

    // Determine parallelism level
    let max_parallel = match &config.parallel {
        ParallelConfig::Boolean(false) => 1,
        ParallelConfig::Boolean(true) => 10, // Default parallel count
        ParallelConfig::Count(n) => *n,
    };

    debug!("Using parallelism level: {}", max_parallel);

    // Create progress bar
    let progress_bar = create_progress_bar(total_items);
    progress_bar.set_message("Processing items");

    // Execute items in parallel with semaphore for concurrency control
    let semaphore = Arc::new(Semaphore::new(max_parallel));
    let subprocess_manager = Arc::new(SubprocessManager::production());

    // Create futures for all items
    let mut futures = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let context = ForeachExecutionContext {
            item: item.clone(),
            index,
            total: total_items,
            working_dir: std::env::current_dir()?,
            env_vars: HashMap::new(),
            subprocess_manager: subprocess_manager.clone(),
        };

        let do_block = config.do_block.clone();
        let continue_on_error = config.continue_on_error;
        let semaphore = semaphore.clone();
        let progress = progress_bar.clone();

        let future = async move {
            let _permit = semaphore.acquire().await.unwrap();
            let result = execute_item_commands(context, &do_block).await;
            progress.inc(1);

            match result {
                Ok(_) => Ok(()),
                Err(e) if continue_on_error => {
                    warn!("Item {} failed but continuing: {}", item, e);
                    Err(e.to_string())
                }
                Err(e) => {
                    error!("Item {} failed: {}", item, e);
                    Err(e.to_string())
                }
            }
        };

        futures.push(future);
    }

    // Execute all futures and collect results
    let results = join_all(futures).await;

    // Count successes and failures
    let mut successful_items = 0;
    let mut failed_items = 0;
    let mut errors = Vec::new();

    for result in results {
        match result {
            Ok(_) => successful_items += 1,
            Err(error_msg) => {
                failed_items += 1;
                errors.push(error_msg);
                if !config.continue_on_error {
                    progress_bar.finish_with_message("Failed - stopping execution");
                    return Err(anyhow!("Foreach execution failed: {}", errors.join(", ")));
                }
            }
        }
    }

    progress_bar.finish_with_message(format!(
        "Completed: {} successful, {} failed",
        successful_items, failed_items
    ));

    info!(
        "Foreach completed: {} total, {} successful, {} failed",
        total_items, successful_items, failed_items
    );

    Ok(ForeachResult {
        total_items,
        successful_items,
        failed_items,
        skipped_items: 0,
        errors,
    })
}

/// Get items from input source
pub async fn get_items(input: &ForeachInput) -> Result<Vec<String>> {
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

/// Execute commands for a single item
async fn execute_item_commands(
    context: ForeachExecutionContext,
    commands: &[Box<WorkflowStepCommand>],
) -> Result<()> {
    debug!("Processing item: {}", context.item);

    // Create interpolation context with item variable
    let mut interpolation_context = InterpolationContext::new();
    interpolation_context.set("item", json!(context.item));
    interpolation_context.set("index", json!(context.index));
    interpolation_context.set("total", json!(context.total));

    // Create interpolation engine
    let mut engine = InterpolationEngine::new(false);

    // Execute each command in the do block
    for (cmd_index, command) in commands.iter().enumerate() {
        debug!(
            "Executing command {} for item {}",
            cmd_index + 1,
            context.item
        );

        // Interpolate variables in the command
        let interpolated_command = interpolate_command(command, &mut engine, &interpolation_context)?;

        // Execute the interpolated command
        execute_single_command(
            &interpolated_command,
            &context.working_dir,
            &context.env_vars,
            &context.subprocess_manager,
        )
        .await
        .with_context(|| {
            format!(
                "Failed to execute command {} for item '{}'",
                cmd_index + 1,
                context.item
            )
        })?;
    }

    Ok(())
}

/// Interpolate variables in a command
fn interpolate_command(
    command: &WorkflowStepCommand,
    engine: &mut InterpolationEngine,
    context: &InterpolationContext,
) -> Result<WorkflowStepCommand> {
    let mut interpolated = command.clone();

    // Interpolate based on which field is present
    if let Some(claude_cmd) = &command.claude {
        interpolated.claude = Some(engine.interpolate(claude_cmd, context)?);
    }
    if let Some(shell_cmd) = &command.shell {
        interpolated.shell = Some(engine.interpolate(shell_cmd, context)?);
    }
    if let Some(test_cmd) = &command.test {
        let mut new_test = test_cmd.clone();
        new_test.command = engine.interpolate(&test_cmd.command, context)?;
        interpolated.test = Some(new_test);
    }

    Ok(interpolated)
}

/// Execute a single command
async fn execute_single_command(
    command: &WorkflowStepCommand,
    working_dir: &PathBuf,
    env_vars: &HashMap<String, String>,
    subprocess_manager: &Arc<SubprocessManager>,
) -> Result<()> {
    // Execute based on which field is present
    if let Some(shell_cmd) = &command.shell {
        debug!("Executing shell command: {}", shell_cmd);

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(shell_cmd);
        cmd.current_dir(working_dir);

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute shell command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Shell command failed: {}", stderr));
        }

        return Ok(());
    }

    if let Some(claude_cmd) = &command.claude {
        debug!("Executing Claude command: {}", claude_cmd);

        // Use subprocess manager to execute Claude command
        let process_command = ProcessCommand {
            program: "claude".to_string(),
            args: vec![claude_cmd.clone()],
            env: env_vars.clone(),
            working_dir: Some(working_dir.clone()),
            stdin: None,
            timeout: None,
            suppress_stderr: false,
        };

        let result = subprocess_manager
            .runner()
            .run(process_command)
            .await
            .context("Failed to execute Claude command")?;

        if result.status.code() != Some(0) {
            return Err(anyhow!("Claude command failed: {}", result.stderr));
        }

        return Ok(());
    }

    if let Some(test_cmd) = &command.test {
        warn!("Test command type is deprecated, executing as shell command: {}", test_cmd.command);

        // Execute test as shell command (deprecated)
        let mut cmd_builder = tokio::process::Command::new("sh");
        cmd_builder.arg("-c").arg(&test_cmd.command);
        cmd_builder.current_dir(working_dir);

        for (key, value) in env_vars {
            cmd_builder.env(key, value);
        }

        let output = cmd_builder
            .output()
            .await
            .context("Failed to execute test command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Test command failed: {}", stderr));
        }

        return Ok(());
    }

    // Other command types not supported in foreach
    Err(anyhow!(
        "Command type not supported in foreach do block"
    ))
}

/// Create a progress bar for foreach execution
fn create_progress_bar(total: usize) -> ProgressBar {
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
            )
            .unwrap()
            .progress_chars("█▓▒░ "),
    );
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_foreach_execution() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec!["item1".to_string(), "item2".to_string()]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Processing ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: false,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();
        assert_eq!(result.total_items, 2);
        assert_eq!(result.successful_items, 2);
        assert_eq!(result.failed_items, 0);
    }

    #[tokio::test]
    async fn test_foreach_with_interpolation() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec!["test1".to_string(), "test2".to_string()]),
            parallel: ParallelConfig::Boolean(false),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Item ${item} at index ${index} of ${total}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: false,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();
        assert_eq!(result.total_items, 2);
        assert_eq!(result.successful_items, 2);
    }

    #[tokio::test]
    async fn test_foreach_parallel_execution() {
        let config = ForeachConfig {
            input: ForeachInput::List(vec![
                "p1".to_string(),
                "p2".to_string(),
                "p3".to_string(),
            ]),
            parallel: ParallelConfig::Count(2),
            do_block: vec![Box::new(WorkflowStepCommand {
                shell: Some("echo Parallel ${item}".to_string()),
                claude: None,
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: false,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let result = execute_foreach(&config).await.unwrap();
        assert_eq!(result.total_items, 3);
        assert_eq!(result.successful_items, 3);
        assert_eq!(result.failed_items, 0);
    }
}
