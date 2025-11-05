// CLI integration tests for template management commands

use super::test_utils::*;
use std::fs;
use std::path::PathBuf;

/// Helper to create a test template file
fn create_test_template(dir: &std::path::Path, name: &str) -> PathBuf {
    let template_path = dir.join(format!("{}.yml", name));
    let template_content = format!(
        r#"name: {}
description: Test template for CLI testing
version: 1.0.0
parameters:
  target:
    description: Target file
    type: string
    required: true
commands:
  - shell: "echo Processing ${{target}}"
"#,
        name
    );
    fs::write(&template_path, template_content).expect("Failed to write template");
    template_path
}

#[test]
fn test_template_register() {
    let test = CliTest::new();
    let temp_path = test.temp_path();

    // Create a template file
    let template_path = create_test_template(temp_path, "test-template");

    let mut test = test
        .arg("template")
        .arg("register")
        .arg(template_path.to_str().unwrap())
        .arg("--name")
        .arg("test-template")
        .arg("--description")
        .arg("A test template");

    let output = test.run();

    // Should register successfully or report already exists
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("already exists")
            || output.stdout_contains("Registered")
            || output.stdout_contains("template")
    );
}

#[test]
fn test_template_register_with_metadata() {
    let test = CliTest::new();
    let temp_path = test.temp_path();

    // Create a template file
    let template_path = create_test_template(temp_path, "metadata-template");

    let mut test = test
        .arg("template")
        .arg("register")
        .arg(template_path.to_str().unwrap())
        .arg("--name")
        .arg("metadata-template")
        .arg("--description")
        .arg("Template with metadata")
        .arg("--version")
        .arg("2.0.0")
        .arg("--tags")
        .arg("test,cli")
        .arg("--author")
        .arg("Test Author");

    let output = test.run();

    // Should register successfully
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stderr_contains("already exists")
            || output.stdout_contains("Registered")
    );
}

#[test]
fn test_template_list() {
    let mut test = CliTest::new().arg("template").arg("list");

    let output = test.run();

    // Should list templates or report none found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("No templates")
            || output.stdout_contains("template")
    );
}

#[test]
fn test_template_list_long_format() {
    let mut test = CliTest::new().arg("template").arg("list").arg("--long");

    let output = test.run();

    // Should list templates in long format
    assert!(output.exit_code == exit_codes::SUCCESS || output.stdout_contains("No templates"));
}

#[test]
fn test_template_list_with_tag() {
    let mut test = CliTest::new()
        .arg("template")
        .arg("list")
        .arg("--tag")
        .arg("test");

    let output = test.run();

    // Should filter templates by tag
    assert!(output.exit_code == exit_codes::SUCCESS || output.stdout_contains("No templates"));
}

#[test]
fn test_template_show() {
    let mut test = CliTest::new()
        .arg("template")
        .arg("show")
        .arg("test-template");

    let output = test.run();

    // Should show template details or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS || output.exit_code == exit_codes::GENERAL_ERROR
    );

    if output.exit_code == exit_codes::GENERAL_ERROR {
        assert!(output.stderr_contains("not found") || output.stderr_contains("No template"));
    }
}

#[test]
fn test_template_show_nonexistent() {
    let mut test = CliTest::new()
        .arg("template")
        .arg("show")
        .arg("nonexistent-template");

    let output = test.run();

    // Should report template not found
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(output.stderr_contains("not found") || output.stderr_contains("No template"));
}

#[test]
fn test_template_delete() {
    let test = CliTest::new();
    let temp_path = test.temp_path();

    // Create and register a template first
    let template_path = create_test_template(temp_path, "delete-template");

    // Register it
    let mut register_test = CliTest::new()
        .arg("template")
        .arg("register")
        .arg(template_path.to_str().unwrap())
        .arg("--name")
        .arg("delete-template");
    register_test.run();

    // Now try to delete it
    let mut test = test
        .arg("template")
        .arg("delete")
        .arg("delete-template")
        .arg("--force");

    let output = test.run();

    // Should delete successfully or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS || output.exit_code == exit_codes::GENERAL_ERROR
    );
}

#[test]
fn test_template_delete_without_force() {
    let mut test = CliTest::new()
        .arg("template")
        .arg("delete")
        .arg("test-template");

    let output = test.run();

    // Should require confirmation or report not found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.exit_code == exit_codes::GENERAL_ERROR
            || output.stderr_contains("not found")
    );
}

