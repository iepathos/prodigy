---
number: 61
title: Fix Ignored Tests with Proper Mocking
category: testing
priority: high
status: draft
dependencies: [57]
created: 2024-01-15
---

# Specification 61: Fix Ignored Tests with Proper Mocking

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: [57-subprocess-abstraction-layer]

## Context

Currently, MMM has 7 critical tests marked with `#[ignore]` because they hang waiting for external tools like `cargo tarpaulin`, `cargo clippy`, and `cargo build`. These tests are essential for validating core functionality but cannot run in CI/CD environments or during development because they:

1. Make real subprocess calls to external tools (cargo, git, tarpaulin)
2. Have no timeout mechanisms
3. Depend on external tool availability and configuration
4. Block indefinitely when tools are not installed or misconfigured
5. Cannot be reliably tested in isolated environments

The ignored tests include:
- `analyze::tests::test_analyze_empty_project`
- `analyze::tests::test_execute_all_analysis`
- `analyze::tests::test_execute_metrics_analysis`
- `analyze::tests::test_metrics_analysis_output_formats`
- `cook::metrics::collector::tests::test_metrics_collection`
- `metrics::collector::tests::test_collect_metrics_success`
- `metrics::collector::tests::test_collect_metrics_analyzer_failure`

This significantly impacts test coverage and development velocity.

## Objective

Un-ignore and fix all hanging tests by implementing proper mocking for external tool dependencies, enabling reliable test execution in all environments while maintaining test validity and coverage.

## Requirements

### Functional Requirements
- All currently ignored tests must pass without hanging
- Tests must run reliably in CI/CD environments without external tools
- Mock implementations must accurately simulate real tool behavior
- Tests must validate the actual logic being tested, not just mocked responses
- Support both success and failure scenarios for external tools
- Maintain compatibility with existing test infrastructure

### Non-Functional Requirements
- Tests must complete within 30 seconds total
- Mock setup must be simple and maintainable
- No impact on production code performance
- Tests must be deterministic and repeatable
- Support for parallel test execution

## Acceptance Criteria

- [ ] All 7 ignored tests are un-ignored and pass consistently
- [ ] Test suite completes within 30 seconds without hanging
- [ ] Tests pass in environments without cargo, git, or tarpaulin installed
- [ ] Mock implementations cover both success and error cases
- [ ] Tests validate business logic rather than just subprocess execution
- [ ] No regression in existing test functionality
- [ ] CI/CD pipeline runs all tests successfully
- [ ] Documentation explains how to add new mocked tests

## Technical Details

### Implementation Approach

1. **Leverage Subprocess Abstraction (Spec 57)**
   - Use the ProcessRunner trait from Spec 57 for all external tool calls
   - Implement comprehensive MockProcessRunner for test scenarios
   - Replace direct Command usage in test-affected modules

2. **Test-Specific Mocking Strategy**
   ```rust
   // Test utility for creating mocked subprocess environments
   pub struct TestSubprocessEnvironment {
       mock_runner: MockProcessRunner,
   }

   impl TestSubprocessEnvironment {
       pub fn new() -> Self { /* ... */ }
       
       pub fn expect_cargo_check(&mut self) -> &mut MockCommandExpectation { /* ... */ }
       pub fn expect_cargo_tarpaulin(&mut self) -> &mut MockCommandExpectation { /* ... */ }
       pub fn expect_git_status(&mut self) -> &mut MockCommandExpectation { /* ... */ }
       
       pub fn into_runner(self) -> Arc<dyn ProcessRunner> { /* ... */ }
   }
   ```

3. **Enhanced Mock Responses**
   ```rust
   // Realistic mock responses for different tools
   pub struct MockResponses;

   impl MockResponses {
       pub fn cargo_check_success() -> ProcessOutput { /* ... */ }
       pub fn cargo_check_with_warnings() -> ProcessOutput { /* ... */ }
       pub fn tarpaulin_coverage_report() -> ProcessOutput { /* ... */ }
       pub fn git_status_clean() -> ProcessOutput { /* ... */ }
   }
   ```

### Architecture Changes

1. **Test Module Refactoring**
   - Extract subprocess-dependent logic into testable components
   - Inject ProcessRunner dependencies through constructor or builder pattern
   - Separate business logic from subprocess execution

2. **Analysis Components Update**
   ```rust
   // Before: Direct subprocess calls in analyzer
   impl ProjectAnalyzer {
       pub async fn analyze(&self, path: &Path) -> Result<AnalysisResult> {
           let output = Command::new("cargo").arg("check").output().await?;
           // ... process output
       }
   }

   // After: Dependency injection
   impl ProjectAnalyzer {
       pub fn new(subprocess_runner: Arc<dyn ProcessRunner>) -> Self { /* ... */ }
       
       pub async fn analyze(&self, path: &Path) -> Result<AnalysisResult> {
           let output = self.subprocess_runner.run(
               ProcessCommandBuilder::new("cargo").arg("check").build()
           ).await?;
           // ... process output
       }
   }
   ```

