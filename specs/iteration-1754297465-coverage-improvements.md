# Coverage Improvements - Iteration 1754297465

## Overview
Test coverage improvements based on MMM context analysis.
Current coverage: 58.28% → Target: 75%

## Critical Functions Needing Tests

### Function: `test_format_subprocess_error_unauthorized` in src/cook/retry.rs:351
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/cook/retry.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_subprocess_error_unauthorized_success() {
        // Test formatting of unauthorized error messages
        let error_output = "error: Authentication failed for 'https://github.com/user/repo.git'";
        let formatted = format_subprocess_error("git", error_output, 1);
        assert!(formatted.contains("Authentication failed"));
        assert!(formatted.contains("unauthorized"));
    }
    
    #[test]
    fn test_format_subprocess_error_unauthorized_variations() {
        // Test various unauthorized error formats
        let test_cases = vec![
            ("remote: Invalid username or password", true),
            ("fatal: Authentication failed", true),
            ("Permission denied (publickey)", true),
            ("Normal error message", false),
        ];
        
        for (error_msg, should_be_unauthorized) in test_cases {
            let formatted = format_subprocess_error("git", error_msg, 1);
            assert_eq!(
                formatted.contains("unauthorized"),
                should_be_unauthorized,
                "Failed for: {}", error_msg
            );
        }
    }
}
```

### Function: `with_subprocess` in src/abstractions/claude.rs:311
**Criticality**: Medium
**Current Status**: No test coverage

#### Add these tests to src/abstractions/claude.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use subprocess::{SubprocessLimits, MockProcessRunner};
    
    #[tokio::test]
    async fn test_with_subprocess_success() {
        // Test successful subprocess execution
        let mut mock_runner = MockProcessRunner::new();
        mock_runner.expect_run()
            .returning(|_, _, _| Ok("test output".to_string()));
        
        let command = CommandContext {
            subprocess_runner: Some(Box::new(mock_runner)),
            limits: SubprocessLimits::default(),
            // ... other fields
        };
        
        let result = command.with_subprocess("test", &["arg"]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test output");
    }
    
    #[tokio::test]
    async fn test_with_subprocess_error_cases() {
        // Test subprocess failure scenarios
        let mut mock_runner = MockProcessRunner::new();
        mock_runner.expect_run()
            .returning(|_, _, _| Err(anyhow::anyhow!("Process failed")));
        
        let command = CommandContext {
            subprocess_runner: Some(Box::new(mock_runner)),
            limits: SubprocessLimits::default(),
            // ... other fields
        };
        
        let result = command.with_subprocess("failing", &["cmd"]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Process failed"));
    }
}
```

### Function: `execute_with_subprocess` in src/analyze/command.rs:53
**Criticality**: Medium
**Current Status**: No test coverage

