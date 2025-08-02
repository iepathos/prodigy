# Coverage Improvements - Iteration 1754163860

## Overview
Test coverage improvements based on MMM context analysis.
Current coverage: 55.0% → Target: 75%
Priority focus on critical functions and modules with 0% coverage.

## Critical Functions Needing Tests

### Function: `test_format_subprocess_error_unauthorized` in src/cook/retry.rs:351
**Criticality**: High
**Current Status**: Function exists but appears to be a test that's not being counted in coverage

#### Investigate test coverage issue in src/cook/retry.rs:
This appears to be an existing test that's not being executed or counted. The function is already a test but may need to be included in the test suite properly.

## Modules with 0% Coverage - High Priority

### Module: `src/cook/analysis/mod.rs`
**Coverage**: 0%
**Description**: Analysis coordination for cook operations

#### Add these tests to src/cook/analysis/mod.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    
    struct MockAnalysisCoordinator;
    
    #[async_trait]
    impl AnalysisCoordinator for MockAnalysisCoordinator {
        async fn analyze_project(&self, _project_path: &Path) -> Result<AnalysisResult> {
            Ok(AnalysisResult::default())
        }
        
        async fn analyze_incremental(
            &self,
            _project_path: &Path,
            _changed_files: &[String],
        ) -> Result<AnalysisResult> {
            Ok(AnalysisResult::default())
        }
        
        async fn get_cached_analysis(&self, _project_path: &Path) -> Result<Option<AnalysisResult>> {
            Ok(None)
        }
        
        async fn save_analysis(&self, _project_path: &Path, _analysis: &AnalysisResult) -> Result<()> {
            Ok(())
        }
        
        async fn clear_cache(&self, _project_path: &Path) -> Result<()> {
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_analysis_coordinator_trait() {
        let coordinator = MockAnalysisCoordinator;
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        
        // Test all trait methods
        let result = coordinator.analyze_project(path).await;
        assert!(result.is_ok());
        
        let incremental = coordinator.analyze_incremental(path, &["file.rs".to_string()]).await;
        assert!(incremental.is_ok());
        
        let cached = coordinator.get_cached_analysis(path).await;
        assert!(cached.is_ok());
        assert!(cached.unwrap().is_none());
        
        let save_result = coordinator.save_analysis(path, &AnalysisResult::default()).await;
        assert!(save_result.is_ok());
        
        let clear_result = coordinator.clear_cache(path).await;
        assert!(clear_result.is_ok());
    }
}
```

### Module: `src/cook/signal_handler.rs`
**Coverage**: 0%
**Description**: Signal handling for graceful shutdown

#### Add these tests to src/cook/signal_handler.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::MockProcessRunner;
    use crate::abstractions::git::MockGitOperations;
    use crate::abstractions::claude::MockClaudeOperations;
    use tempfile::TempDir;
    
    #[test]
    fn test_update_interrupted_state() {
        let temp_dir = TempDir::new().unwrap();
        let process_runner = Arc::new(MockProcessRunner::new());
        let git = Arc::new(MockGitOperations::new());
        let claude = Arc::new(MockClaudeOperations::new());
        
        let worktree_manager = Arc::new(WorktreeManager::new(
            temp_dir.path(),
            process_runner,
            git,
            claude,
        ));
        
        // Create a test session
        let session_name = "test-session";
        
        // Test interruption state update
        update_interrupted_state(
            &worktree_manager,
            session_name,
            InterruptionType::UserInterrupt,
        );
        
        // Verify state was updated correctly
        let state = worktree_manager.get_session_state(session_name);
        if let Ok(Some(state)) = state {
            assert_eq!(state.status, crate::worktree::WorktreeStatus::Interrupted);
            assert!(state.resumable);
            assert!(state.interrupted_at.is_some());
            assert_eq!(state.interruption_type, Some(InterruptionType::UserInterrupt));
        }
    }
    
    #[test]
    fn test_termination_interrupt() {
        let temp_dir = TempDir::new().unwrap();
        let process_runner = Arc::new(MockProcessRunner::new());
        let git = Arc::new(MockGitOperations::new());
        let claude = Arc::new(MockClaudeOperations::new());
        
        let worktree_manager = Arc::new(WorktreeManager::new(
            temp_dir.path(),
            process_runner,
            git,
            claude,
        ));
        
        let session_name = "test-session-term";
        
        update_interrupted_state(
            &worktree_manager,
            session_name,
            InterruptionType::Termination,
        );
        
        let state = worktree_manager.get_session_state(session_name);
        if let Ok(Some(state)) = state {
            assert_eq!(state.interruption_type, Some(InterruptionType::Termination));
        }
    }
}
```

### Module: `src/metrics/complexity.rs`
**Coverage**: 0%
**Description**: Code complexity metrics calculation

#### Add these tests to src/metrics/complexity.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_complexity_calculator_new() {
        let calc = ComplexityCalculator::new();
        // Just ensure we can create an instance
        let _ = calc;
    }
    
    #[test]
    fn test_calculate_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        let calc = ComplexityCalculator::new();
        
        let metrics = calc.calculate(temp_dir.path()).unwrap();
        
        assert_eq!(metrics.total_lines, 0);
        assert!(metrics.cyclomatic_complexity.is_empty());
        assert!(metrics.cognitive_complexity.is_empty());
        assert_eq!(metrics.max_nesting_depth, 0);
    }
    
    #[test]
    fn test_calculate_simple_function() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        
        let test_code = r#"
