# Coverage Improvements - Iteration 1754166011

## Overview
Test coverage improvements based on MMM context analysis.
Current coverage: 55.0% → Target: 75.0%

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
    fn test_format_subprocess_error_unauthorized() {
        let error = format_subprocess_error("claude", Some(1), "Error: unauthorized access", "");
        
        assert!(error.contains("Check that you have authenticated"));
        assert!(error.contains("unauthorized access"));
        assert!(error.contains("claude"));
    }
    
    #[test]
    fn test_format_subprocess_error_unauthorized_variations() {
        // Test different unauthorized error messages
        let test_cases = vec![
            ("Error: 401 Unauthorized", "401 Unauthorized"),
            ("API key invalid", "API key"),
            ("Authentication failed", "Authentication"),
        ];
        
        for (stderr, expected) in test_cases {
            let error = format_subprocess_error("claude", Some(1), stderr, "");
            assert!(error.contains(expected));
        }
    }
}
```

### Function: `setup_interrupt_handlers` in src/cook/signal_handler.rs:13
**Criticality**: Medium
**Current Status**: No test coverage

#### Add these tests to src/cook/signal_handler.rs:
```rust
#[cfg(test)]
mod signal_tests {
    use super::*;
    
    #[test]
    fn test_setup_interrupt_handlers() {
        let (_temp_dir, worktree_manager) = create_test_worktree_manager();
        let arc_manager = Arc::new(worktree_manager);
        let session_name = "test-signal-session".to_string();
        
        // Test that setup doesn't panic
        let result = setup_interrupt_handlers(arc_manager.clone(), session_name.clone());
        assert!(result.is_ok());
        
        // Allow time for thread to spawn
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    
    #[test]
    fn test_update_interrupted_state_error_handling() {
        let (_temp_dir, worktree_manager) = create_test_worktree_manager();
        let nonexistent_session = "nonexistent-session";
        
        // Should not panic even if session doesn't exist
        update_interrupted_state(
            &worktree_manager,
            nonexistent_session,
            InterruptionType::UserInterrupt,
        );
    }
}
```

### Function: `AnalysisCoordinator` trait methods in src/cook/analysis/mod.rs
**Criticality**: Medium
**Current Status**: No test coverage for trait implementation

#### Add integration tests to tests/analysis_integration.rs:
```rust
use mmm::cook::analysis::{AnalysisCoordinator, AnalysisRunnerImpl, AnalysisCacheImpl};
use mmm::context::AnalysisResult;
use tempfile::TempDir;

#[tokio::test]
async fn test_analysis_coordinator_full_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let cache = AnalysisCacheImpl::new(temp_dir.path());
    let runner = AnalysisRunnerImpl::new();
    
    // Test analyze_project
    let result = runner.analyze_project(temp_dir.path()).await;
    assert!(result.is_ok());
    
    // Test save_analysis
    let analysis = result.unwrap();
    let save_result = cache.save_analysis(temp_dir.path(), &analysis).await;
    assert!(save_result.is_ok());
    
    // Test get_cached_analysis
    let cached = cache.get_cached_analysis(temp_dir.path()).await;
    assert!(cached.is_ok());
    assert!(cached.unwrap().is_some());
}

#[tokio::test]
async fn test_incremental_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let runner = AnalysisRunnerImpl::new();
    
    // Create test files
    std::fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();
    
    // Test incremental analysis
    let changed_files = vec!["test.rs".to_string()];
    let result = runner.analyze_incremental(temp_dir.path(), &changed_files).await;
    assert!(result.is_ok());
}
```

### Function: `SessionManagerAdapter` methods in src/cook/session/adapter.rs
**Criticality**: Medium
**Current Status**: No test coverage for adapter functionality

#### Add these tests to src/cook/session/adapter.rs:
```rust
#[cfg(test)]
mod adapter_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_complete_session_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        
        // Start session
        adapter.start_session("lifecycle-test").await.unwrap();
        
        // Perform multiple operations
        for i in 0..3 {
            adapter.update_session(SessionUpdate::IncrementIteration).await.unwrap();
            adapter.update_session(SessionUpdate::AddFilesChanged(i + 1)).await.unwrap();
        }
        
        // Update status to completed
        adapter.update_session(SessionUpdate::UpdateStatus(SessionStatus::Completed))
            .await.unwrap();
        
        // Complete and verify
        let summary = adapter.complete_session().await.unwrap();
        assert!(summary.iterations > 0);
        assert_eq!(summary.files_changed, 6); // 1 + 2 + 3
    }
    
    #[tokio::test]
    async fn test_save_and_load_state() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        let state_path = temp_dir.path().join("state.json");
        
        // Start session and save state
        adapter.start_session("save-test").await.unwrap();
        adapter.save_state(&state_path).await.unwrap();
        
        // Verify file exists
        assert!(state_path.exists());
        
        // Load state (currently no-op but should not error)
        adapter.load_state(&state_path).await.unwrap();
    }
}
```

### Function: `ComplexityCalculator::calculate` in src/metrics/complexity.rs:29
**Criticality**: Medium
**Current Status**: No test coverage

#### Add these tests to src/metrics/complexity.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_calculate_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        let calculator = ComplexityCalculator::new();
        
        let metrics = calculator.calculate(temp_dir.path()).unwrap();
        assert_eq!(metrics.total_lines, 0);
        assert!(metrics.cyclomatic_complexity.is_empty());
    }
    
    #[test]
    fn test_calculate_with_source_files() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        
        // Create test file with known complexity
        let test_code = r#"
fn simple_function() {
    println!("Hello");
}

fn complex_function(x: i32) {
    if x > 0 {
        println!("Positive");
    } else if x < 0 {
        println!("Negative");
    } else {
        println!("Zero");
    }
}
"#;
        std::fs::write(src_dir.join("test.rs"), test_code).unwrap();
        
        let calculator = ComplexityCalculator::new();
        let metrics = calculator.calculate(temp_dir.path()).unwrap();
        
        assert!(metrics.total_lines > 0);
        assert!(!metrics.cyclomatic_complexity.is_empty());
        assert!(metrics.cyclomatic_complexity.values().any(|&v| v > 1));
    }
}
```

