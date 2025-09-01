---
number: 46
title: Self-Healing Analysis Errors
category: foundation
priority: high
status: draft
dependencies: [47]
created: 2025-08-03
---

# Specification 46: Self-Healing Analysis Errors

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [47 - Workflow Syntax Improvements]

## Context

MMM runs various analysis tools (coverage, linting, benchmarks) during the metrics collection phase when workflow steps have `analysis:` configured. When these tools fail - often due to compilation errors, missing dependencies, or environment issues - the entire workflow breaks. Currently, users must manually intervene to fix these issues, breaking the automation loop.

The recent `serde_toml` error after a merge is a perfect example: coverage compilation failed because of an unresolved module reference, requiring manual fixes before the workflow could continue. These failures are particularly frustrating because they're often simple to fix but require human intervention.

Additionally, tests can fail during workflow execution, and we need a mechanism to automatically debug and fix failing tests to maintain continuous improvement loops.

## Objective

Enable MMM to automatically detect, diagnose, and recover from analysis tool failures and test failures during workflow execution, making automation loops truly self-sufficient and resilient to common environmental, dependency, and test issues.

## Requirements

### Functional Requirements
- Detect when analysis tools (coverage, lint, benchmarks) fail during metrics collection
- Detect when tests fail during workflow execution
- Capture complete error context including stdout, stderr, and exit codes
- Support conditional workflow steps based on command exit codes
- Automatically trigger recovery commands to fix detected issues
- Retry analysis/tests after recovery attempts
- Continue workflow execution if recovery succeeds
- Provide clear diagnostics if recovery fails

### Non-Functional Requirements
- Recovery attempts should complete within 5 minutes
- Should not retry more than 2 times to avoid infinite loops
- Must preserve all error context for debugging
- Should work with existing workflow files without modification
- Must be configurable (enable/disable, max retries, timeout)
- Manual `mmm analyze` commands should NOT trigger automatic recovery by default
- Workflow analysis should trigger recovery by default (configurable)

## Acceptance Criteria

- [ ] MMM detects coverage tool failures during analysis phase
- [ ] MMM detects test failures during workflow execution
- [ ] Error context is captured and passed to recovery command
- [ ] Recovery command `/mmm-fix-analysis-errors` is automatically invoked in workflows
- [ ] Recovery command `/mmm-fix-test-failures` is automatically invoked for test failures
- [ ] Manual `mmm analyze` does NOT trigger recovery unless `--auto-recover` flag is used
- [ ] Analysis is retried after successful recovery
- [ ] Tests are retried after successful recovery
- [ ] Workflow continues if retry succeeds
- [ ] Clear error reporting if recovery fails after max attempts
- [ ] Feature can be disabled via workflow analysis configuration
- [ ] Works with all existing analysis tools (coverage, lint, bench)
- [ ] Workflow YAML can configure recovery behavior per step
- [ ] Workflow YAML supports conditional steps based on exit codes

## Technical Details

### Implementation Approach

1. **Error Detection in Analysis Runner**
   ```rust
   // In src/metrics/analyzer.rs or similar
   pub async fn run_analysis(&self, config: &AnalysisConfig, context: &AnalysisContext) -> Result<Metrics> {
       match self.run_analysis_tools().await {
           Ok(metrics) => Ok(metrics),
           Err(e) if self.should_attempt_recovery(&e, context) => {
               self.attempt_recovery(e, context).await
           }
           Err(e) => Err(e),
       }
   }
   
   fn should_attempt_recovery(&self, error: &AnalysisError, context: &AnalysisContext) -> bool {
       match context.source {
           AnalysisSource::Manual { auto_recover } => auto_recover,
           AnalysisSource::Workflow { step_config } => {
               step_config.analysis.auto_recover.unwrap_or(true)
           }
       }
   }
   ```

2. **Recovery Logic**
   ```rust
   async fn attempt_recovery(&self, error: AnalysisError) -> Result<Metrics> {
       let mut attempts = 0;
       let max_attempts = self.config.max_recovery_attempts.unwrap_or(2);
       
       while attempts < max_attempts {
           // Create error specification
           let error_spec = self.create_error_spec(&error).await?;
           
           // Run recovery command
           if self.run_recovery_command(error_spec).await.is_ok() {
               // Retry analysis
               if let Ok(metrics) = self.run_analysis_tools().await {
                   return Ok(metrics);
               }
           }
           attempts += 1;
       }
       
       Err(error)
   }
   ```

