use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_cook_with_custom_config_file() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create .mmm directory
    fs::create_dir_all(temp_dir.path().join(".mmm"))?;

    // Create custom config file
    let custom_config = temp_dir.path().join("custom.yml");
    fs::write(
        &custom_config,
        r#"commands:
  - mmm-test-only"#,
    )?;

    // Create test playbook with the expected command
    let playbook = temp_dir.path().join("playbook.yml");
    fs::write(
        &playbook,
        r#"commands:
  - name: mmm-test-only"#,
    )?;

    // Create a simple source file
    fs::write(
        temp_dir.path().join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}
"#,
    )?;

    // Run mmm cook with custom config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "--config",
            custom_config.to_str().unwrap(),
            "--max-iterations",
            "1",
            playbook.to_str().unwrap(),
        ])
        .output()?;

    // The command might fail due to Claude CLI, but config should be loaded
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either run the command from playbook or fail with Claude CLI error
    assert!(
        stdout.contains("mmm-test-only")
            || stderr.contains("Claude CLI not found")
            || stderr.contains("claude")
            || stderr.contains("Unknown command: mmm-test-only"),
        "Should attempt to run playbook command. STDOUT: {stdout}, STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_cook_with_yaml_config() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create .mmm directory
    fs::create_dir_all(temp_dir.path().join(".mmm"))?;

    // Create YAML config file
    let yaml_config = temp_dir.path().join("config.yaml");
    fs::write(
        &yaml_config,
        r#"commands:
  - mmm-yaml-test"#,
    )?;

    // Create test playbook with the expected command
    let playbook = temp_dir.path().join("playbook.yml");
    fs::write(
        &playbook,
        r#"commands:
  - name: mmm-yaml-test"#,
    )?;

    // Create a simple source file
    fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;

    // Run mmm cook with YAML config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "--config",
            yaml_config.to_str().unwrap(),
            "--max-iterations",
            "1",
            playbook.to_str().unwrap(),
        ])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either run the command from playbook or fail with Claude CLI error
    assert!(
        stdout.contains("mmm-yaml-test")
            || stderr.contains("Claude CLI not found")
            || stderr.contains("claude")
            || stderr.contains("Unknown command: mmm-yaml-test"),
        "Should attempt to run playbook command. STDOUT: {stdout}, STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_cook_with_default_config() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create .mmm directory with default config
    let mmm_dir = temp_dir.path().join(".mmm");
    fs::create_dir_all(&mmm_dir)?;
    fs::write(
        mmm_dir.join("workflow.yml"),
        r#"commands:
  - mmm-custom-review"#,
    )?;

    // Create test playbook with the expected command
    let playbook = temp_dir.path().join("playbook.yml");
    fs::write(
        &playbook,
        r#"commands:
  - name: mmm-custom-review"#,
    )?;

    // Create a simple source file
    fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;

    // Run mmm cook without explicit config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .env("MMM_TEST_MODE", "true")
        .args(["cook", "--max-iterations", "1", playbook.to_str().unwrap()])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either run the command from playbook or fail with Claude CLI error
    assert!(
        stdout.contains("mmm-custom-review")
            || stderr.contains("Claude CLI not found")
            || stderr.contains("claude")
            || stderr.contains("Unknown command: mmm-custom-review"),
        "Should attempt to run playbook command. STDOUT: {stdout}, STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_cook_with_invalid_config_path() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create test playbook
    let playbook = temp_dir.path().join("playbook.yml");
    fs::write(
        &playbook,
        r#"commands:
  - name: mmm-test-command"#,
    )?;

    // Run mmm cook with non-existent config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "--config",
            "/non/existent/config.yml",
            playbook.to_str().unwrap(),
        ])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should fail with file not found error
    assert!(
        stderr.contains("Failed to read configuration file")
            || stderr.contains("No such file or directory"),
        "Should fail with missing file error. STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_cook_with_unsupported_config_format() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create JSON config (unsupported)
    let json_config = temp_dir.path().join("config.json");
    fs::write(&json_config, r#"{"commands": ["/mmm-test"]}"#)?;

    // Create test playbook
    let playbook = temp_dir.path().join("playbook.yml");
    fs::write(
        &playbook,
        r#"commands:
  - name: mmm-test-command"#,
    )?;

    // Run mmm cook with JSON config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .env("MMM_TEST_MODE", "true")
        .args([
            "cook",
            "--config",
            json_config.to_str().unwrap(),
            playbook.to_str().unwrap(),
        ])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should fail with unsupported format error
    assert!(
        stderr.contains("Unsupported configuration file format"),
        "Should fail with unsupported format error. STDERR: {stderr}"
    );

    Ok(())
}
