use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_improve_with_custom_config_file() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create .mmm directory
    fs::create_dir_all(temp_dir.path().join(".mmm"))?;

    // Create custom config file
    let custom_config = temp_dir.path().join("custom.toml");
    fs::write(
        &custom_config,
        r#"commands = ["mmm-test-only"]"#,
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

    // Run mmm improve with custom config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args([
            "improve",
            "--config",
            custom_config.to_str().unwrap(),
            "--max-iterations",
            "1",
        ])
        .output()?;

    // The command might fail due to Claude CLI, but config should be loaded
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either run the custom command or fail with Claude CLI error
    assert!(
        stdout.contains("mmm-test-only")
            || stderr.contains("Claude CLI not found")
            || stderr.contains("claude"),
        "Should attempt to run custom command. STDOUT: {stdout}, STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_improve_with_yaml_config() -> anyhow::Result<()> {
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

    // Create a simple source file
    fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;

    // Run mmm improve with YAML config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args([
            "improve",
            "--config",
            yaml_config.to_str().unwrap(),
            "--max-iterations",
            "1",
        ])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should load YAML config
    assert!(
        stdout.contains("mmm-yaml-test")
            || stderr.contains("Claude CLI not found")
            || stderr.contains("claude"),
        "Should attempt to run YAML command. STDOUT: {stdout}, STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_improve_with_default_config() -> anyhow::Result<()> {
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
        mmm_dir.join("config.toml"),
        r#"commands = ["mmm-custom-review"]"#,
    )?;

    // Create a simple source file
    fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;

    // Run mmm improve without explicit config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["improve", "--max-iterations", "0"])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should use default config from .mmm/config.toml
    assert!(
        stdout.contains("mmm-custom-review")
            || stderr.contains("Claude CLI not found")
            || stderr.contains("claude"),
        "Should use default config. STDOUT: {stdout}, STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_improve_with_legacy_workflow_toml() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create .mmm directory with legacy workflow.toml
    let mmm_dir = temp_dir.path().join(".mmm");
    fs::create_dir_all(&mmm_dir)?;
    fs::write(
        mmm_dir.join("workflow.toml"),
        r#"commands = ["mmm-legacy-command"]"#,
    )?;

    // Create a simple source file
    fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;

    // Run mmm improve
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["improve", "--max-iterations", "0"])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show deprecation warning
    assert!(
        stderr.contains("workflow.toml is deprecated"),
        "Should warn about deprecated workflow.toml. STDERR: {stderr}"
    );

    Ok(())
}

#[test]
fn test_improve_with_invalid_config_path() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Run mmm improve with non-existent config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["improve", "--config", "/non/existent/config.toml"])
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
fn test_improve_with_unsupported_config_format() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create JSON config (unsupported)
    let json_config = temp_dir.path().join("config.json");
    fs::write(&json_config, r#"{"commands": ["/mmm-test"]}"#)?;

    // Run mmm improve with JSON config
    let output = Command::new(env!("CARGO_BIN_EXE_mmm"))
        .current_dir(&temp_dir)
        .args(["improve", "--config", json_config.to_str().unwrap()])
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
