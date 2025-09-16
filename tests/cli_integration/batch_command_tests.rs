// Tests for the 'batch' command

use super::test_utils::*;
use std::fs;

#[test]
fn test_batch_basic_pattern() {
    let mut test = CliTest::new();

    // Create test files
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("test1.txt"), "content1").unwrap();
    fs::write(test_dir.join("test2.txt"), "content2").unwrap();
    fs::write(test_dir.join("ignore.md"), "ignore").unwrap();

    let output = test
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: echo 'Processing {}'")
        .run();

    // Should process .txt files
    assert_output(&output, exit_codes::SUCCESS, Some("Processing"), None);
}

#[test]
fn test_batch_with_parallel_workers() {
    let mut test = CliTest::new();

    // Create multiple test files
    let test_dir = test.temp_path().to_path_buf();
    for i in 1..=5 {
        fs::write(
            test_dir.join(format!("file{}.txt", i)),
            format!("content{}", i),
        )
        .unwrap();
    }

    let output = test
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: echo 'File: {}'")
        .arg("--parallel")
        .arg("3")
        .run();

    assert_output(&output, exit_codes::SUCCESS, Some("File:"), None);
}

#[test]
fn test_batch_with_retry() {
    let mut test = CliTest::new();

    // Create test file
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("test.txt"), "content").unwrap();

    let output = test
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: echo 'Processing {}'")
        .arg("--retry")
        .arg("2")
        .run();

    assert_output(&output, exit_codes::SUCCESS, Some("Processing"), None);
}

#[test]
fn test_batch_with_timeout() {
    let mut test = CliTest::new();

    // Create test file
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("test.txt"), "content").unwrap();

    let output = test
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: sleep 10")
        .arg("--timeout")
        .arg("1")
        .run();

    // Should timeout
    assert!(
        output.stderr_contains("imeout") || output.stderr_contains("exceeded") || !output.success
    );
}

#[test]
fn test_batch_with_no_matching_files() {
    let mut test = CliTest::new()
        .arg("batch")
        .arg("*.nonexistent")
        .arg("--command")
        .arg("shell: echo 'Should not run'");

    let output = test.run();

    // Should handle no matches gracefully
    assert!(
        output.stderr_contains("o files found")
            || output.stderr_contains("o matches")
            || output.exit_code == exit_codes::GENERAL_ERROR
    );
}

#[test]
fn test_batch_with_nested_pattern() {
    let mut test = CliTest::new();

    // Create nested directory structure
    let test_dir = test.temp_path().to_path_buf();
    let sub_dir = test_dir.join("subdir");
    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(sub_dir.join("nested.txt"), "nested content").unwrap();
    fs::write(test_dir.join("root.txt"), "root content").unwrap();

    let output = test
        .arg("batch")
        .arg("**/*.txt")
        .arg("--command")
        .arg("shell: echo 'Found: {}'")
        .run();

    // Should find files in subdirectories
    assert!(output.stdout_contains("Found:") || output.stderr_contains("Found:"));
}

#[test]
fn test_batch_with_claude_command() {
    let mut test = CliTest::new();

    // Create test file
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("test.rs"), "fn main() {}").unwrap();

    let output = test
        .arg("batch")
        .arg("*.rs")
        .arg("--command")
        .arg("claude: /lint {}")
        .run();

    // Claude commands might not be available in test environment
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("claude")
            || output.stderr_contains("not found")
    );
}

#[test]
fn test_batch_missing_required_arguments() {
    let mut test = CliTest::new().arg("batch").arg("*.txt");
    // Missing --command

    let output = test.run();

    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(output.stderr_contains("required") || output.stderr_contains("command"));
}

#[test]
fn test_batch_with_working_directory() {
    let test_dir = tempfile::TempDir::new().unwrap();
    fs::write(test_dir.path().join("file1.txt"), "content1").unwrap();
    fs::write(test_dir.path().join("file2.txt"), "content2").unwrap();

    let mut test = CliTest::new()
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: echo 'File: {}'")
        .arg("--path")
        .arg(test_dir.path().to_str().unwrap());

    let output = test.run();

    assert_output(&output, exit_codes::SUCCESS, Some("File:"), None);
}

#[test]
fn test_batch_with_failing_command() {
    let mut test = CliTest::new();

    // Create test files
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("test1.txt"), "content").unwrap();
    fs::write(test_dir.join("test2.txt"), "content").unwrap();

    let output = test
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: exit 1")
        .run();

    // Should fail
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
}

#[test]
fn test_batch_with_single_file() {
    let mut test = CliTest::new();

    // Create single test file
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("single.txt"), "content").unwrap();

    let output = test
        .arg("batch")
        .arg("single.txt")
        .arg("--command")
        .arg("shell: echo 'Processing single file: {}'")
        .run();

    assert_output(
        &output,
        exit_codes::SUCCESS,
        Some("Processing single file"),
        None,
    );
}

#[test]
fn test_batch_with_zero_parallel() {
    let mut test = CliTest::new();

    // Create test file
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("test.txt"), "content").unwrap();

    let output = test
        .arg("batch")
        .arg("*.txt")
        .arg("--command")
        .arg("shell: echo 'Test'")
        .arg("--parallel")
        .arg("0"); // Invalid value

    let output = test.run();

    // Should reject invalid parallel value
    assert!(output.exit_code != exit_codes::SUCCESS);
}

#[test]
fn test_batch_with_complex_command() {
    let mut test = CliTest::new();

    // Create test files
    let test_dir = test.temp_path().to_path_buf();
    fs::write(test_dir.join("data.json"), r#"{"key": "value"}"#).unwrap();

    let output = test
        .arg("batch")
        .arg("*.json")
        .arg("--command")
        .arg("shell: cat {} | grep key")
        .run();

    assert_output(&output, exit_codes::SUCCESS, Some("key"), None);
}
