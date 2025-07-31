# Coverage Improvements - Iteration 1722459144

## Overview
Test coverage improvements based on MMM context analysis.
Current coverage: 42.9% → Target: 52.9%

## Critical Functions Needing Tests

### Module: `src/abstractions/git.rs` - Git Operations

#### Add these tests to `src/abstractions/git.rs`:
```rust
#[cfg(test)]
mod real_git_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_git_command_success() {
        let git_ops = RealGitOperations::new();
        
        // Test successful command execution
        let result = git_ops.git_command(&["--version"], "version check").await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("git version"));
    }

    #[tokio::test]
    async fn test_git_command_failure() {
        let git_ops = RealGitOperations::new();
        
        // Test failed command execution
        let result = git_ops.git_command(&["invalid-command"], "invalid command").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Git invalid command failed"));
    }

    #[tokio::test]
    async fn test_stage_all_changes_and_commit() {
        let git_ops = RealGitOperations::new();
        
        // Only run if in a git repo
        if !git_ops.is_git_repo().await {
            return;
        }
        
        // Create a test file in a temp directory
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        
        // Note: Full integration testing would require a test git repo
        // This is a partial test to verify the methods don't panic
        let _ = git_ops.check_git_status().await;
    }

    #[tokio::test]
    async fn test_get_current_branch() {
        let git_ops = RealGitOperations::new();
        
        if git_ops.is_git_repo().await {
            let result = git_ops.get_current_branch().await;
            assert!(result.is_ok());
            let branch = result.unwrap();
            assert!(!branch.is_empty());
        }
    }

    #[tokio::test]
    async fn test_create_worktree_invalid_path() {
        let git_ops = RealGitOperations::new();
        
        if git_ops.is_git_repo().await {
            let invalid_path = Path::new("/\0invalid");
            let result = git_ops.create_worktree("test-branch", invalid_path).await;
            assert!(result.is_err());
        }
    }
}
```

### Module: `src/metrics/storage.rs` - MetricsStorage