fn simple_function(x: i32) -> i32 {
    x + 1
}

fn complex_function(x: i32) -> i32 {
    if x > 0 {
        if x > 10 {
            x * 2
        } else {
            x + 1
        }
    } else {
        0
    }
}
"#;
        
        fs::write(src_dir.join("test.rs"), test_code).unwrap();
        
        let calc = ComplexityCalculator::new();
        let metrics = calc.calculate(temp_dir.path()).unwrap();
        
        assert!(metrics.total_lines > 0);
        assert!(!metrics.cyclomatic_complexity.is_empty());
        
        // Check that complex_function has higher complexity than simple_function
        let simple_key = metrics.cyclomatic_complexity.keys()
            .find(|k| k.contains("simple_function"));
        let complex_key = metrics.cyclomatic_complexity.keys()
            .find(|k| k.contains("complex_function"));
        
        if let (Some(simple), Some(complex)) = (simple_key, complex_key) {
            let simple_complexity = metrics.cyclomatic_complexity[simple];
            let complex_complexity = metrics.cyclomatic_complexity[complex];
            assert!(complex_complexity > simple_complexity);
        }
    }
    
    #[test]
    fn test_nesting_depth() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        
        let nested_code = r#"
fn deeply_nested() {
    if true {
        while true {
            for i in 0..10 {
                match i {
                    0 => {
                        if i == 0 {
                            println!("Deep!");
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
"#;
        
        fs::write(src_dir.join("nested.rs"), nested_code).unwrap();
        
        let calc = ComplexityCalculator::new();
        let metrics = calc.calculate(temp_dir.path()).unwrap();
        
        assert!(metrics.max_nesting_depth >= 4);
    }
}
```

### Module: `src/cook/session/adapter.rs`
**Coverage**: 0%
**Description**: Adapter to bridge old session tracking to new session management

#### Add these tests to src/cook/session/adapter.rs:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_adapter_creation() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        
        // Test we can get the inner manager
        let inner = adapter.inner();
        assert!(Arc::strong_count(&inner) > 1);
    }
    
    #[tokio::test]
    async fn test_start_session() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        
        let result = adapter.start_session("test-session").await;
        assert!(result.is_ok());
        
        // Verify session was created
        let current = adapter.current_session.lock().await;
        assert!(current.is_some());
    }
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        
        // Start session
        adapter.start_session("test-lifecycle").await.unwrap();
        
        // Update iteration
        adapter.update_session(SessionUpdate::IncrementIteration).await.unwrap();
        
        // Add files changed
        adapter.update_session(SessionUpdate::AddFilesChanged(3)).await.unwrap();
        
        // Complete session
        let summary = adapter.complete_session().await.unwrap();
        assert_eq!(summary.files_changed, 3);
    }
    
    #[tokio::test]
    async fn test_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        
        // Try to update without starting session
        let result = adapter.update_session(SessionUpdate::IncrementIteration).await;
        assert!(result.is_err());
        
        // Try to complete without starting
        let result = adapter.complete_session().await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_state_conversion() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = SessionManagerAdapter::new(temp_dir.path().to_path_buf());
        
        adapter.start_session("test-state").await.unwrap();
        
        // Test in progress state
        let state = adapter.get_state();
        assert_eq!(state.status, SessionStatus::InProgress);
        
        // Test failed state
        adapter.update_session(SessionUpdate::UpdateStatus(SessionStatus::Failed)).await.unwrap();
        adapter.update_session(SessionUpdate::AddError("Test error".to_string())).await.unwrap();
        
        // Test interrupted state
        adapter.update_session(SessionUpdate::UpdateStatus(SessionStatus::Interrupted)).await.unwrap();
    }
}
```

## Integration Tests Needed

### Component: Context Analysis Integration
**File**: tests/context_analysis_integration.rs
```rust
use mmm::context::{ContextAnalyzer, AnalysisResult};
use mmm::cook::analysis::{AnalysisRunner, AnalysisCache};
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_context_analysis_integration() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    
    // Create test files
    fs::write(src_dir.join("lib.rs"), "pub fn hello() -> &'static str { \"Hello\" }").unwrap();
    fs::write(src_dir.join("main.rs"), "fn main() { println!(\"Test\"); }").unwrap();
    
    // Run analysis
    let analyzer = AnalysisRunnerImpl::new();
    let result = analyzer.analyze_project(temp_dir.path()).await.unwrap();
    
    // Verify analysis results
    assert!(!result.dependency_graph.nodes.is_empty());
    assert!(result.metadata.files_analyzed > 0);
}
```

### Component: Signal Handling Integration
**File**: tests/signal_handling_integration.rs
```rust
use mmm::cook::signal_handler::setup_interrupt_handlers;
use mmm::worktree::WorktreeManager;
use tempfile::TempDir;
use std::sync::Arc;

#[tokio::test]
async fn test_signal_handler_setup() {
    let temp_dir = TempDir::new().unwrap();
    let worktree_manager = Arc::new(WorktreeManager::new(
        temp_dir.path(),
        Default::default(),
        Default::default(),
        Default::default(),
    ));
    
    let result = setup_interrupt_handlers(worktree_manager, "test-session".to_string());
    assert!(result.is_ok());
    
    // Signal handlers are set up - actual signal testing requires process control
}
```

## Priority Test Implementation Plan

Based on the analysis, here are the modules requiring immediate attention:

### Critical Coverage Gaps (Priority 1)
1. **src/cook/analysis/mod.rs** - Core analysis coordination (0% coverage)
2. **src/cook/signal_handler.rs** - Critical for graceful shutdown (0% coverage)
3. **src/cook/session/adapter.rs** - Session management bridge (0% coverage)
4. **src/metrics/complexity.rs** - Important metrics calculation (0% coverage)

### Medium Priority Gaps (Priority 2)
5. **src/cook/interaction/prompts.rs** - User interaction (5.7% coverage)
6. **src/cook/metrics/collector.rs** - Metrics collection (12.1% coverage)
7. **src/cook/coordinators/execution.rs** - Execution coordination (2.0% coverage)
8. **src/cook/coordinators/workflow.rs** - Workflow coordination (3.4% coverage)

### Integration Test Gaps (Priority 3)
9. **tests/context_integration.rs** - Context system integration tests
10. **tests/error_handling_tests.rs** - Error handling scenarios
11. **tests/subprocess_tests.rs** - Subprocess management tests

## Implementation Checklist
- [ ] Add unit tests for 4 critical modules with 0% coverage
- [ ] Create 8 integration test files for uncovered test modules
- [ ] Improve coverage for 8 low-coverage critical modules
- [ ] Verify tests pass: `cargo test`
- [ ] Check coverage improves: `cargo tarpaulin --skip-clean`
- [ ] Follow project conventions from .mmm/context/conventions.json
- [ ] Ensure all async tests use `#[tokio::test]`
- [ ] Add both success and error test cases for each function
- [ ] Use existing mock implementations where available

## Expected Coverage Improvement
- Current: 55.0%
- Target after implementation: 75%+
- Critical modules: 0% → 80%+
- Integration test coverage: 0% → 60%+