#### Add these tests to src/analyze/command.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use subprocess::MockProcessRunner;
    
    #[tokio::test]
    async fn test_execute_with_subprocess_success() {
        // Test successful command execution
        let mut mock_runner = MockProcessRunner::new();
        mock_runner.expect_run()
            .returning(|cmd, args, _| {
                assert_eq!(cmd, "cargo");
                assert!(args.contains(&"test".to_string()));
                Ok("All tests passed".to_string())
            });
        
        let result = execute_with_subprocess(Box::new(mock_runner), "cargo", &["test"]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "All tests passed");
    }
    
    #[tokio::test]
    async fn test_execute_with_subprocess_error_cases() {
        // Test command execution failures
        let mut mock_runner = MockProcessRunner::new();
        mock_runner.expect_run()
            .returning(|_, _, _| Err(anyhow::anyhow!("Command not found")));
        
        let result = execute_with_subprocess(Box::new(mock_runner), "invalid", &["cmd"]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Command not found"));
    }
}
```

### Function: `validate_command` in src/config/command_validator.rs:226
**Criticality**: Medium
**Current Status**: No test coverage

#### Add these tests to src/config/command_validator.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Command, Argument, Option};
    
    #[test]
    fn test_validate_command_success() {
        // Test valid command validation
        let command = Command {
            name: "test-command".to_string(),
            description: "Test command".to_string(),
            command: "echo {{arg}}".to_string(),
            arguments: vec![
                Argument {
                    name: "arg".to_string(),
                    description: "Test argument".to_string(),
                    required: true,
                    arg_type: Some("string".to_string()),
                    default: None,
                }
            ],
            options: vec![],
        };
        
        let result = validate_command(&command);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_command_error_cases() {
        // Test invalid command scenarios
        let invalid_cases = vec![
            // Empty command
            Command {
                name: "test".to_string(),
                description: "Test".to_string(),
                command: "".to_string(),
                arguments: vec![],
                options: vec![],
            },
            // Invalid argument type
            Command {
                name: "test".to_string(),
                description: "Test".to_string(),
                command: "echo test".to_string(),
                arguments: vec![
                    Argument {
                        name: "arg".to_string(),
                        description: "Test".to_string(),
                        required: true,
                        arg_type: Some("invalid_type".to_string()),
                        default: None,
                    }
                ],
                options: vec![],
            },
        ];
        
        for invalid_command in invalid_cases {
            let result = validate_command(&invalid_command);
            assert!(result.is_err());
        }
    }
}
```

### Function: `update_architecture` in src/context/architecture.rs:379
**Criticality**: Medium
**Current Status**: No test coverage

#### Add these tests to src/context/architecture.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_update_architecture_success() {
        // Test architecture update functionality
        let temp_dir = TempDir::new().unwrap();
        let context_dir = temp_dir.path().join(".mmm/context");
        std::fs::create_dir_all(&context_dir).unwrap();
        
        let architecture = Architecture {
            patterns: vec!["MVC".to_string()],
            layers: vec![],
            components: HashMap::new(),
            violations: vec![],
        };
        
        let result = update_architecture(&context_dir, architecture).await;
        assert!(result.is_ok());
        
        // Verify file was created
        let arch_file = context_dir.join("architecture.json");
        assert!(arch_file.exists());
    }
    
    #[tokio::test]
    async fn test_update_architecture_error_cases() {
        // Test architecture update failures
        let invalid_path = "/invalid/path/that/does/not/exist";
        let architecture = Architecture::default();
        
        let result = update_architecture(Path::new(invalid_path), architecture).await;
        assert!(result.is_err());
    }
}
```

## Integration Tests Needed

### Component: worktree
**File**: tests/worktree_integration.rs
```rust
use mmm::worktree::*;
use mmm::git::{GitOperations, MockGitOperations};
use tempfile::TempDir;

#[tokio::test]
async fn test_worktree_session_lifecycle() {
    // Test complete worktree session lifecycle
    let temp_dir = TempDir::new().unwrap();
    let mut mock_git = MockGitOperations::new();
    
    // Setup expectations
    mock_git.expect_create_worktree()
        .returning(|_, _| Ok(()));
    mock_git.expect_remove_worktree()
        .returning(|_| Ok(()));
    
    // Create session
    let session = WorktreeSession::new("test-session")
        .with_git(Box::new(mock_git));
    
    let result = session.create(temp_dir.path()).await;
    assert!(result.is_ok());
    
    // Verify session state
    assert_eq!(session.status(), "in_progress");
    
    // Cleanup
    let cleanup_result = session.cleanup().await;
    assert!(cleanup_result.is_ok());
}