3. **Error Specification Format**
   ```markdown
   # Analysis Error Recovery Specification
   
   ## Error Context
   - Tool: cargo-tarpaulin
   - Phase: coverage
   - Exit Code: 101
   - Timestamp: 2025-08-03T19:46:16Z
   
   ## Error Output
   ```
   error[E0433]: failed to resolve: use of unresolved module or unlinked crate `serde_toml`
   --> tests/config_integration_tests.rs:67:26
   ```
   
   ## Working Directory
   /Users/glen/prodigy
   
   ## Recent Changes
   - Merged branch 'mmm-session-xyz'
   - Modified files: tests/config_integration_tests.rs
   ```

### Architecture Changes

1. **New Recovery Module**
   - `src/recovery/mod.rs` - Main recovery orchestration
   - `src/recovery/analysis.rs` - Analysis-specific recovery logic
   - `src/recovery/patterns.rs` - Common error patterns and fixes

2. **Modified Components**
   - `src/metrics/analyzer.rs` - Add recovery hooks
   - `src/commands/cook.rs` - Handle recovery command execution
   - `src/config/mod.rs` - Add recovery configuration options

### Data Structures

```rust
pub struct RecoveryConfig {
    pub enabled: bool,
    pub max_attempts: u8,
    pub timeout_seconds: u64,
    pub recovery_command: String,
}

pub enum AnalysisSource {
    Manual { auto_recover: bool },
    Workflow { step_config: WorkflowStep },
}

pub struct AnalysisContext {
    pub source: AnalysisSource,
    pub project_root: PathBuf,
}

pub struct AnalysisError {
    pub tool: String,
    pub phase: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub working_dir: PathBuf,
}

pub struct ErrorSpecification {
    pub error: AnalysisError,
    pub context: ProjectContext,
    pub recent_changes: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct WorkflowAnalysisConfig {
    pub max_cache_age: Option<u64>,
    pub auto_recover: Option<bool>,  // Default: true
    pub recovery_command: Option<String>,
    pub max_recovery_attempts: Option<u8>,
}

// New structures for conditional execution
pub struct ConditionalStep {
    pub condition: StepCondition,
    pub step: WorkflowStep,
}

pub enum StepCondition {
    OnSuccess,      // Run if previous step succeeded (exit code 0)
    OnFailure,      // Run if previous step failed (non-zero exit code)
    OnExitCode(i32), // Run on specific exit code
    Always,         // Always run (default)
}

pub struct WorkflowStep {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub commit_required: bool,
    pub analysis: Option<WorkflowAnalysisConfig>,
    pub on_failure: Option<Box<WorkflowStep>>, // Step to run on failure
    pub on_success: Option<Box<WorkflowStep>>, // Step to run on success
    pub capture_output: bool,  // Whether to capture output for next step
}
```

### APIs and Interfaces

1. **Recovery Command Interface**
   - Input: Error specification file path
   - Output: Success/failure status
   - Side effects: Fixes to project files, dependencies, or environment

2. **CLI Interface**
   ```bash
   # Manual analysis - no auto-recovery by default
   mmm analyze
   
   # Manual analysis with auto-recovery enabled
   mmm analyze --auto-recover
   ```

3. **Workflow Configuration API**
   ```yaml
   commands:
     # Standard analysis recovery
     - name: mmm-coverage
       analysis:
         max_cache_age: 300
         auto_recover: true  # Default: true
         recovery_command: "mmm-fix-analysis-errors"  # Optional override
         max_recovery_attempts: 2  # Optional override
     
     # Test execution with conditional recovery
     - name: run-tests
       command: "cargo test"
       capture_output: true
       on_failure:
         name: fix-test-failures
         command: "mmm-fix-test-failures"
         args: ["$CAPTURED_OUTPUT"]  # Pass test output to recovery command
         commit_required: true
       on_success:
         name: celebrate
         command: "echo"
         args: ["All tests passed!"]
         commit_required: false
     
     # Complex conditional workflow
     - name: complex-test
       command: "cargo test --workspace"
       conditions:
         - exit_code: 0
           next: continue-workflow
         - exit_code: 1
           next: fix-and-retry
         - exit_code: 101
           next: fix-compilation-errors
         - default: fail-workflow
   ```

4. **Global Configuration API**
   ```toml
   [recovery]
   # Global defaults for recovery behavior
   max_attempts = 2
   timeout_seconds = 300
   command = "mmm-fix-analysis-errors"
   ```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - Metrics analysis system
  - Command execution framework
  - Configuration system
- **External Dependencies**: None new

## Testing Strategy

- **Unit Tests**: 
  - Test error detection logic
  - Test recovery attempt counting
  - Test error specification generation
  - Test configuration parsing

- **Integration Tests**:
  - Simulate coverage failure and recovery
  - Test with different error types
  - Verify workflow continuation after recovery
  - Test max retry limits

- **Performance Tests**:
  - Ensure recovery doesn't significantly slow workflows
  - Test timeout enforcement

