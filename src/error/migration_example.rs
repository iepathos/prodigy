/// Example module showing how to migrate existing code to use the unified error system
///
/// This module demonstrates various patterns for migrating from:
/// - String errors
/// - anyhow errors
/// - Custom error types
/// - std::io::Error and other standard library errors

use super::{ProdigyError, ErrorCode, ErrorExt, common};
use std::path::Path;

// Example 1: Migrating from string errors
pub fn load_config_old(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config: {}", e))
}

pub fn load_config_new(path: &Path) -> Result<String, ProdigyError> {
    std::fs::read_to_string(path)
        .map_err(|e| {
            ProdigyError::config_with_code(
                ErrorCode::CONFIG_NOT_FOUND,
                format!("Cannot read config file: {}", path.display())
            ).with_source(e)
        })
}

// Example 2: Using the ErrorExt trait for cleaner code
pub fn load_config_with_trait(path: &Path) -> Result<String, ProdigyError> {
    std::fs::read_to_string(path)
        .to_config_error(format!("Cannot read config file: {}", path.display()))
}

// Example 3: Migrating from anyhow
pub fn process_workflow_old(name: &str) -> anyhow::Result<()> {
    anyhow::bail!("Workflow {} not found", name)
}

pub fn process_workflow_new(name: &str) -> Result<(), ProdigyError> {
    Err(ProdigyError::workflow_with_code(
        ErrorCode::WORKFLOW_NOT_FOUND,
        format!("Workflow '{}' not found", name),
        Some(name.to_string())
    ))
}

// Example 4: Using helper functions
pub fn find_session(session_id: &str) -> Result<String, ProdigyError> {
    // Simulate session lookup
    if session_id.is_empty() {
        return Err(common::session_not_found(session_id));
    }
    Ok(session_id.to_string())
}

// Example 5: Error recovery patterns
pub fn execute_with_recovery(command: &str) -> Result<String, ProdigyError> {
    match execute_command(command) {
        Ok(output) => Ok(output),
        Err(e) if e.is_recoverable() => {
            // Try recovery
            tracing::warn!("Command failed, attempting recovery: {}", e);
            execute_command_with_fallback(command)
                .map_err(|recovery_err| {
                    ProdigyError::execution("Recovery failed after initial failure")
                        .with_source(recovery_err)
                })
        }
        Err(e) => Err(e),
    }
}

// Example 6: Adding context to errors (modern pattern with .context())
pub fn complex_operation(data: &str) -> Result<String, ProdigyError> {
    // Add context at each effect boundary
    validate_data(data)
        .context("Failed to validate data")?;

    transform_data(data)
        .context("Failed to transform data")?;

    save_data(data)
        .context("Failed to save data")
}

// Example 7: Using the macro
pub fn macro_example() -> Result<(), ProdigyError> {
    use crate::prodigy_error;

    // Simple error
    let _err = prodigy_error!(config: "Configuration is invalid");

    // Error with source
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file");
    let _err_with_source = prodigy_error!(storage: "Cannot save file", io_err);

    Ok(())
}

// Example 8: Pattern for CLI commands
pub fn cli_command_handler() -> anyhow::Result<()> {
    // ProdigyError automatically converts to anyhow::Error
    let result = internal_operation()?;

    // Can still use anyhow features
    external_library_call()
        .context("Failed to call external library")?;

    Ok(result)
}

// Example 9: Effect boundary migration - comprehensive example
pub fn process_workflow_file(workflow_path: &Path) -> Result<(), ProdigyError> {
    // Effect boundary 1: File I/O
    let content = std::fs::read_to_string(workflow_path)
        .map_err(ProdigyError::from)
        .context(format!("Failed to read workflow file at {}", workflow_path.display()))?;

    // Effect boundary 2: Parsing/deserialization
    let workflow: WorkflowConfig = serde_json::from_str(&content)
        .map_err(ProdigyError::from)
        .context("Failed to parse workflow JSON")?;

    // Effect boundary 3: Validation
    validate_workflow(&workflow)
        .context(format!("Validation failed for workflow '{}'", workflow.name))?;

    // Effect boundary 4: External command execution
    execute_setup_commands(&workflow.setup)
        .context("Failed to execute setup commands")?;

    // Effect boundary 5: Storage operation
    save_workflow_state(&workflow)
        .context("Failed to persist workflow state")?;

    Ok(())
}

