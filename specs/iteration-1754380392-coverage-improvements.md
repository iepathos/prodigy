# Coverage Improvements - Iteration 1754380392

## Overview
Test coverage improvements based on MMM context analysis.
Current coverage: 57.9% → Target: 75%

## Critical Functions Needing Tests

### Function: `execute_with_subprocess` in src/analyze/command.rs:53
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/analyze/command.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::subprocess::SubprocessManager;
    use crate::abstractions::claude::MockClaudeClient;
    
    #[tokio::test]
    async fn test_execute_with_subprocess_success() {
        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::mock();
        let claude = MockClaudeClient::new();
        claude.add_success_response("/mmm-analyze-context", "Analysis complete");
        
        let args = AnalyzeArgs {
            command: AnalyzeCommand::Context,
            save: false,
            show: false,
            incremental: false,
        };
        
        let result = execute_with_subprocess(args, &subprocess, &claude).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_execute_with_subprocess_error_cases() {
        let subprocess = SubprocessManager::mock();
        let claude = MockClaudeClient::new();
        claude.add_error_response("/mmm-analyze-context", "Analysis failed");
        
        let args = AnalyzeArgs {
            command: AnalyzeCommand::Context,
            save: false,
            show: false,
            incremental: false,
        };
        
        let result = execute_with_subprocess(args, &subprocess, &claude).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Analysis failed"));
    }
}
```

### Function: `get_claude_api_key` in src/config/mod.rs:131
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/config/mod.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;
    
    #[test]
    fn test_get_claude_api_key_from_env() {
        env::set_var("ANTHROPIC_API_KEY", "test-key-from-env");
        let config = Config::default();
        
        let result = config.get_claude_api_key();
        assert_eq!(result, Some("test-key-from-env".to_string()));
        
        env::remove_var("ANTHROPIC_API_KEY");
    }
    
    #[test]
    fn test_get_claude_api_key_from_config() {
        let config = Config {
            global: GlobalConfig {
                claude_api_key: Some("test-key-from-config".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let result = config.get_claude_api_key();
        assert_eq!(result, Some("test-key-from-config".to_string()));
    }
    
    #[test]
    fn test_get_claude_api_key_config_precedence_over_env() {
        env::set_var("ANTHROPIC_API_KEY", "env-key");
        let config = Config {
            global: GlobalConfig {
                claude_api_key: Some("config-key".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let result = config.get_claude_api_key();
        assert_eq!(result, Some("config-key".to_string()));
        
        env::remove_var("ANTHROPIC_API_KEY");
    }
    
    #[test]
    fn test_get_claude_api_key_none() {
        env::remove_var("ANTHROPIC_API_KEY");
        let config = Config::default();
        
        let result = config.get_claude_api_key();
        assert_eq!(result, None);
    }
}
```