- **User Acceptance**:
  - Test with real-world error scenarios
  - Verify clear error messaging
  - Test configuration options

## Documentation Requirements

- **Code Documentation**: 
  - Document recovery module APIs
  - Add inline documentation for configuration options
  - Document error specification format

- **User Documentation**:
  - Add recovery section to MMM docs
  - Document configuration options
  - Provide troubleshooting guide

- **Architecture Updates**:
  - Update ARCHITECTURE.md with recovery flow
  - Document new module responsibilities

## Implementation Notes

1. **Error Pattern Library**: Build a library of common error patterns for quick diagnosis
2. **Telemetry**: Log recovery attempts for analysis and improvement
3. **Graceful Degradation**: If recovery is disabled or fails, provide helpful manual fix instructions
4. **Command Safety**: Ensure recovery commands can't cause destructive changes
5. **Context Preservation**: Keep all error context for debugging even after successful recovery

## Migration and Compatibility

- **No Breaking Changes**: Feature is opt-in by default
- **Backward Compatible**: Works with all existing workflows
- **Configuration Migration**: Add default recovery config to existing projects
- **Command Availability**: Requires `/mmm-fix-analysis-errors` command to be available

## Example Recovery Scenarios

1. **Missing Dependency**
   - Error: "unresolved module or unlinked crate"
   - Recovery: Add missing dependency to Cargo.toml

2. **Compilation Error**
   - Error: "failed to compile tests"
   - Recovery: Fix syntax errors or imports

3. **Tool Not Installed**
   - Error: "cargo-tarpaulin: command not found"
   - Recovery: Install missing tool

4. **Environment Issue**
   - Error: "RUSTFLAGS conflict"
   - Recovery: Adjust environment variables

5. **Test Failures**
   - Error: "test result: FAILED. 3 passed; 2 failed"
   - Recovery: Analyze test output and fix failing assertions
   - Context: Pass full test output including failure details

6. **Integration Test Failures**
   - Error: "thread 'test_api_endpoint' panicked"
   - Recovery: Debug panic, fix test setup or implementation
   - Context: Stack trace and test environment state

## Usage Examples

### Manual Analysis
```bash
# Default behavior - no auto-recovery
$ mmm analyze
✗ Coverage analysis failed: unresolved module 'serde_toml'
  Run with --auto-recover to attempt automatic fixes

# With auto-recovery enabled
$ mmm analyze --auto-recover
✗ Coverage analysis failed: unresolved module 'serde_toml'
→ Attempting automatic recovery...
✓ Fixed: Updated imports to use 'toml' crate
✓ Analysis completed successfully
```

### Workflow Configuration
```yaml
# Enable recovery for specific step (default: true)
commands:
  - name: mmm-coverage
    analysis:
      auto_recover: true
      
# Disable recovery for specific step
commands:
  - name: mmm-lint
    analysis:
      auto_recover: false
      
# Custom recovery settings
commands:
  - name: mmm-benchmark
    analysis:
      auto_recover: true
      recovery_command: "mmm-fix-bench-errors"
      max_recovery_attempts: 3

# Test workflow with automatic recovery
commands:
  - name: run-unit-tests
    command: "cargo test --lib"
    capture_output: true
    on_failure:
      name: fix-unit-test-failures
      command: "mmm-fix-test-failures"
      args: ["$CAPTURED_OUTPUT", "--test-type", "unit"]
      
  - name: run-integration-tests
    command: "cargo test --test '*'"
    capture_output: true
    on_failure:
      name: fix-integration-test-failures
      command: "mmm-fix-test-failures"
      args: ["$CAPTURED_OUTPUT", "--test-type", "integration"]
      on_failure:
        name: notify-test-failure
        command: "echo"
        args: ["Tests still failing after recovery attempt"]
        commit_required: false
```

### Complete Implementation Workflow Example
```yaml
# implement-with-tests.yml - Implementation with test-driven recovery
commands:
  # Implement the specification
  - name: mmm-implement-spec
    args: ["$ARG"]
    analysis:
      max_cache_age: 300
  
  # Run tests to verify implementation
  - name: run-tests
    command: "cargo test"
    capture_output: true
    commit_required: false
    on_failure:
      name: debug-and-fix-tests
      command: "mmm-debug-test-failures"
      args: ["$CAPTURED_OUTPUT"]
      commit_required: true
      on_success:
        name: verify-fix
        command: "cargo test"
        commit_required: false
  
  # Run linting after tests pass
  - name: mmm-lint
    commit_required: false
    
  # Final check
  - name: final-test-run
    command: "cargo test"
    commit_required: false
    on_failure:
      name: report-persistent-failures
      command: "mmm-report-test-status"
      args: ["failed", "$CAPTURED_OUTPUT"]
```