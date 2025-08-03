---
number: 46
title: Self-Healing Analysis Errors
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-08-03
---

# Specification 46: Self-Healing Analysis Errors

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MMM runs various analysis tools (coverage, linting, benchmarks) during the metrics collection phase when workflow steps have `analysis:` configured. When these tools fail - often due to compilation errors, missing dependencies, or environment issues - the entire workflow breaks. Currently, users must manually intervene to fix these issues, breaking the automation loop.

The recent `serde_toml` error after a merge is a perfect example: coverage compilation failed because of an unresolved module reference, requiring manual fixes before the workflow could continue. These failures are particularly frustrating because they're often simple to fix but require human intervention.

## Objective

Enable MMM to automatically detect, diagnose, and recover from analysis tool failures during workflow execution, making automation loops truly self-sufficient and resilient to common environmental and dependency issues.

## Requirements

### Functional Requirements
- Detect when analysis tools (coverage, lint, benchmarks) fail during metrics collection
- Capture complete error context including stdout, stderr, and exit codes
- Automatically trigger recovery commands to fix detected issues
- Retry analysis after recovery attempts
- Continue workflow execution if recovery succeeds
- Provide clear diagnostics if recovery fails

### Non-Functional Requirements
- Recovery attempts should complete within 5 minutes
- Should not retry more than 2 times to avoid infinite loops
- Must preserve all error context for debugging
- Should work with existing workflow files without modification
- Must be configurable (enable/disable, max retries, timeout)

## Acceptance Criteria

- [ ] MMM detects coverage tool failures during analysis phase
- [ ] Error context is captured and passed to recovery command
- [ ] Recovery command `/mmm-fix-analysis-errors` is automatically invoked
- [ ] Analysis is retried after successful recovery
- [ ] Workflow continues if retry succeeds
- [ ] Clear error reporting if recovery fails after max attempts
- [ ] Feature can be disabled via configuration
- [ ] Works with all existing analysis tools (coverage, lint, bench)
- [ ] No changes required to existing workflow YAML files

## Technical Details

### Implementation Approach

1. **Error Detection in Analysis Runner**
   ```rust
   // In src/metrics/analyzer.rs or similar
   pub async fn run_analysis(&self, config: &AnalysisConfig) -> Result<Metrics> {
       match self.run_analysis_tools().await {
           Ok(metrics) => Ok(metrics),
           Err(e) if self.should_attempt_recovery(&e) => {
               self.attempt_recovery(e).await
           }
           Err(e) => Err(e),
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
   /Users/glen/memento-mori/mmm
   
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
```

### APIs and Interfaces

1. **Recovery Command Interface**
   - Input: Error specification file path
   - Output: Success/failure status
   - Side effects: Fixes to project files, dependencies, or environment

2. **Configuration API**
   ```toml
   [recovery]
   enabled = true
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