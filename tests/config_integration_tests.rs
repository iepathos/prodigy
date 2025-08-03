use std::fs;
use std::process::Command;
use tempfile::TempDir;

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

#[tokio::test]
async fn test_config_integration() {
    use mmm::config::*;
    use std::path::PathBuf;

    // Test configuration loading and merging
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    // Create .mmm directory
    let mmm_dir = temp_dir.path().join(".mmm");
    fs::create_dir(&mmm_dir).unwrap();

    // Create project config in YAML format
    let project_config = r#"
name: test-project
auto_commit: true
max_iterations: 5
spec_dir: specs
"#;

    fs::write(mmm_dir.join("config.yml"), project_config).unwrap();

    // Test loading
    let loader = ConfigLoader::new().await.unwrap();
    loader.load_project(temp_dir.path()).await.unwrap();
    let config = loader.get_config();
    assert_eq!(config.get_auto_commit(), true);
    assert_eq!(config.get_spec_dir(), PathBuf::from("specs"));

    // Test environment variable override
    // Note: Environment variables set global config, but project config takes precedence
    std::env::set_var("MMM_AUTO_COMMIT", "false");
    let mut config = loader.get_config();
    config.merge_env_vars();
    // Since project config has auto_commit=true, it takes precedence over global env var
    assert_eq!(config.get_auto_commit(), true);
    
    // Test that env var is actually set in global config
    assert_eq!(config.global.auto_commit, Some(false));

    // Clean up
    std::env::remove_var("MMM_AUTO_COMMIT");
}
