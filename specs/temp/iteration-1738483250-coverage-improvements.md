# Coverage Improvements - Iteration 1738483250

## Overview
Test coverage improvements based on MMM context analysis.
Current coverage: 54.86% → Target: 65%

## Critical Functions Needing Tests

### Function: `test_format_subprocess_error_unauthorized` in src/cook/retry.rs:351
**Criticality**: High
**Current Status**: No test coverage (function exists but isn't being executed in tests)

#### Add comprehensive test coverage for retry module:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_subprocess_error_general() {
        let error = format_subprocess_error("command", Some(1), "general error", "");
        assert!(error.contains("Process 'command' failed"));
        assert!(error.contains("Exit code: 1"));
        assert!(error.contains("general error"));
    }
    
    #[test]
    fn test_format_subprocess_error_network() {
        let error = format_subprocess_error("command", Some(1), "network error", "");
        assert!(error.contains("This might be a temporary network issue"));
    }
    
    #[test]
    fn test_format_subprocess_error_permission_denied() {
        let error = format_subprocess_error("command", Some(1), "permission denied", "");
        assert!(error.contains("Check file permissions"));
    }
    
    #[test]
    fn test_format_subprocess_error_authentication() {
        let error = format_subprocess_error("claude", Some(1), "API key invalid", "");
        assert!(error.contains("Check that you have authenticated"));
    }
    
    #[test]
    fn test_should_retry_on_network_errors() {
        assert!(should_retry("Connection refused", ""));
        assert!(should_retry("Network is unreachable", ""));
        assert!(should_retry("", "timed out"));
    }
    
    #[test]
    fn test_should_not_retry_on_permanent_errors() {
        assert!(!should_retry("Authentication failed", ""));
        assert!(!should_retry("Permission denied", ""));
        assert!(!should_retry("Invalid API key", ""));
    }
    
    #[test]
    fn test_parse_claude_error_response() {
        let response = r#"{"error": {"message": "Rate limit exceeded"}}"#;
        let parsed = parse_claude_error_response(response);
        assert_eq!(parsed, Some("Rate limit exceeded".to_string()));
    }
    
    #[test]
    fn test_determine_backoff_ms() {
        let backoff = determine_backoff_ms(1, "Rate limit");
        assert!(backoff >= 1000);
        
        let backoff = determine_backoff_ms(3, "Network error");
        assert!(backoff >= 4000);
    }
}
```

## High-Priority Modules Needing Tests

### Module: `src/cook/orchestrator.rs` (36 untested functions)
**Coverage**: 53.4%
**Priority**: Medium

#### Add integration tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestContext, MockGitBuilder, MockClaudeBuilder};
    
    #[tokio::test]
    async fn test_orchestrator_initialization() {
        let context = TestContext::new();
        let git = MockGitBuilder::new().is_repo(true).build();
        let claude = MockClaudeBuilder::new().is_available(true).build();
        
        let orchestrator = Orchestrator::new(
            context.temp_path(),
            git,
            claude,
            Default::default()
        );
        
        assert!(orchestrator.is_initialized());
    }
    
    #[tokio::test]
    async fn test_orchestrator_run_iteration() {
        let context = TestContext::new();
        let git = MockGitBuilder::new()
            .is_repo(true)
            .with_clean_status()
            .build();
        let claude = MockClaudeBuilder::new()
            .is_available(true)
            .with_response("Test completed")
            .build();
        
        let mut orchestrator = Orchestrator::new(
            context.temp_path(),
            git,
            claude,
            Default::default()
        );
        
        let result = orchestrator.run_iteration().await;
        assert!(result.is_ok());
    }
}
```

### Module: `src/cook/interaction/mod.rs` (35 untested functions)
**Coverage**: 52.4%
**Priority**: Medium

#### Add unit tests for interaction module:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prompt_creation() {
        let prompt = create_prompt("test command", None);
        assert!(prompt.contains("test command"));
    }
    
    #[test]
    fn test_display_formatting() {
        let display = format_display_output("Test output");
        assert!(display.contains("Test output"));
    }
    
    #[test]
    fn test_error_display() {
        let error = format_error_display("Test error");
        assert!(error.contains("Error"));
        assert!(error.contains("Test error"));
    }
}
```

### Module: `src/git/scenario.rs` (32 untested functions)
**Coverage**: 60.2%
**Priority**: Medium

#### Add scenario-based tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_git_scenario_clean() {
        let temp_dir = TempDir::new().unwrap();
        let scenario = GitScenario::analyze(temp_dir.path()).unwrap();
        assert_eq!(scenario.state, GitState::Clean);
    }
    
    #[test]
    fn test_git_scenario_dirty() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
        
        let scenario = GitScenario::analyze(temp_dir.path()).unwrap();
        assert_eq!(scenario.state, GitState::Dirty);
        assert_eq!(scenario.untracked_files.len(), 1);
    }
    
    #[test]
    fn test_git_scenario_merge_conflict() {
        let temp_dir = TempDir::new().unwrap();
        // Setup merge conflict scenario
        
        let scenario = GitScenario::analyze(temp_dir.path()).unwrap();
        assert_eq!(scenario.state, GitState::MergeConflict);
    }
}
```

