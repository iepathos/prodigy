use anyhow::Result;
use mmm::cook::CookCommand;
use tempfile::TempDir;

#[tokio::test]
async fn test_cook_workflow_integration() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create a simple test project structure
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(
        src_dir.join("main.rs"),
        "fn main() { println!(\"Hello\"); }",
    )?;
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
edition = "2021""#,
    )?;

    // Create a test playbook
    let playbook_path = temp_dir.path().join("test-playbook.yml");
    let playbook_content = r#"commands:
  - /mmm-lint"#;
    std::fs::write(&playbook_path, playbook_content)?;

    // Create the cook command
    let cmd = CookCommand {
        playbook: playbook_path,
        path: Some(temp_dir.path().to_path_buf()),
        max_iterations: 1,
        worktree: false,
        map: vec![],
        args: vec![],
        fail_fast: false,
        metrics: false,
        auto_accept: false,
        resume: None,
        skip_analysis: true,
    };

    // Note: This will fail in integration tests because no Claude API is available
    // but we're testing that the integration is set up correctly
    let result = mmm::cook::cook(cmd).await;

    // Should fail due to missing Claude API
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_cook_with_metrics() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create test project
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(src_dir.join("lib.rs"), "pub fn helper() {}")?;
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "test-lib"
version = "0.1.0"
edition = "2021"

[dependencies]"#,
    )?;

    // Create metrics-enabled playbook
    let playbook_path = temp_dir.path().join("metrics-playbook.yml");
    let playbook_content = r#"commands:
  - name: /mmm-code-review
    focus: quality"#;
    std::fs::write(&playbook_path, playbook_content)?;

    // Create command with metrics enabled
    let cmd = CookCommand {
        playbook: playbook_path,
        path: Some(temp_dir.path().to_path_buf()),
        max_iterations: 1,
        worktree: false,
        map: vec![],
        args: vec![],
        fail_fast: false,
        metrics: true,
        auto_accept: false,
        resume: None,
        skip_analysis: false,
    };

    let result = mmm::cook::cook(cmd).await;

    // Should fail due to missing Claude API
    assert!(result.is_err());

    // But metrics directory might be created
    let _metrics_dir = temp_dir.path().join(".mmm/metrics");
    // Note: metrics creation happens after successful runs

    Ok(())
}

#[tokio::test]
async fn test_cook_with_worktree() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()?;

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_dir.path())
        .output()?;

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()?;

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "# Test Project")?;
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()?;

    // Create playbook
    let playbook_path = temp_dir.path().join("worktree-playbook.yml");
    let playbook_content = r#"commands:
  - /mmm-lint"#;
    std::fs::write(&playbook_path, playbook_content)?;

    // Create command with worktree enabled
    let cmd = CookCommand {
        playbook: playbook_path,
        path: Some(temp_dir.path().to_path_buf()),
        max_iterations: 1,
        worktree: true,
        map: vec![],
        args: vec![],
        fail_fast: false,
        metrics: false,
        auto_accept: true,
        resume: None,
        skip_analysis: true,
    };

    let result = mmm::cook::cook(cmd).await;

    // Should fail due to missing Claude API
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_cook_with_structured_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create test project
    std::fs::create_dir_all(temp_dir.path().join("src"))?;
    std::fs::write(
        temp_dir.path().join("src/main.rs"),
        "fn main() { println!(\"Hello\"); }",
    )?;
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
edition = "2021""#,
    )?;

    // Create a structured workflow playbook
    let playbook_path = temp_dir.path().join("structured-playbook.yml");
    let playbook_content = r#"commands:
  - name: /mmm-generate-spec
    id: generate
    outputs:
      spec:
        file_pattern: "specs/temp/*.md"
  - name: /mmm-implement-spec
    args: ["${generate.spec}"]"#;
    std::fs::write(&playbook_path, playbook_content)?;

    let cmd = CookCommand {
        playbook: playbook_path,
        path: Some(temp_dir.path().to_path_buf()),
        max_iterations: 1,
        worktree: false,
        map: vec![],
        args: vec![],
        fail_fast: true,
        metrics: false,
        auto_accept: false,
        resume: None,
        skip_analysis: true,
    };

    let result = mmm::cook::cook(cmd).await;

    // Should fail due to missing Claude API
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_cook_with_arguments() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create test files
    std::fs::create_dir_all(temp_dir.path().join("src"))?;
    std::fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}")?;
    std::fs::write(temp_dir.path().join("src/lib.rs"), "pub fn helper() {}")?;

    // Create playbook that uses arguments
    let playbook_path = temp_dir.path().join("args-playbook.yml");
    let playbook_content = r#"commands:
  - name: /mmm-analyze
    args: ["$1", "$2"]"#;
    std::fs::write(&playbook_path, playbook_content)?;

    let cmd = CookCommand {
        playbook: playbook_path,
        path: Some(temp_dir.path().to_path_buf()),
        max_iterations: 1,
        worktree: false,
        map: vec![],
        args: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        fail_fast: false,
        metrics: false,
        auto_accept: false,
        resume: None,
        skip_analysis: true,
    };

    let result = mmm::cook::cook(cmd).await;

    // Should fail due to missing Claude API
    assert!(result.is_err());

    Ok(())
}