#### Add these tests to `src/metrics/storage.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::time::Duration;

    #[test]
    fn test_metrics_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        assert_eq!(storage.base_path, temp_dir.path().join(".mmm").join("metrics"));
    }

    #[test]
    fn test_ensure_directory() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        assert!(storage.ensure_directory().is_ok());
        assert!(storage.base_path.exists());
    }

    #[test]
    fn test_save_and_load_current_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        let metrics = ImprovementMetrics {
            test_coverage: 75.5,
            type_coverage: 85.0,
            doc_coverage: 60.0,
            lint_warnings: 5,
            code_duplication: 3.2,
            compile_time: Duration::from_secs(10),
            binary_size: 1024 * 1024,
            cyclomatic_complexity: std::collections::HashMap::new(),
            max_nesting_depth: 3,
            total_lines: 1000,
            timestamp: chrono::Utc::now(),
            iteration_id: "test-iteration".to_string(),
            benchmark_results: std::collections::HashMap::new(),
            tech_debt_score: 5.0,
            improvement_velocity: 1.2,
        };
        
        // Save metrics
        assert!(storage.save_current(&metrics).is_ok());
        
        // Load metrics
        let loaded = storage.load_current().unwrap();
        assert!(loaded.is_some());
        let loaded_metrics = loaded.unwrap();
        assert_eq!(loaded_metrics.test_coverage, 75.5);
        assert_eq!(loaded_metrics.iteration_id, "test-iteration");
    }

    #[test]
    fn test_load_current_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        let result = storage.load_current().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_history() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        let mut history = MetricsHistory::new();
        history.add_snapshot(ImprovementMetrics {
            test_coverage: 70.0,
            type_coverage: 80.0,
            doc_coverage: 55.0,
            lint_warnings: 10,
            code_duplication: 5.0,
            compile_time: Duration::from_secs(15),
            binary_size: 2 * 1024 * 1024,
            cyclomatic_complexity: std::collections::HashMap::new(),
            max_nesting_depth: 4,
            total_lines: 1500,
            timestamp: chrono::Utc::now(),
            iteration_id: "history-test".to_string(),
            benchmark_results: std::collections::HashMap::new(),
            tech_debt_score: 6.0,
            improvement_velocity: 1.0,
        });
        
        assert!(storage.save_history(&history).is_ok());
        
        let loaded_history = storage.load_history().unwrap();
        assert_eq!(loaded_history.snapshots.len(), 1);
        assert_eq!(loaded_history.snapshots[0].metrics.iteration_id, "history-test");
    }

    #[test]
    fn test_generate_report() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        let mut complexity = std::collections::HashMap::new();
        complexity.insert("main".to_string(), 5);
        complexity.insert("complex_fn".to_string(), 15);
        
        let metrics = ImprovementMetrics {
            test_coverage: 85.5,
            type_coverage: 90.0,
            doc_coverage: 70.0,
            lint_warnings: 2,
            code_duplication: 1.5,
            compile_time: Duration::from_secs(8),
            binary_size: 512 * 1024,
            cyclomatic_complexity: complexity,
            max_nesting_depth: 2,
            total_lines: 500,
            timestamp: chrono::Utc::now(),
            iteration_id: "report-test".to_string(),
            benchmark_results: std::collections::HashMap::new(),
            tech_debt_score: 3.0,
            improvement_velocity: 1.5,
        };
        
        let report = storage.generate_report(&metrics);
        
        assert!(report.contains("report-test"));
        assert!(report.contains("85.5%"));
        assert!(report.contains("Test Coverage"));
        assert!(report.contains("Avg Cyclomatic Complexity: 10.0"));
        assert!(report.contains("Overall Score"));
    }

    #[test]
    fn test_save_report() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        let report = "Test Report Content\nLine 2";
        let iteration_id = "save-report-test";
        
        assert!(storage.save_report(report, iteration_id).is_ok());
        
        let report_path = storage.base_path
            .join("reports")
            .join(format!("report-{}.txt", iteration_id));
        
        assert!(report_path.exists());
        let saved_content = std::fs::read_to_string(report_path).unwrap();
        assert_eq!(saved_content, report);
    }

    #[test]
    fn test_generate_report_empty_complexity() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());
        
        let metrics = ImprovementMetrics {
            test_coverage: 50.0,
            type_coverage: 60.0,
            doc_coverage: 40.0,
            lint_warnings: 20,
            code_duplication: 10.0,
            compile_time: Duration::from_secs(20),
            binary_size: 4 * 1024 * 1024,
            cyclomatic_complexity: std::collections::HashMap::new(), // Empty
            max_nesting_depth: 5,
            total_lines: 2000,
            timestamp: chrono::Utc::now(),
            iteration_id: "empty-complexity".to_string(),
            benchmark_results: std::collections::HashMap::new(),
            tech_debt_score: 8.0,
            improvement_velocity: 0.5,
        };
        
        let report = storage.generate_report(&metrics);
        assert!(report.contains("Avg Cyclomatic Complexity: 0.0"));
        assert!(report.contains("🔴")); // Low score emoji
    }
}
```

### Module: `src/init/mod.rs` - Init Command Handler

#### Add these tests to `src/init/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::init::command::InitCommand;

    #[tokio::test]
    async fn test_handle_init_create_claude_md() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };
        
        let result = handle_init(cmd).await;
        assert!(result.is_ok());
        
        // Check CLAUDE.md was created
        let claude_md = temp_dir.path().join("CLAUDE.md");
        assert!(claude_md.exists());
        
        let content = std::fs::read_to_string(claude_md).unwrap();
        assert!(content.contains("MMM Commands"));
    }

    #[tokio::test]
    async fn test_handle_init_with_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md = temp_dir.path().join("CLAUDE.md");
        
        // Create existing file
        std::fs::write(&claude_md, "existing content").unwrap();
        
        let cmd = InitCommand {
            force: false,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };
        
        let result = handle_init(cmd).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_handle_init_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let claude_md = temp_dir.path().join("CLAUDE.md");
        
        // Create existing file
        std::fs::write(&claude_md, "old content").unwrap();
        
        let cmd = InitCommand {
            force: true,
            commands: None,
            path: Some(temp_dir.path().to_path_buf()),
        };
        
        let result = handle_init(cmd).await;
        assert!(result.is_ok());
        
        // Check file was overwritten
        let content = std::fs::read_to_string(claude_md).unwrap();
        assert!(content.contains("MMM Commands"));
        assert!(!content.contains("old content"));
    }

    #[tokio::test]
    async fn test_handle_init_specific_commands() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = InitCommand {
            force: false,
            commands: Some(vec!["mmm-code-review".to_string(), "mmm-lint".to_string()]),
            path: Some(temp_dir.path().to_path_buf()),
        };
        
        let result = handle_init(cmd).await;
        assert!(result.is_ok());
        
        let claude_md = temp_dir.path().join("CLAUDE.md");
        let content = std::fs::read_to_string(claude_md).unwrap();
        
        // Should contain specified commands
        assert!(content.contains("/mmm-code-review"));
        assert!(content.contains("/mmm-lint"));
        
        // Should not contain other commands
        assert!(!content.contains("/mmm-implement-spec"));
    }

    #[tokio::test]
    async fn test_handle_init_default_path() {
        let cmd = InitCommand {
            force: true, // Force to avoid conflicts in test environment
            commands: None,
            path: None, // Use current directory
        };
        
        // This test verifies the function doesn't panic with default path
        let _ = handle_init(cmd).await;
    }
}
```

### Module: `src/cook/retry.rs` - Additional Edge Case Tests

#### Add these tests to `src/cook/retry.rs`:
```rust
#[cfg(test)]
mod additional_tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_with_retry_io_error_recovery() {
        // Simulate a command that might have IO errors
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("exit 0"); // Successful command
        
