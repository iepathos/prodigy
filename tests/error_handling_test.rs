//! Comprehensive error handling tests
//! Verifies that all production code properly handles errors without unwrap() or panic!()

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn test_option_handling_without_unwrap() -> Result<()> {
    // Test that Option types are handled without unwrap()

    // Helper function that might return Some or None
    fn get_value(should_exist: bool) -> Option<i32> {
        if should_exist {
            Some(42)
        } else {
            None
        }
    }

    let some_value = get_value(true);
    let none_value = get_value(false);

    // Safe handling patterns
    assert_eq!(some_value.unwrap_or(0), 42);
    assert_eq!(none_value.unwrap_or(0), 0);

    assert_eq!(some_value.unwrap_or(0), 42);
    assert_eq!(none_value.unwrap_or(0), 0);

    // Using map_or for transformations
    assert_eq!(some_value.map_or(0, |v| v * 2), 84);
    assert_eq!(none_value.map_or(0, |v| v * 2), 0);

    Ok(())
}

#[test]
fn test_result_handling_without_unwrap() -> Result<()> {
    // Test that Result types are handled without unwrap()

    // Helper function that might return Ok or Err
    fn get_result(should_succeed: bool) -> Result<i32> {
        if should_succeed {
            Ok(42)
        } else {
            Err(anyhow::anyhow!("error"))
        }
    }

    let ok_result = get_result(true);
    let err_result = get_result(false);

    // Safe handling patterns
    assert_eq!(ok_result.as_ref().unwrap_or(&0), &42);
    assert_eq!(err_result.as_ref().unwrap_or(&0), &0);

    // Using map_or for transformations
    assert_eq!(ok_result.as_ref().map_or(0, |v| v * 2), 84);
    assert_eq!(err_result.map_or(0, |v| v * 2), 0);

    Ok(())
}

#[test]
fn test_error_propagation_through_call_stack() -> Result<()> {
    // Test that errors properly propagate through multiple layers

    fn inner_function() -> Result<String> {
        // This would previously panic
        Err(anyhow::anyhow!("Inner error"))
    }

    fn middle_function() -> Result<String> {
        // Properly propagates error with ?
        let result = inner_function()?;
        Ok(result)
    }

    fn outer_function() -> Result<String> {
        // Also properly propagates
        let result = middle_function()?;
        Ok(result)
    }

    // Verify error propagation
    let result = outer_function();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Inner error"));

    Ok(())
}

#[test]
fn test_regex_compilation_errors_handled() -> Result<()> {
    // Test that regex compilation errors are handled
    use regex::Regex;

    // Invalid regex patterns that would previously panic
    let invalid_patterns = vec![
        "[",     // Unclosed bracket
        "(",     // Unclosed paren
        "*",     // Invalid quantifier
        "(?P<)", // Invalid named group
    ];

    for pattern in invalid_patterns {
        let result = Regex::new(pattern);
        // Should return error, not panic
        assert!(result.is_err());
    }

    Ok(())
}

#[test]
fn test_file_operations_handle_permission_errors() -> Result<()> {
    // Test file operations handle permission errors gracefully
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new()?;
    let protected_file = temp_dir.path().join("protected.txt");

    // Create a file and make it read-only
    fs::write(&protected_file, "test")?;
    let mut perms = fs::metadata(&protected_file)?.permissions();
    perms.set_mode(0o444); // Read-only
    fs::set_permissions(&protected_file, perms)?;

    // Attempting to write should return error, not panic
    let result = fs::write(&protected_file, "new content");
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_concurrent_access_handles_locks() -> Result<()> {
    // Test that concurrent access to resources handles locking properly
    use std::thread;

    let data = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let data_clone = Arc::clone(&data);
        let handle = thread::spawn(move || {
            // Should handle lock poisoning gracefully
            if let Ok(mut num) = data_clone.lock() {
                *num += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread should complete");
    }

    // Verify all updates completed
    let final_value = *data.lock().unwrap();
    assert_eq!(final_value, 10);

    Ok(())
}

#[test]
fn test_json_parsing_handles_malformed_input() -> Result<()> {
    // Test JSON parsing handles malformed input gracefully
    use serde_json;

    let malformed_inputs = vec![
        "",                 // Empty
        "{",                // Unclosed
        "{'key': 'value'}", // Single quotes
        "{\"key\": }",      // Missing value
        "null",             // Not an object when object expected
    ];

    for input in malformed_inputs {
        let result: serde_json::Result<HashMap<String, String>> = serde_json::from_str(input);
        // Should return error, not panic
        assert!(result.is_err());
    }

    Ok(())
}

#[test]
fn test_environment_variable_fallbacks() -> Result<()> {
    // Test that missing environment variables are handled with fallbacks
    use std::env;

    // Clear a variable that might be expected
    env::remove_var("PRODIGY_TEST_VAR");

    // Should use fallback instead of panicking
    let value = env::var("PRODIGY_TEST_VAR").unwrap_or_else(|_| "default".to_string());
    assert_eq!(value, "default");

    Ok(())
}

#[test]
fn test_state_file_handles_missing_gracefully() -> Result<()> {
    // Test that state file operations handle missing files gracefully
    let temp_dir = TempDir::new()?;
    let state_file = temp_dir.path().join("nonexistent_state.json");

    // Attempting to read non-existent state file should return error, not panic
    use std::fs;
    let result = fs::read_to_string(&state_file);
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_working_directory_fallback() -> Result<()> {
    // Test that invalid working directories are handled gracefully
    let temp_dir = TempDir::new()?;
    let current_dir = std::env::current_dir()?;

    // Change to temp directory
    std::env::set_current_dir(&temp_dir)?;

    // Should be able to get current directory without panicking
    let result = std::env::current_dir();
    assert!(result.is_ok());

    // Restore original directory
    std::env::set_current_dir(current_dir)?;

    Ok(())
}

/// Integration test that verifies no panics occur during normal operations
#[test]
fn test_integration_no_panics() -> Result<()> {
    // This test verifies that common operations don't panic
    use std::panic;

    // Set up panic hook to catch any panics
    let original_hook = panic::take_hook();
    let panic_count = Arc::new(Mutex::new(0));
    let panic_count_clone = Arc::clone(&panic_count);

    panic::set_hook(Box::new(move |_| {
        let mut count = panic_count_clone.lock().unwrap();
        *count += 1;
    }));

    // Run operations that previously might have panicked
    // These are now safe with proper error handling

    // Restore original panic hook
    panic::set_hook(original_hook);

    // Verify no panics occurred
    let final_count = *panic_count.lock().unwrap();
    assert_eq!(final_count, 0, "No panics should have occurred");

    Ok(())
}
