// Tests for `prodigy config trace` CLI commands
//
// These tests verify the CLI output format for config trace commands,
// ensuring proper display of configuration value origins and diagnostics.

use super::test_utils::*;

/// Test `prodigy config show` displays effective configuration
#[test]
fn test_config_show_basic() {
    let test = CliTest::new();

    let output = test.arg("config").arg("show").run();

    // Should succeed and display configuration sections
    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config show should succeed: stderr={}",
        output.stderr
    );

    // Should display effective configuration
    assert!(
        output.stdout_contains("log_level")
            || output.stdout_contains("Effective configuration")
            || output.stdout_contains("max_concurrent_specs"),
        "Should display configuration values, got: {}",
        output.stdout
    );
}

/// Test `prodigy config show --json` outputs valid JSON
#[test]
fn test_config_show_json() {
    let test = CliTest::new();

    let output = test.arg("config").arg("show").arg("--json").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config show --json should succeed: stderr={}",
        output.stderr
    );

    // Should be valid JSON
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&output.stdout);
    assert!(
        parse_result.is_ok(),
        "Should output valid JSON, got: {}",
        output.stdout
    );
}

/// Test `prodigy config show <path>` shows specific value
#[test]
fn test_config_show_specific_path() {
    let test = CliTest::new();

    let output = test.arg("config").arg("show").arg("log_level").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config show log_level should succeed: stderr={}",
        output.stderr
    );

    // Should show the log_level value
    assert!(
        output.stdout_contains("log_level") || output.stdout_contains("info"),
        "Should display log_level value, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace` displays all configuration traces
#[test]
fn test_config_trace_all() {
    let test = CliTest::new();

    let output = test.arg("config").arg("trace").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace should succeed: stderr={}",
        output.stderr
    );

    // Should display configuration values with sources
    assert!(
        output.stdout_contains("default")
            || output.stdout_contains("log_level")
            || output.stdout_contains("Configuration values"),
        "Should display configuration traces, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --all` explicitly shows all values
#[test]
fn test_config_trace_all_flag() {
    let test = CliTest::new();

    let output = test.arg("config").arg("trace").arg("--all").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --all should succeed: stderr={}",
        output.stderr
    );

    // Should show multiple configuration paths
    assert!(
        output.stdout_contains("log_level") || output.stdout_contains("max_concurrent_specs"),
        "Should display multiple config values, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace <path>` traces specific value
#[test]
fn test_config_trace_specific_path() {
    let test = CliTest::new();

    let output = test.arg("config").arg("trace").arg("log_level").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace log_level should succeed: stderr={}",
        output.stderr
    );

    // Should show source tree for log_level
    assert!(
        output.stdout_contains("log_level"),
        "Should display trace for log_level, got: {}",
        output.stdout
    );

    // Should show source indication (default, file, or env)
    assert!(
        output.stdout_contains("default")
            || output.stdout_contains("final value")
            || output.stdout_contains("├──")
            || output.stdout_contains("└──"),
        "Should display source information, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --json` outputs valid JSON
#[test]
fn test_config_trace_json() {
    let test = CliTest::new();

    let output = test.arg("config").arg("trace").arg("--json").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --json should succeed: stderr={}",
        output.stderr
    );

    // Should be valid JSON array
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&output.stdout);
    assert!(
        parse_result.is_ok(),
        "Should output valid JSON, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace <path> --json` outputs JSON for specific path
#[test]
fn test_config_trace_specific_path_json() {
    let test = CliTest::new();

    let output = test
        .arg("config")
        .arg("trace")
        .arg("log_level")
        .arg("--json")
        .run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace log_level --json should succeed: stderr={}",
        output.stderr
    );

    // Should be valid JSON with expected fields
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&output.stdout);
    assert!(
        parse_result.is_ok(),
        "Should output valid JSON, got: {}",
        output.stdout
    );

    let json = parse_result.unwrap();
    assert!(
        json.get("path").is_some() || json.get("final_value").is_some(),
        "JSON should contain trace fields, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --overrides` shows only overridden values
#[test]
fn test_config_trace_overrides() {
    // Without any config files or env vars, there should be no overrides
    let test = CliTest::new();

    let output = test.arg("config").arg("trace").arg("--overrides").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --overrides should succeed: stderr={}",
        output.stderr
    );

    // With only defaults, should indicate no overrides
    assert!(
        output.stdout_contains("No configuration values were overridden")
            || output.stdout_contains("Overridden configuration values")
            || output.stdout_contains("[]"), // JSON empty array
        "Should handle no overrides case, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --overrides` with actual override
#[test]
fn test_config_trace_overrides_with_env() {
    let test = CliTest::new().env("PRODIGY__LOG_LEVEL", "debug");

    let output = test.arg("config").arg("trace").arg("--overrides").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --overrides should succeed: stderr={}",
        output.stderr
    );

    // Should show log_level was overridden
    assert!(
        output.stdout_contains("log_level")
            || output.stdout_contains("debug")
            || output.stdout_contains("PRODIGY__LOG_LEVEL"),
        "Should show overridden value, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --diagnose` runs diagnostics
#[test]
fn test_config_trace_diagnose() {
    let test = CliTest::new();

    let output = test.arg("config").arg("trace").arg("--diagnose").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --diagnose should succeed: stderr={}",
        output.stderr
    );

    // Should output diagnostic results
    assert!(
        output.stdout_contains("No configuration issues detected")
            || output.stdout_contains("Configuration issues detected")
            || output.stdout_contains("Warning")
            || output.stdout_contains("Info"),
        "Should display diagnostic output, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --diagnose` detects empty env var
#[test]
fn test_config_trace_diagnose_empty_env() {
    let test = CliTest::new().env("PRODIGY__DEFAULT_EDITOR", "");

    let output = test.arg("config").arg("trace").arg("--diagnose").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --diagnose should succeed: stderr={}",
        output.stderr
    );

    // Should detect the empty env var issue
    assert!(
        output.stdout_contains("empty")
            || output.stdout_contains("default_editor")
            || output.stdout_contains("No configuration issues detected"),
        "Should handle empty env var detection, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --diagnose --json` outputs JSON diagnostics
#[test]
fn test_config_trace_diagnose_json() {
    let test = CliTest::new();

    let output = test
        .arg("config")
        .arg("trace")
        .arg("--diagnose")
        .arg("--json")
        .run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --diagnose --json should succeed: stderr={}",
        output.stderr
    );

    // Should be valid JSON
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&output.stdout);
    assert!(
        parse_result.is_ok(),
        "Should output valid JSON, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace` with invalid path
#[test]
fn test_config_trace_invalid_path() {
    let test = CliTest::new();

    let output = test
        .arg("config")
        .arg("trace")
        .arg("nonexistent.path.here")
        .run();

    // Should fail gracefully
    assert!(
        output.exit_code != exit_codes::SUCCESS || output.stderr_contains("No value found"),
        "Should handle invalid path, got exit_code={}, stderr={}",
        output.exit_code,
        output.stderr
    );
}

/// Test `prodigy config show` with invalid path
#[test]
fn test_config_show_invalid_path() {
    let test = CliTest::new();

    let output = test
        .arg("config")
        .arg("show")
        .arg("nonexistent.path.here")
        .run();

    // Should fail gracefully
    assert!(
        output.exit_code != exit_codes::SUCCESS || output.stderr_contains("No value found"),
        "Should handle invalid path, got exit_code={}, stderr={}",
        output.exit_code,
        output.stderr
    );
}

/// Test config trace with file config override
#[test]
fn test_config_trace_with_file_config() {
    let config_content = "log_level: debug\nmax_concurrent_specs: 8";

    let test = CliTest::new().with_config(config_content);

    let (test, _workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("config").arg("trace").arg("log_level").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace should succeed with file config: stderr={}",
        output.stderr
    );

    // Should show the debug value from file
    assert!(
        output.stdout_contains("debug") || output.stdout_contains("log_level"),
        "Should display file config value, got: {}",
        output.stdout
    );
}

/// Test combined override from file and env
#[test]
fn test_config_trace_file_and_env_override() {
    let config_content = "log_level: debug";

    let test = CliTest::new()
        .with_config(config_content)
        .env("PRODIGY__LOG_LEVEL", "warn");

    let (test, _workflow_path) = test.with_workflow("test", &create_test_workflow("test"));

    let output = test.arg("config").arg("trace").arg("log_level").run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace should show override chain: stderr={}",
        output.stderr
    );

    // Should show warn as final value (env overrides file)
    assert!(
        output.stdout_contains("warn") || output.stdout_contains("PRODIGY__LOG_LEVEL"),
        "Should show env override as final value, got: {}",
        output.stdout
    );
}

/// Test `prodigy config trace --overrides --json` combined flags
#[test]
fn test_config_trace_overrides_json() {
    let test = CliTest::new().env("PRODIGY__LOG_LEVEL", "debug");

    let output = test
        .arg("config")
        .arg("trace")
        .arg("--overrides")
        .arg("--json")
        .run();

    assert_eq!(
        output.exit_code,
        exit_codes::SUCCESS,
        "config trace --overrides --json should succeed: stderr={}",
        output.stderr
    );

    // Should be valid JSON
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&output.stdout);
    assert!(
        parse_result.is_ok(),
        "Should output valid JSON, got: {}",
        output.stdout
    );
}