#[tokio::test]
async fn test_worktree_error_handling() {
    // Test worktree error scenarios
    let mut mock_git = MockGitOperations::new();
    mock_git.expect_create_worktree()
        .returning(|_, _| Err(anyhow::anyhow!("Git error")));
    
    let session = WorktreeSession::new("failing-session")
        .with_git(Box::new(mock_git));
    
    let result = session.create("/tmp").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Git error"));
}
```

### Component: metrics
**File**: tests/metrics_integration.rs
```rust
use mmm::metrics::*;
use mmm::context::ContextAnalyzer;
use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_collection_integration() {
    // Test integrated metrics collection
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();
    
    // Setup test project structure
    std::fs::write(project_dir.join("Cargo.toml"), r#"
        [package]
        name = "test"
        version = "0.1.0"
    "#).unwrap();
    
    std::fs::write(project_dir.join("src/main.rs"), r#"
        fn main() {
            println!("Hello, world!");
        }
    "#).unwrap();
    
    // Run metrics collection
    let metrics = ImprovementMetrics::new("test-iteration");
    let collected = metrics.collect(project_dir).await;
    
    assert!(collected.is_ok());
    let result = collected.unwrap();
    
    // Verify metrics were collected
    assert!(result.test_coverage >= 0.0);
    assert!(result.lint_warnings >= 0);
    assert!(result.total_lines > 0);
}

#[tokio::test]
async fn test_metrics_comparison() {
    // Test metrics comparison functionality
    let baseline = ImprovementMetrics {
        test_coverage: 50.0,
        lint_warnings: 10,
        code_duplication: 5.0,
        ..Default::default()
    };
    
    let current = ImprovementMetrics {
        test_coverage: 65.0,
        lint_warnings: 5,
        code_duplication: 3.0,
        ..Default::default()
    };
    
    let comparison = MetricsComparison::compare(&baseline, &current);
    
    assert_eq!(comparison.coverage_change, 15.0);
    assert_eq!(comparison.warnings_change, -5);
    assert_eq!(comparison.duplication_change, -2.0);
    assert!(comparison.is_improvement());
}
```

### Component: git
**File**: tests/git_operations_integration.rs
```rust
use mmm::git::*;
use tempfile::TempDir;
use std::process::Command;

#[tokio::test]
async fn test_git_operations_integration() {
    // Test real git operations (requires git)
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    // Initialize git repo
    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");
    
    let git_ops = GitCommandRunner::new();
    
    // Test status
    let status = git_ops.status(repo_path).await;
    assert!(status.is_ok());
    
    // Create test file
    std::fs::write(repo_path.join("test.txt"), "test content").unwrap();
    
    // Test add
    let add_result = git_ops.add(repo_path, &["test.txt"]).await;
    assert!(add_result.is_ok());
    
    // Test commit
    let commit_result = git_ops.commit(
        repo_path,
        "test: initial commit"
    ).await;
    assert!(commit_result.is_ok());
}

#[tokio::test]
async fn test_git_worktree_operations() {
    // Test git worktree functionality
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    // Setup git repo with initial commit
    Command::new("git").args(&["init"]).current_dir(repo_path).output().unwrap();
    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    Command::new("git").args(&["add", "."]).current_dir(repo_path).output().unwrap();
    Command::new("git").args(&["commit", "-m", "Initial"]).current_dir(repo_path).output().unwrap();
    
    let git_ops = GitCommandRunner::new();
    
    // Create worktree
    let worktree_path = temp_dir.path().join("worktree");
    let create_result = git_ops.create_worktree(
        repo_path,
        &worktree_path,
        "test-branch"
    ).await;
    assert!(create_result.is_ok());
    assert!(worktree_path.exists());
    
    // List worktrees
    let list_result = git_ops.list_worktrees(repo_path).await;
    assert!(list_result.is_ok());
    assert!(list_result.unwrap().len() >= 2); // main + worktree
    
    // Remove worktree
    let remove_result = git_ops.remove_worktree(repo_path, "worktree").await;
    assert!(remove_result.is_ok());
}
```

## Implementation Checklist
- [ ] Add unit tests for 2 critical functions
- [ ] Add unit tests for 18 medium priority functions
- [ ] Create 3 integration test files
- [ ] Verify tests pass: `cargo test`
- [ ] Check coverage improves: `cargo tarpaulin`
- [ ] Follow project conventions from .mmm/context/conventions.json