// Example 10: Dynamic context with closures (for loops/iterations)
pub fn process_multiple_items(items: Vec<WorkItem>) -> Result<Vec<String>, ProdigyError> {
    items.iter()
        .map(|item| {
            process_single_item(item)
                .with_context(|| format!("Failed to process item {}", item.id))
        })
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to process work items batch")
}

// Example 11: Before/After migration pattern
mod before_migration {
    use super::*;

    // BEFORE: No context chaining
    pub fn deploy_workflow_old(path: &Path) -> Result<(), ProdigyError> {
        let workflow = std::fs::read_to_string(path)
            .map_err(ProdigyError::from)?;

        let parsed: WorkflowConfig = serde_json::from_str(&workflow)
            .map_err(ProdigyError::from)?;

        validate_workflow(&parsed)?;
        execute_setup_commands(&parsed.setup)?;

        Ok(())
    }
}

mod after_migration {
    use super::*;

    // AFTER: Rich context at each boundary
    pub fn deploy_workflow_new(path: &Path) -> Result<(), ProdigyError> {
        // Effect boundary: file I/O
        let workflow = std::fs::read_to_string(path)
            .map_err(ProdigyError::from)
            .context(format!("Failed to read workflow from {}", path.display()))?;

        // Effect boundary: deserialization
        let parsed: WorkflowConfig = serde_json::from_str(&workflow)
            .map_err(ProdigyError::from)
            .context("Failed to parse workflow YAML")?;

        // Effect boundary: validation
        validate_workflow(&parsed)
            .context(format!("Validation failed for workflow '{}'", parsed.name))?;

        // Effect boundary: command execution
        execute_setup_commands(&parsed.setup)
            .context("Failed during workflow setup phase")?;

        Ok(())
    }
}

// Helper types for examples
#[allow(dead_code)]
struct WorkflowConfig {
    name: String,
    setup: Vec<String>,
}

#[allow(dead_code)]
struct WorkItem {
    id: String,
    data: String,
}

// Helper functions for examples
fn execute_command(_cmd: &str) -> Result<String, ProdigyError> {
    Err(ProdigyError::execution("Command failed").with_exit_code(1))
}

fn execute_command_with_fallback(_cmd: &str) -> Result<String, ProdigyError> {
    Ok("fallback output".to_string())
}

fn validate_data(_data: &str) -> Result<(), ProdigyError> {
    Ok(())
}

fn transform_data(_data: &str) -> Result<String, ProdigyError> {
    Ok("transformed".to_string())
}

fn save_data(_data: &str) -> Result<String, ProdigyError> {
    Ok("saved".to_string())
}

fn internal_operation() -> Result<(), ProdigyError> {
    Ok(())
}

fn external_library_call() -> anyhow::Result<()> {
    Ok(())
}

fn validate_workflow(_workflow: &WorkflowConfig) -> Result<(), ProdigyError> {
    Ok(())
}

fn execute_setup_commands(_commands: &[String]) -> Result<(), ProdigyError> {
    Ok(())
}

fn save_workflow_state(_workflow: &WorkflowConfig) -> Result<(), ProdigyError> {
    Ok(())
}

fn process_single_item(_item: &WorkItem) -> Result<String, ProdigyError> {
    Ok("processed".to_string())
}

use anyhow::Context;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_migration() {
        let err = process_workflow_new("test").unwrap_err();
        assert_eq!(err.code(), ErrorCode::WORKFLOW_NOT_FOUND);
        assert!(err.user_message().contains("Workflow error"));
    }

    #[test]
    fn test_error_recovery() {
        let result = execute_with_recovery("test");
        // Should succeed with fallback
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_context() {
        let result = complex_operation("test");
        assert!(result.is_ok());
    }
}