### Data Structures

1. **Test Fixtures**
   ```rust
   #[derive(Debug, Clone)]
   pub struct TestProjectFixture {
       pub temp_dir: TempDir,
       pub cargo_toml: String,
       pub source_files: HashMap<PathBuf, String>,
   }

   impl TestProjectFixture {
       pub fn rust_project() -> Self { /* ... */ }
       pub fn empty_project() -> Self { /* ... */ }
       pub fn with_tests() -> Self { /* ... */ }
   }
   ```

2. **Mock Expectation Builder**
   ```rust
   pub struct MockCommandExpectation {
       program: String,
       args: Vec<String>,
       response: ProcessOutput,
       call_count: usize,
   }

   impl MockCommandExpectation {
       pub fn with_args(mut self, args: &[&str]) -> Self { /* ... */ }
       pub fn returns_success(mut self, output: &str) -> Self { /* ... */ }
       pub fn returns_error(mut self, code: i32, stderr: &str) -> Self { /* ... */ }
       pub fn called_times(mut self, count: usize) -> Self { /* ... */ }
   }
   ```

## Dependencies

- **Prerequisites**: 
  - Spec 57: Subprocess Abstraction Layer (for ProcessRunner trait)
- **Affected Components**: 
  - `src/analyze/` - Analysis command tests
  - `src/cook/metrics/` - Metrics collector tests  
  - `src/metrics/` - Core metrics tests
  - `src/context/` - Context analysis components
- **External Dependencies**: 
  - tempfile (for test fixtures)
  - tokio-test (for async test utilities)

## Testing Strategy

- **Unit Tests**: 
  - Test mock runner behavior thoroughly
  - Test realistic subprocess response scenarios
  - Test error handling and edge cases
  - Test timeout scenarios
- **Integration Tests**: 
  - Test complete analysis workflows with mocked tools
  - Test metrics collection end-to-end
  - Test error propagation through the system
- **Performance Tests**: 
  - Verify test suite completes within 30 seconds
  - Measure mock overhead vs real subprocess calls
- **User Acceptance**: 
  - All tests pass in clean CI environment
  - Tests provide meaningful failure messages
  - Easy to add new mocked tests

## Documentation Requirements

- **Code Documentation**: 
  - Document mock setup patterns
  - Provide examples of test fixture usage
  - Document realistic mock response patterns
- **Testing Guide**: 
  - How to create new tests with subprocess mocking
  - Common mock scenarios and patterns
  - Debugging test failures with mocks
- **Migration Guide**: 
  - How existing tests were converted
  - Patterns for future test development

## Implementation Notes

1. **Realistic Mock Behavior**
   ```rust
   // Mock cargo check with realistic warning output
   mock_env.expect_cargo_check()
       .with_args(&["check", "--message-format=json"])
       .returns_success(r#"
   {"reason":"compiler-message","message":{"code":null,"level":"warning",...}}
   {"reason":"build-finished","success":true}
   "#);
   ```

2. **Test Organization**
   - Group related tests in modules
   - Use test utilities for common setup
   - Provide clear test naming conventions
   - Document test scenario coverage

3. **Error Simulation**
   ```rust
   // Test error handling paths
   mock_env.expect_cargo_tarpaulin()
       .returns_error(1, "cargo-tarpaulin not installed");
   
   // Verify graceful degradation
   let result = analyzer.analyze(&project_path).await;
   assert!(result.is_ok());
   assert_eq!(result.unwrap().test_coverage, None);
   ```

## Migration and Compatibility

1. **Gradual Migration**
   - Phase 1: Implement base mocking infrastructure
   - Phase 2: Convert analyze tests
   - Phase 3: Convert metrics collector tests
   - Phase 4: Convert remaining tests

2. **Backwards Compatibility**
   - Maintain existing test structure
   - No changes to public APIs
   - Optional real subprocess execution for integration testing

3. **Test Environment Configuration**
   ```rust
   // Allow switching between real and mocked execution
   #[cfg(feature = "integration-tests")]
   fn create_subprocess_runner() -> Arc<dyn ProcessRunner> {
       Arc::new(TokioProcessRunner::new())
   }

   #[cfg(not(feature = "integration-tests"))]
   fn create_subprocess_runner() -> Arc<dyn ProcessRunner> {
       Arc::new(create_test_mock_runner())
   }
   ```

## Success Metrics

- **Test Coverage**: Increase from current (with ignored tests) to 100% test execution
- **Test Performance**: Complete test suite in <30 seconds
- **CI Reliability**: 100% test pass rate in CI without external dependencies
- **Developer Experience**: Zero test hangs during development
- **Test Maintainability**: Easy addition of new subprocess-dependent tests