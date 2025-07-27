# Iteration 1753605002: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.
Focus directive: test coverage

## Issues to Address

### 1. Missing Test Coverage for Error Handling in improve/mod.rs
**Severity**: High
**Category**: Testing
**File**: src/improve/mod.rs
**Line**: 335-528

#### Current Code:
```rust
async fn call_claude_code_review(verbose: bool, focus: Option<&str>) -> Result<bool> {
    println!("🤖 Running /mmm-code-review...");

    // First check if claude command exists with improved error handling
    check_claude_cli().await?;

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions")
        .arg("--print")
        .arg("/mmm-code-review")
        .env("MMM_AUTOMATION", "true");

    // Pass focus directive via environment variable on first iteration
    if let Some(focus_directive) = focus {
        cmd.env("MMM_FOCUS", focus_directive);
    }

    // Execute with retry logic for transient failures
    let output =
        execute_with_retry(cmd, "Claude code review", DEFAULT_CLAUDE_RETRIES, verbose).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = format_subprocess_error(
            "claude /mmm-code-review",
            output.status.code(),
            &stderr,
            &stdout,
        );
        return Err(anyhow!(error_msg));
    }

    if verbose {
        println!("✅ Code review completed");
    }

    Ok(true)
}
```

#### Required Change:
Add comprehensive unit tests for error scenarios:

```rust
#[cfg(test)]
mod claude_integration_tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    #[tokio::test]
    async fn test_call_claude_code_review_success() {
        // Test successful execution path
        // This would require mocking Command execution
    }

    #[tokio::test]
    async fn test_call_claude_code_review_with_focus() {
        // Test that focus directive is passed correctly
    }

    #[tokio::test]
    async fn test_call_claude_code_review_command_not_found() {
        // Test handling when claude CLI is not installed
    }

    #[tokio::test]
    async fn test_call_claude_code_review_auth_failure() {
        // Test handling of authentication failures
    }

    #[tokio::test]
    async fn test_call_claude_code_review_network_timeout() {
        // Test retry logic for network timeouts
    }
}
```

#### Implementation Notes:
- Add tests for all three Claude CLI functions (code_review, implement_spec, lint)
- Mock Command execution to test different failure scenarios
- Test retry logic for transient failures
- Verify environment variables are set correctly

### 2. Missing Integration Tests for Git Operations
**Severity**: High  
**Category**: Testing
**File**: src/improve/git_ops.rs
**Line**: entire module

#### Current Code:
```rust
use anyhow::{Context, Result};
use std::sync::Mutex;
use tokio::process::Command;

static GIT_MUTEX: Mutex<()> = Mutex::new(());

pub async fn get_last_commit_message() -> Result<String> {
    let _lock = GIT_MUTEX.lock().unwrap();
    
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .await
        .context("Failed to execute git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Git log failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

#### Required Change:
Add integration tests for git operations:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command as StdCommand;

    #[tokio::test]
    async fn test_get_last_commit_message_success() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        StdCommand::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to init git repo");

        // Configure git
        StdCommand::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to config email");

        StdCommand::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to config name");

        // Create test commit
        let test_message = "test: sample commit message";
        StdCommand::new("git")
            .args(["commit", "--allow-empty", "-m", test_message])
            .current_dir(repo_path)
            .output()
            .expect("Failed to create commit");

        // Change to test directory
        std::env::set_current_dir(repo_path).unwrap();

        // Test the function
        let result = get_last_commit_message().await.unwrap();
        assert_eq!(result, test_message);
    }

    #[tokio::test]
    async fn test_get_last_commit_message_no_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = get_last_commit_message().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a git repository"));
    }

    #[tokio::test]
    async fn test_mutex_prevents_concurrent_access() {
        // Test that the mutex properly serializes git operations
    }
}
```

#### Implementation Notes:
- Create proper integration tests using temporary git repositories
- Test both success and failure scenarios
- Verify mutex functionality for thread safety
- Test edge cases like empty repositories

### 3. Insufficient Test Coverage for Worktree Operations
**Severity**: Medium
**Category**: Testing
**File**: src/worktree/manager.rs
**Line**: 200-300

#### Current Code:
The merge_session function lacks comprehensive error scenario testing:

```rust
pub fn merge_session(&self, session_name: &str) -> Result<()> {
    // Get default branch
    let default_branch = self.get_default_branch()?;
    
    // Use Claude CLI for merge instead of git merge
    println!("🔄 Using Claude CLI to merge worktree changes...");
    
    let mut cmd = std::process::Command::new("claude");
    cmd.arg("--dangerously-skip-permissions")
        .arg("--print")
        .arg("/mmm-merge-worktree")
        .arg(session_name)
        .arg(&default_branch)
        .env("MMM_AUTOMATION", "true");
    
    let output = cmd
        .output()
        .context("Failed to execute Claude merge command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "Claude merge failed: stderr={}, stdout={}",
            stderr,
            stdout
        ));
    }

    Ok(())
}
```

#### Required Change:
Add comprehensive tests for merge operations:

```rust
#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn test_merge_session_success() {
        // Test successful merge operation
    }

    #[test]
    fn test_merge_session_claude_not_found() {
        // Test when Claude CLI is not available
    }

    #[test]
    fn test_merge_session_conflict_resolution() {
        // Test Claude's conflict resolution capabilities
    }

    #[test]
    fn test_merge_session_invalid_worktree() {
        // Test merging non-existent worktree
    }

    #[test]
    fn test_bulk_merge_partial_failure() {
        // Test --all flag with some failures
    }
}
```

#### Implementation Notes:
- Mock Claude CLI responses for different scenarios
- Test error propagation and recovery
- Verify proper cleanup on failure
- Test bulk operations with mixed success/failure

### 4. Missing Error Handling Tests for Configuration Loading
**Severity**: Medium
**Category**: Testing
**File**: src/config/loader.rs
**Line**: Configuration loading functions

#### Current Code:
The configuration loader lacks tests for malformed configuration files and error scenarios.

#### Required Change:
Add tests for configuration error handling:

```rust
#[cfg(test)]
mod config_error_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_malformed_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".mmm/config.toml");
        
        // Create malformed TOML
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, "invalid toml content {").unwrap();
        
        let loader = ConfigLoader::new().await.unwrap();
        let result = loader.load_with_explicit_path(temp_dir.path(), None).await;
        
        // Should handle gracefully, not panic
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_load_invalid_yaml() {
        // Test YAML parsing errors
    }

    #[tokio::test]
    async fn test_config_precedence() {
        // Test that command-line config overrides defaults
    }
}
```

#### Implementation Notes:
- Test all configuration file formats (TOML, YAML)
- Verify graceful degradation on parse errors
- Test configuration precedence rules
- Ensure no panics on invalid input

### 5. Add Property-Based Tests for Spec ID Validation
**Severity**: Low
**Category**: Testing
**File**: src/improve/mod.rs
**Line**: 436-446

#### Current Code:
The spec ID validation has basic unit tests but could benefit from property-based testing.

#### Required Change:
Add property-based tests using proptest or similar:

```rust
#[cfg(test)]
mod spec_validation_property_tests {
    use super::*;
    // use proptest::prelude::*;

    // proptest! {
    //     #[test]
    //     fn test_spec_id_validation_properties(s: String) {
    //         let is_valid = validate_spec_id(&s);
    //         if is_valid {
    //             assert!(s.starts_with("iteration-"));
    //             assert!(s.ends_with("-improvements"));
    //             assert!(s.len() > 24);
    //         }
    //     }
    // }

    #[test]
    fn test_spec_id_injection_prevention() {
        let injection_attempts = vec![
            "iteration-$(rm -rf /)-improvements",
            "iteration-`cat /etc/passwd`-improvements",
            "iteration-;shutdown -h now;-improvements",
            "iteration-${PATH}-improvements",
        ];
        
        for attempt in injection_attempts {
            assert!(!validate_spec_id(attempt), 
                   "Should reject injection attempt: {}", attempt);
        }
    }
}

fn validate_spec_id(spec_id: &str) -> bool {
    spec_id.starts_with("iteration-") 
        && spec_id.ends_with("-improvements")
        && spec_id.len() > 24
        && spec_id[10..spec_id.len()-13].chars().all(|c| c.is_ascii_digit() || c == '-')
}
```

#### Implementation Notes:
- Add property-based testing for robust validation
- Test against command injection attempts
- Verify all edge cases are handled
- Consider adding proptest as dev dependency

## Success Criteria
- [ ] All Claude CLI integration functions have comprehensive unit tests
- [ ] Git operations have proper integration tests with temporary repositories  
- [ ] Worktree merge operations have error scenario coverage
- [ ] Configuration loading handles malformed files gracefully
- [ ] Spec ID validation is thoroughly tested against injection
- [ ] All tests pass consistently
- [ ] No new clippy warnings introduced
- [ ] Test coverage improves measurably