### Module: `src/context/debt.rs` (30 untested functions)
**Coverage**: 73.96%
**Priority**: Medium

#### Add debt analysis tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_todo_comments() {
        let content = "// TODO: Fix this later";
        let debt_items = analyze_debt_in_content(content, "test.rs");
        assert_eq!(debt_items.len(), 1);
        assert_eq!(debt_items[0].debt_type, DebtType::Todo);
    }
    
    #[test]
    fn test_detect_fixme_comments() {
        let content = "// FIXME: This is broken";
        let debt_items = analyze_debt_in_content(content, "test.rs");
        assert_eq!(debt_items.len(), 1);
        assert_eq!(debt_items[0].debt_type, DebtType::Fixme);
    }
    
    #[test]
    fn test_complexity_detection() {
        let complex_function = r#"
        fn complex_function(x: i32) -> i32 {
            match x {
                1 => if x > 0 { 1 } else { 2 },
                2 => if x < 10 { 3 } else { 4 },
                3 => if x == 3 { 5 } else { 6 },
                _ => 0
            }
        }
        "#;
        
        let complexity = calculate_complexity(complex_function);
        assert!(complexity > 5);
    }
}
```

### Module: `src/testing/mod.rs` (28 untested functions)
**Coverage**: 0%
**Priority**: Low

#### Add self-tests for testing infrastructure:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_test_context_creation() {
        let context = TestContext::new();
        assert!(context.temp_path().exists());
    }
    
    #[test]
    fn test_mock_git_builder() {
        let git = MockGitBuilder::new()
            .is_repo(true)
            .with_clean_status()
            .build();
        
        assert!(git.is_repo().unwrap());
        assert!(git.status().unwrap().is_clean);
    }
    
    #[test]
    fn test_mock_claude_builder() {
        let claude = MockClaudeBuilder::new()
            .is_available(true)
            .with_response("Test")
            .build();
        
        assert!(claude.is_available().unwrap());
    }
}
```

## Integration Tests Needed

### Component: Cook Workflow Integration
**File**: tests/cook_integration.rs
```rust
use mmm::cook::{Orchestrator, CookOptions};
use mmm::testing::TestContext;

#[tokio::test]
async fn test_cook_full_workflow() {
    let context = TestContext::new();
    context.create_test_file("src/main.rs", "fn main() {}");
    
    let options = CookOptions {
        max_iterations: 1,
        auto_commit: false,
        ..Default::default()
    };
    
    let result = Orchestrator::run_cook_session(
        context.temp_path(),
        options
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cook_with_failing_tests() {
    let context = TestContext::new();
    context.create_test_file("src/lib.rs", r#"
        fn add(a: i32, b: i32) -> i32 { a + b }
        
        #[test]
        fn test_add() {
            assert_eq!(add(2, 2), 5); // Intentionally failing
        }
    "#);
    
    let options = CookOptions {
        max_iterations: 2,
        auto_commit: false,
        ..Default::default()
    };
    
    let result = Orchestrator::run_cook_session(
        context.temp_path(),
        options
    ).await;
    
    // Should attempt to fix the failing test
    assert!(result.is_ok());
}
```

### Component: Metrics Collection
**File**: tests/metrics_integration.rs
```rust
use mmm::metrics::{MetricsRegistry, MetricsCollector};
use mmm::testing::TestContext;

#[tokio::test]
async fn test_metrics_collection() {
    let context = TestContext::new();
    let registry = MetricsRegistry::new(context.temp_path());
    let collector = MetricsCollector::new(registry);
    
    let metrics = collector.collect_all_metrics().await.unwrap();
    
    assert!(metrics.test_coverage >= 0.0);
    assert!(metrics.test_coverage <= 100.0);
    assert!(metrics.lint_warnings >= 0);
}
```

## Implementation Checklist
- [ ] Add unit tests for 1 critical function in src/cook/retry.rs
- [ ] Create 120+ unit tests across high-priority modules
- [ ] Create 5 integration test files
- [ ] Verify tests pass: `cargo test`
- [ ] Check coverage improves: `cargo tarpaulin`
- [ ] Follow project conventions from .mmm/context/conventions.json

## Validation Commands
```bash
# Run all tests
cargo test --all-features

# Run tests with coverage
cargo tarpaulin --out Html --output-dir target/coverage

# Run specific module tests
cargo test --test cook_integration
cargo test retry::tests

# Verify coverage improvement
cargo tarpaulin --print-summary
```

## Expected Coverage Improvements
- Overall: 54.86% → 65%+ (10% improvement)
- src/cook/retry.rs: 79.4% → 95%+
- src/cook/orchestrator.rs: 53.4% → 75%+
- src/cook/interaction/mod.rs: 52.4% → 70%+
- src/git/scenario.rs: 60.2% → 80%+
- src/testing/mod.rs: 0% → 60%+