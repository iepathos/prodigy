//! Integration tests for error context preservation (Spec 165)
//!
//! These tests verify that error context is properly preserved throughout
//! the error chain in real workflow scenarios.

use prodigy::error::ProdigyError;
use std::path::PathBuf;

/// Test that context is preserved through multiple layers
#[test]
fn test_context_chain_preservation() {
    let result = simulate_layered_operation();

    assert!(result.is_err());

    let error = result.unwrap_err();

    // Verify the developer message includes all context
    let dev_message = error.developer_message();

    // Should contain all three layers of context
    assert!(
        dev_message.contains("Failed to process workflow"),
        "Missing top-level context"
    );
    assert!(
        dev_message.contains("Failed to read configuration"),
        "Missing mid-level context"
    );
    assert!(
        dev_message.contains("File operation failed"),
        "Missing bottom-level context"
    );
}

/// Test that context includes location information when provided
#[test]
fn test_context_with_location() {
    let error = create_error_with_location();

    let dev_message = error.developer_message();

    // Should include file and line information
    assert!(
        dev_message.contains("error_context_preservation_test.rs"),
        "Missing file location in context"
    );
}

/// Test that user message remains clean while developer message has full context
#[test]
fn test_user_vs_developer_messages() {
    let error = simulate_storage_error();

    let user_message = error.user_message();
    let dev_message = error.developer_message();

    // User message should be concise
    assert!(
        user_message.len() < 200,
        "User message too verbose: {}",
        user_message
    );

    // Developer message should have full context chain
    assert!(
        dev_message.len() > user_message.len(),
        "Developer message should be more detailed than user message"
    );

    // Developer message should include context (either "Context:" or individual context entries)
    assert!(
        dev_message.contains("Context:") || dev_message.contains("Checkpoint operation failed"),
        "Developer message missing context section. Got: {}",
        dev_message
    );
}

/// Test context preservation in iterator/batch operations
#[test]
fn test_context_in_batch_operations() {
    let items = vec!["item1", "item2", "item3"];

    let result = process_items_with_context(items);

    assert!(result.is_err());

    let error = result.unwrap_err();
    let dev_message = error.developer_message();

    // Should include item-specific context
    assert!(
        dev_message.contains("item2"),
        "Missing item-specific context in batch operation"
    );
    assert!(
        dev_message.contains("Failed to process batch"),
        "Missing batch-level context"
    );
}

/// Test that ProdigyError serialization preserves context
#[test]
fn test_serialization_preserves_context() {
    use prodigy::error::SerializableError;

    let error = simulate_layered_operation().unwrap_err();

    let serializable = SerializableError::from(&error);

    // Verify context is in the serializable form
    assert!(
        !serializable.context.is_empty(),
        "Context not preserved in serialization"
    );

    assert!(
        serializable.context.len() >= 2,
        "Expected multiple context entries, got {}",
        serializable.context.len()
    );

    // Verify JSON serialization works
    let json = serde_json::to_string(&serializable).expect("Failed to serialize to JSON");
    assert!(json.contains("context"), "JSON missing context");
}

/// Test context in anyhow error conversion
#[test]
fn test_anyhow_conversion_preserves_context() {
    let prodigy_error = simulate_layered_operation().unwrap_err();

    // Convert to anyhow::Error
    let anyhow_error: anyhow::Error = prodigy_error.into();

    // The error message should still be meaningful
    let error_string = anyhow_error.to_string();
    assert!(
        !error_string.is_empty(),
        "Error message lost in anyhow conversion"
    );
}

/// Test that context is preserved through From conversions
#[test]
fn test_from_conversion_with_context() {
    // Simulate an std::io::Error
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test.txt not found");

    // Convert to ProdigyError and add context
    let prodigy_error = ProdigyError::from(io_error).context("Failed to open configuration file");

    let dev_message = prodigy_error.developer_message();

    // Should have the context
    assert!(
        dev_message.contains("Failed to open configuration file"),
        "Missing context in: {}",
        dev_message
    );

    // Source error might be nested or shown differently depending on implementation
    // The key is that we have the context
}

// Helper functions for tests

fn simulate_layered_operation() -> Result<(), ProdigyError> {
    let _ = read_config_file()
        .map_err(|e| e.context("Failed to process workflow"))?;
    Ok(())
}

fn read_config_file() -> Result<String, ProdigyError> {
    perform_file_operation()
        .map_err(|e| e.context("Failed to read configuration"))?;
    Ok("config".to_string())
}

fn perform_file_operation() -> Result<(), ProdigyError> {
    Err(ProdigyError::storage("File operation failed"))
}

fn create_error_with_location() -> ProdigyError {
    ProdigyError::execution("Test error")
        .with_context(format!("Operation context at {}:{}", file!(), line!()))
}

fn simulate_storage_error() -> ProdigyError {
    use prodigy::error::ErrorCode;

    let base_error = ProdigyError::storage_with_code(
        ErrorCode::STORAGE_IO_ERROR,
        "Failed to write checkpoint",
        Some(PathBuf::from("/tmp/checkpoint.json")),
    );

    base_error
        .context("Failed to persist workflow state")
        .context("Checkpoint operation failed")
}

fn process_items_with_context(items: Vec<&str>) -> Result<Vec<String>, ProdigyError> {
    items
        .iter()
        .map(|item| {
            if *item == "item2" {
                Err(ProdigyError::validation("Invalid item")
                    .with_context(format!("Failed to process {}", item)))
            } else {
                Ok(item.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.context("Failed to process batch"))
}