## Integration Tests Needed

### Component: cook/orchestrator integration
**File**: tests/cook_orchestrator_integration.rs
```rust
use mmm::cook::orchestrator::{CookOrchestrator, OrchestratorConfig};
use mmm::session::InMemorySessionManager;
use mmm::subprocess::SubprocessManager;
use mmm::worktree::WorktreeManager;
use tempfile::TempDir;

#[tokio::test]
async fn test_orchestrator_full_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let subprocess = SubprocessManager::mock();
    let session_manager = InMemorySessionManager::new(None);
    
    let config = OrchestratorConfig {
        working_dir: temp_dir.path().to_path_buf(),
        subprocess: subprocess.clone(),
        session_manager: Box::new(session_manager),
        verbose: false,
        dry_run: false,
    };
    
    let orchestrator = CookOrchestrator::new(config);
    
    // Test basic orchestration
    let result = orchestrator.validate_environment().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_orchestrator_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let subprocess = SubprocessManager::mock();
    
    // Configure mock to simulate failures
    subprocess.runner().configure_mock(|mock| {
        mock.expect_command("claude", vec!["--version"])
            .returns_error("Command not found");
    });
    
    let session_manager = InMemorySessionManager::new(None);
    let config = OrchestratorConfig {
        working_dir: temp_dir.path().to_path_buf(),
        subprocess,
        session_manager: Box::new(session_manager),
        verbose: false,
        dry_run: false,
    };
    
    let orchestrator = CookOrchestrator::new(config);
    let result = orchestrator.validate_environment().await;
    assert!(result.is_err());
}
```

### Component: metrics/collector integration
**File**: tests/metrics_collector_integration.rs
```rust
use mmm::metrics::{MetricsCollector, MetricsRegistry};
use mmm::cook::metrics::collector::CookMetricsCollector;
use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_collection_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let registry = MetricsRegistry::new(temp_dir.path()).unwrap();
    let collector = CookMetricsCollector::new(registry.clone());
    
    // Start collection
    collector.start_collection("test-iteration").await.unwrap();
    
    // Record various metrics
    collector.record_command_execution("test-cmd", true).await.unwrap();
    collector.record_file_change("src/main.rs").await.unwrap();
    
    // Complete collection
    let metrics = collector.complete_collection().await.unwrap();
    assert!(metrics.overall_score() > 0.0);
}
```

## Error Path Tests Needed

### Error handling in subprocess/git.rs
```rust
#[cfg(test)]
mod git_error_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_git_command_failure() {
        let mock_runner = MockProcessRunner::new();
        mock_runner.expect_command("git", vec!["status"])
            .returns_error("fatal: not a git repository");
        
        let git = GitSubprocess::new(Arc::new(mock_runner));
        let result = git.status().await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a git repository"));
    }
    
    #[tokio::test]
    async fn test_git_parse_errors() {
        let mock_runner = MockProcessRunner::new();
        mock_runner.expect_command("git", vec!["log"])
            .returns_output("invalid log format");
        
        let git = GitSubprocess::new(Arc::new(mock_runner));
        let result = git.log(10).await;
        
        assert!(result.is_err());
    }
}
```

## Implementation Checklist
- [ ] Add unit tests for 1 critical function (High priority)
- [ ] Add unit tests for 20 medium priority functions
- [ ] Create 3 integration test files
- [ ] Add error path tests for Result-returning functions
- [ ] Verify tests pass: `cargo test`
- [ ] Check coverage improves: `cargo tarpaulin`
- [ ] Follow project conventions from .mmm/context/conventions.json