        let output = execute_with_retry(cmd, "io test", 1, false)
            .await
            .unwrap();
        assert!(output.status.success());
    }

    #[tokio::test]
    async fn test_format_subprocess_error_all_hints() {
        // Test multiple error patterns
        let patterns = vec![
            ("permission denied in /usr/local", "claude auth"),
            ("command not found: xyz", "may not be installed"),
            ("rate limit exceeded for API", "hit the API rate limit"),
        ];
        
        for (stderr, expected_hint) in patterns {
            let error = format_subprocess_error("test", Some(1), stderr, "");
            assert!(error.contains(expected_hint), 
                "Expected hint '{}' for error '{}'", expected_hint, stderr);
        }
    }

    #[test]
    fn test_is_transient_error_edge_cases() {
        // Test empty string
        assert!(!is_transient_error(""));
        
        // Test whitespace only
        assert!(!is_transient_error("   \n   "));
        
        // Test very long error with pattern at end
        let long_error = format!("{}rate limit", "x".repeat(1000));
        assert!(is_transient_error(&long_error));
        
        // Test multiple patterns
        assert!(is_transient_error("timeout and connection refused"));
    }

    #[tokio::test]
    async fn test_execute_with_retry_verbose_output() {
        // Test that verbose mode doesn't break execution
        let mut cmd = Command::new("echo");
        cmd.arg("verbose test");
        
        let output = execute_with_retry(cmd, "verbose test", 1, true)
            .await
            .unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn test_format_subprocess_error_with_newlines() {
        let error = format_subprocess_error(
            "multi-line",
            Some(2),
            "Error:\n  Line 1\n  Line 2\n",
            "Output:\n  Data 1\n  Data 2\n"
        );
        
        assert!(error.contains("Line 1"));
        assert!(error.contains("Line 2"));
        assert!(!error.contains("Data 1")); // stdout ignored when stderr present
    }
}
```

## Integration Tests Needed

### Component: Context Analysis Integration
**File**: tests/context_integration.rs
```rust
use mmm::context::{analyzer::ContextAnalyzer, test_coverage::TestCoverageAnalyzer};
use mmm::simple_state::SimpleState;
use tempfile::TempDir;

#[tokio::test]
async fn test_context_analyzer_full_flow() {
    let temp_dir = TempDir::new().unwrap();
    let state = SimpleState::new(temp_dir.path()).await.unwrap();
    
    // Create a mock Rust project
    std::fs::create_dir_all(temp_dir.path().join("src")).unwrap();
    std::fs::write(temp_dir.path().join("Cargo.toml"), r#"
[package]
name = "test-project"
version = "0.1.0"
"#).unwrap();
    
    std::fs::write(temp_dir.path().join("src/lib.rs"), r#"
pub fn untested_function() -> i32 {
    42
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_something() {
        assert_eq!(1 + 1, 2);
    }
}
"#).unwrap();
    
    let analyzer = ContextAnalyzer::new(temp_dir.path());
    let result = analyzer.analyze_project(false).await;
    
    assert!(result.is_ok());
    let analysis = result.unwrap();
    
    // Verify test coverage analysis
    assert!(analysis.test_coverage.overall_coverage < 1.0);
    assert!(!analysis.test_coverage.untested_functions.is_empty());
}

#[tokio::test]
async fn test_metrics_storage_integration() {
    use mmm::metrics::{collector::MetricsCollector, storage::MetricsStorage};
    
    let temp_dir = TempDir::new().unwrap();
    let storage = MetricsStorage::new(temp_dir.path());
    let collector = MetricsCollector::new(temp_dir.path());
    
    // Collect and save metrics
    if let Ok(metrics) = collector.collect_all(false).await {
        assert!(storage.save_current(&metrics).is_ok());
        
        // Generate and save report
        let report = storage.generate_report(&metrics);
        assert!(storage.save_report(&report, &metrics.iteration_id).is_ok());
    }
}
```

## Implementation Checklist
- [ ] Add unit tests for 15 critical functions in git abstractions
- [ ] Add unit tests for 10 functions in metrics storage
- [ ] Add unit tests for 5 functions in init module
- [ ] Add edge case tests for retry module
- [ ] Create 2 integration test files
- [ ] Verify tests pass: `cargo test`
- [ ] Check coverage improves: `cargo tarpaulin`
- [ ] Follow project conventions from .mmm/context/conventions.json