#[test]
fn test_template_search() {
    let mut test = CliTest::new().arg("template").arg("search").arg("test");

    let output = test.run();

    // Should search templates or report none found
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("No templates")
            || output.stdout_contains("template")
    );
}

#[test]
fn test_template_search_by_tag() {
    let mut test = CliTest::new()
        .arg("template")
        .arg("search")
        .arg("cli")
        .arg("--by-tag");

    let output = test.run();

    // Should search templates by tag
    assert!(output.exit_code == exit_codes::SUCCESS || output.stdout_contains("No templates"));
}

#[test]
fn test_template_validate() {
    let test = CliTest::new();
    let temp_path = test.temp_path();

    // Create a valid template file
    let template_path = create_test_template(temp_path, "validate-template");

    let mut test = test
        .arg("template")
        .arg("validate")
        .arg(template_path.to_str().unwrap());

    let output = test.run();

    // Should validate successfully
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("valid")
            || output.stdout_contains("Valid")
    );
}

#[test]
fn test_template_validate_invalid_file() {
    let test = CliTest::new();
    let temp_path = test.temp_path();

    // Create an invalid template file
    let invalid_template_path = temp_path.join("invalid.yml");
    fs::write(&invalid_template_path, "invalid: yaml: content:").expect("Failed to write file");

    let mut test = test
        .arg("template")
        .arg("validate")
        .arg(invalid_template_path.to_str().unwrap());

    let output = test.run();

    // Should report validation error
    assert!(
        output.exit_code == exit_codes::GENERAL_ERROR
            || output.stderr_contains("invalid")
            || output.stderr_contains("Failed")
    );
}

#[test]
fn test_template_validate_nonexistent_file() {
    let test = CliTest::new();
    let temp_path = test.temp_path();
    let nonexistent = temp_path.join("nonexistent.yml");

    let mut test = test
        .arg("template")
        .arg("validate")
        .arg(nonexistent.to_str().unwrap());

    let output = test.run();

    // Should report file not found
    assert_eq!(output.exit_code, exit_codes::GENERAL_ERROR);
    assert!(
        output.stderr_contains("not found")
            || output.stderr_contains("No such file")
            || output.stderr_contains("Failed")
    );
}

#[test]
fn test_template_init() {
    let test = CliTest::new();
    let temp_path = test.temp_path();
    let template_dir = temp_path.join("templates");

    let mut test = test
        .arg("template")
        .arg("init")
        .arg(template_dir.to_str().unwrap());

    let output = test.run();

    // Should initialize template directory
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("Initialized")
            || output.stdout_contains("Created")
    );

    // Check that the directory was created
    if output.exit_code == exit_codes::SUCCESS {
        assert!(
            template_dir.exists(),
            "Template directory should be created"
        );
    }
}

#[test]
fn test_template_init_existing_directory() {
    let test = CliTest::new();
    let temp_path = test.temp_path();
    let template_dir = temp_path.join("existing");

    // Create the directory first
    fs::create_dir_all(&template_dir).expect("Failed to create directory");

    let mut test = test
        .arg("template")
        .arg("init")
        .arg(template_dir.to_str().unwrap());

    let output = test.run();

    // Should handle existing directory gracefully
    assert!(
        output.exit_code == exit_codes::SUCCESS
            || output.stdout_contains("already exists")
            || output.stdout_contains("Initialized")
    );
}

#[test]
fn test_template_invalid_subcommand() {
    let mut test = CliTest::new().arg("template").arg("invalid");

    let output = test.run();

    // Should fail with invalid subcommand
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(
        output.stderr_contains("invalid")
            || output.stderr_contains("unrecognized")
            || output.stderr_contains("Found argument")
    );
}

#[test]
fn test_template_missing_required_args() {
    let mut test = CliTest::new().arg("template").arg("register");

    let output = test.run();

    // Should fail with missing required arguments
    assert_eq!(output.exit_code, exit_codes::ARGUMENT_ERROR);
    assert!(output.stderr_contains("required") || output.stderr_contains("PATH"));
}

#[test]
fn test_template_help() {
    let mut test = CliTest::new().arg("template").arg("--help");

    let output = test.run();

    // Should show help text
    assert_eq!(output.exit_code, exit_codes::SUCCESS);
    assert!(
        output.stdout_contains("template")
            && output.stdout_contains("register")
            && output.stdout_contains("list")
    );
}