### Function: `save_analysis` in src/context/mod.rs:377
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/context/mod.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::context::architecture::{ArchitectureInfo, ComponentInfo};
    use crate::context::test_coverage::TestCoverageInfo;
    
    #[tokio::test]
    async fn test_save_analysis_success() {
        let temp_dir = TempDir::new().unwrap();
        let analysis = AnalysisResult {
            dependency_graph: DependencyGraphSummary::default(),
            architecture: ArchitectureInfo::default(),
            conventions: FileConventions::default(),
            technical_debt: TechnicalDebtSummary::default(),
            test_coverage: TestCoverageInfo::default(),
            metadata: AnalysisMetadata {
                timestamp: chrono::Utc::now(),
                duration_ms: 1000,
                files_analyzed: 100,
                incremental: false,
                version: "1.0".to_string(),
            },
        };
        
        let result = save_analysis(temp_dir.path(), &analysis).await;
        assert!(result.is_ok());
        
        // Verify files were created
        let context_dir = temp_dir.path().join(".mmm/context");
        assert!(context_dir.join("analysis.json").exists());
        assert!(context_dir.join("analysis_metadata.json").exists());
    }
    
    #[tokio::test]
    async fn test_save_analysis_with_invalid_path() {
        let analysis = AnalysisResult::default();
        let invalid_path = Path::new("/invalid/path/that/does/not/exist");
        
        let result = save_analysis(invalid_path, &analysis).await;
        assert!(result.is_err());
    }
}
```

### Function: `create_orchestrator` in src/cook/mod.rs:118
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/cook/mod.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestContext;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_create_orchestrator_default() {
        let ctx = TestContext::new();
        let config = Config::default();
        let args = CookArgs::default();
        
        let result = create_orchestrator(&ctx.subprocess, &config, &args).await;
        assert!(result.is_ok());
        
        let orchestrator = result.unwrap();
        assert!(orchestrator.workflow_config.is_some());
    }
    
    #[tokio::test]
    async fn test_create_orchestrator_with_mmm_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mmm_dir = temp_dir.path().join(".mmm");
        fs::create_dir_all(&mmm_dir).unwrap();
        
        let ctx = TestContext::new();
        let config = Config::default();
        let args = CookArgs::default();
        
        std::env::set_current_dir(temp_dir.path()).unwrap();
        let result = create_orchestrator(&ctx.subprocess, &config, &args).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_orchestrator_error_cases() {
        let ctx = TestContext::new();
        let config = Config::default();
        let args = CookArgs {
            claude_api_key: Some("invalid-key".to_string()),
            ..Default::default()
        };
        
        // Test with invalid claude client setup
        ctx.subprocess.claude().is_available(false);
        let result = create_orchestrator(&ctx.subprocess, &config, &args).await;
        assert!(result.is_err());
    }
}
```

### Function: `run_improvement_loop` in src/cook/mod.rs:279
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/cook/mod.rs:
```rust
#[tokio::test]
async fn test_run_improvement_loop_success() {
    let ctx = TestContext::new();
    let config = Config::default();
    let args = CookArgs {
        iterations: 1,
        ..Default::default()
    };
    
    // Setup mock responses
    ctx.subprocess.claude().add_success_response("/mmm-code-review", "No issues found");
    ctx.subprocess.claude().add_success_response("/mmm-implement-spec", "Implementation complete");
    ctx.subprocess.claude().add_success_response("/mmm-lint", "Code cleaned");
    
    let orchestrator = create_orchestrator(&ctx.subprocess, &config, &args).await.unwrap();
    let result = run_improvement_loop(orchestrator, args, &ctx.subprocess).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_improvement_loop_error_handling() {
    let ctx = TestContext::new();
    let config = Config::default();
    let args = CookArgs {
        iterations: 1,
        ..Default::default()
    };
    
    // Setup error response
    ctx.subprocess.claude().add_error_response("/mmm-code-review", "Review failed");
    
    let orchestrator = create_orchestrator(&ctx.subprocess, &config, &args).await.unwrap();
    let result = run_improvement_loop(orchestrator, args, &ctx.subprocess).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Review failed"));
}
```

### Function: `execute_structured_workflow` in src/cook/orchestrator.rs:502
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/cook/orchestrator.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestContext;
    use crate::config::command::WorkflowConfig;
    
    #[tokio::test]
    async fn test_execute_structured_workflow_success() {
        let ctx = TestContext::new();
        let workflow = WorkflowConfig {
            name: "test-workflow".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "step1".to_string(),
                    command: "/mmm-lint".to_string(),
                    args: None,
                    exit_on_failure: false,
                },
            ],
        };
        
        ctx.subprocess.claude().add_success_response("/mmm-lint", "Code cleaned");
        
        let orchestrator = CookOrchestrator::new(
            "test-session",
            ctx.subprocess.clone(),
            Some(workflow),
            None,
        );
        
        let result = orchestrator.execute_structured_workflow(&workflow).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_execute_structured_workflow_with_args() {
        let ctx = TestContext::new();
        let workflow = WorkflowConfig {
            name: "test-workflow".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "analyze".to_string(),
                    command: "/mmm-analyze".to_string(),
                    args: Some("$1".to_string()),
                    exit_on_failure: true,
                },
            ],
        };
        
        ctx.subprocess.claude().add_success_response("/mmm-analyze src/main.rs", "Analysis complete");
        
        let mut orchestrator = CookOrchestrator::new(
            "test-session",
            ctx.subprocess.clone(),
            Some(workflow.clone()),
            None,
        );
        orchestrator.workflow_args = vec!["src/main.rs".to_string()];
        
        let result = orchestrator.execute_structured_workflow(&workflow).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_execute_structured_workflow_exit_on_failure() {
        let ctx = TestContext::new();
        let workflow = WorkflowConfig {
            name: "test-workflow".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "failing-step".to_string(),
                    command: "/mmm-test".to_string(),
                    args: None,
                    exit_on_failure: true,
                },
                WorkflowStep {
                    name: "should-not-run".to_string(),
                    command: "/mmm-lint".to_string(),
                    args: None,
                    exit_on_failure: false,
                },
            ],
        };
        
        ctx.subprocess.claude().add_error_response("/mmm-test", "Tests failed");
        
        let orchestrator = CookOrchestrator::new(
            "test-session",
            ctx.subprocess.clone(),
            Some(workflow.clone()),
            None,
        );
        
        let result = orchestrator.execute_structured_workflow(&workflow).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Tests failed"));
    }
}
```

### Function: `update_checkpoint` in src/worktree/manager.rs:559
**Criticality**: High
**Current Status**: No test coverage

#### Add these tests to src/worktree/manager.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::testing::TestContext;
    
    #[tokio::test]
    async fn test_update_checkpoint_success() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = TestContext::new();
        let manager = WorktreeManager::new(ctx.subprocess.clone());
        
        // Create initial session
        let session = manager.create_session(
            "test-session",
            temp_dir.path(),
            5,
            false,
        ).await.unwrap();
        
        // Update checkpoint
        let result = manager.update_checkpoint(
            &session.session_id,
            "/mmm-code-review",
            "review-spec-123",
            &["src/main.rs".to_string()],
        ).await;
        
        assert!(result.is_ok());
        
        // Verify state was updated
        let state = manager.load_state(temp_dir.path()).await.unwrap();
        assert_eq!(state.last_checkpoint.unwrap().iteration, 1);
        assert_eq!(state.last_checkpoint.unwrap().last_command, "/mmm-code-review");
    }
    
    #[tokio::test]
    async fn test_update_checkpoint_increments_iteration() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = TestContext::new();
        let manager = WorktreeManager::new(ctx.subprocess.clone());
        
        let session = manager.create_session(
            "test-session",
            temp_dir.path(),
            5,
            false,
        ).await.unwrap();
        
        // First checkpoint
        manager.update_checkpoint(
            &session.session_id,
            "/mmm-code-review",
            "spec-1",
            &[],
        ).await.unwrap();
        
        // Second checkpoint
        manager.update_checkpoint(
            &session.session_id,
            "/mmm-implement-spec",
            "spec-2",
            &["src/lib.rs".to_string()],
        ).await.unwrap();
        
        let state = manager.load_state(temp_dir.path()).await.unwrap();
        assert_eq!(state.last_checkpoint.unwrap().iteration, 2);
        assert_eq!(state.iterations.completed, 2);
    }
    
    #[tokio::test]
    async fn test_update_checkpoint_error_cases() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = TestContext::new();
        let manager = WorktreeManager::new(ctx.subprocess.clone());
        
        // Try to update checkpoint for non-existent session
        let result = manager.update_checkpoint(
            "non-existent-session",
            "/mmm-code-review",
            "spec-123",
            &[],
        ).await;
        
        assert!(result.is_err());
    }
}
```

### Function: `main` in src/main.rs:148
**Criticality**: High
**Current Status**: No test coverage

#### Add integration tests to tests/main_integration.rs:
```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_main_help_command() {
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("memento-mori"));
}

#[test]
fn test_main_version_command() {
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_main_invalid_command() {
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}
```

## Integration Tests Needed

### Component: cook
**File**: tests/cook_integration.rs
```rust
use mmm::cook::*;
use mmm::testing::TestContext;
use tempfile::TempDir;

#[tokio::test]
async fn test_cook_workflow_integration() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = TestContext::new();
    
    // Setup test project
    ctx.create_test_file(&temp_dir, "src/main.rs", "fn main() { println!(\"Hello\"); }");
    ctx.create_test_file(&temp_dir, "Cargo.toml", "[package]\nname = \"test\"\nversion = \"0.1.0\"");
    
    // Mock Claude responses
    ctx.subprocess.claude().add_success_response("/mmm-code-review", "Found 1 issue: missing documentation");
    ctx.subprocess.claude().add_success_response("/mmm-implement-spec", "Added documentation");
    ctx.subprocess.claude().add_success_response("/mmm-lint", "Code cleaned");
    
    // Run cook workflow
    let args = CookArgs {
        iterations: 1,
        worktree: false,
        metrics: true,
        ..Default::default()
    };
    
    std::env::set_current_dir(&temp_dir).unwrap();
    let result = run(args).await;
    
    assert!(result.is_ok());
    assert!(temp_dir.path().join(".mmm/metrics/current.json").exists());
}
```

### Component: worktree
**File**: tests/worktree_integration.rs
```rust
use mmm::worktree::*;
use mmm::testing::TestContext;
use tempfile::TempDir;

#[tokio::test]
async fn test_worktree_full_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = TestContext::new();
    
    // Setup git repo
    ctx.subprocess.git().is_repo(true);
    ctx.subprocess.git().add_response("worktree list", temp_dir.path().to_str().unwrap());
    
    let manager = WorktreeManager::new(ctx.subprocess.clone());
    
    // Create worktree session
    let session = manager.create_session(
        "test-feature",
        temp_dir.path(),
        3,
        false,
    ).await.unwrap();
    
    assert_eq!(session.worktree_name, "mmm-test-feature");
    assert_eq!(session.status, "in_progress");
    
    // Update checkpoint
    manager.update_checkpoint(
        &session.session_id,
        "/mmm-code-review",
        "spec-123",
        &["src/main.rs".to_string()],
    ).await.unwrap();
    
    // Complete session
    let result = manager.complete_session(&session.session_id).await;
    assert!(result.is_ok());
}
```

### Component: context
**File**: tests/context_analysis_integration.rs
```rust
use mmm::context::*;
use mmm::testing::TestContext;
use tempfile::TempDir;

#[tokio::test]
async fn test_context_analysis_full_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = TestContext::new();
    
    // Create test files
    ctx.create_test_files(&temp_dir, vec![
        ("src/main.rs", "fn main() {}"),
        ("src/lib.rs", "pub fn helper() {}"),
        ("tests/test.rs", "#[test] fn test_helper() {}"),
    ]);
    
    // Run analysis
    let analyzer = StandardAnalyzer::new(temp_dir.path());
    let result = analyzer.analyze_all(false).await.unwrap();
    
    assert!(result.metadata.files_analyzed > 0);
    assert!(result.test_coverage.overall_coverage >= 0.0);
    
    // Save analysis
    save_analysis(temp_dir.path(), &result).await.unwrap();
    
    // Load and verify
    let loaded = load_analysis(temp_dir.path()).await.unwrap();
    assert_eq!(loaded.metadata.files_analyzed, result.metadata.files_analyzed);
}
```

## Implementation Checklist
- [ ] Add unit tests for 55 critical functions
- [ ] Create 3 integration test files
- [ ] Verify tests pass: `cargo test`
- [ ] Check coverage improves: `cargo tarpaulin`
- [ ] Follow project conventions from .mmm/context/conventions.json