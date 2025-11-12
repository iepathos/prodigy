---
number: 161
title: Resume Error Messaging Enhancement
category: optimization
priority: medium
status: draft
dependencies: [134, 159]
created: 2025-01-11
---

# Specification 161: Resume Error Messaging Enhancement

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 134 (MapReduce Checkpoint and Resume), Spec 159 (MapReduce Resume CLI)

## Context

The Phase 1 investigation identified that error messages in the checkpoint and resume system lack actionability and context. When resume operations fail, users receive generic error messages that don't guide them toward resolution. This leads to frustration, support requests, and abandoned workflows.

Current error messaging problems:
- ❌ "Checkpoint not found" - no suggestion for next steps
- ❌ "Workflow file not found" - no explanation of why or how to fix
- ❌ "Version not supported" - no migration path provided
- ❌ Generic anyhow errors - lack specific context for debugging

Good error messages should be:
1. **Specific**: Clearly state what went wrong
2. **Contextual**: Include relevant details (paths, IDs, values)
3. **Actionable**: Suggest concrete next steps
4. **Helpful**: Guide users toward resolution

## Objective

Enhance error messages throughout the checkpoint and resume system to provide clear, actionable guidance that enables users to resolve issues independently without requiring support intervention.

## Requirements

### Functional Requirements

1. **Checkpoint Not Found Errors**
   - Explain possible causes (cleared storage, wrong ID, early failure)
   - Suggest commands to list available checkpoints
   - Provide example of correct session ID format
   - Include path to checkpoint directory for verification

2. **Workflow File Not Found Errors**
   - Show the exact path that was checked
   - Explain that file may have been moved or deleted
   - Suggest using `--workflow-path` flag to specify location
   - Provide command to verify checkpoint contains correct path

3. **Version Incompatibility Errors**
   - Show checkpoint version vs. current code version
   - Explain version compatibility rules
   - Suggest upgrade/downgrade path if available
   - Provide command to check checkpoint version

4. **Workflow Hash Mismatch Errors**
   - Explain that workflow file has changed since checkpoint
   - Show what changed (step count, hash difference)
   - Suggest reviewing workflow modifications
   - Provide option to force resume (if safe)

5. **Environment Validation Errors**
   - Show which environment variable changed
   - Display old value vs. new value (mask secrets)
   - Explain potential impact of change
   - Suggest resetting environment or acknowledging change

6. **Resume Lock Errors**
   - Show which process holds the lock (PID, hostname)
   - Show when lock was acquired and how long held
   - Suggest waiting or using `--force` flag
   - Explain stale lock detection and cleanup

7. **MapReduce-Specific Errors**
   - DLQ errors: Show item count, retry limits, failure patterns
   - Phase errors: Explain current phase and why resume failed
   - Work item errors: Show which items failed and why
   - Reduce phase errors: Explain missing map results

### Non-Functional Requirements

1. **Clarity**
   - Use plain language, avoid jargon
   - Structure errors consistently
   - Highlight key information visually
   - Provide examples where helpful

2. **Conciseness**
   - Keep error messages under 200 characters for summary
   - Provide details in structured format below
   - Use bullet points for multiple suggestions
   - Avoid redundant information

3. **Consistency**
   - Use consistent formatting across all error types
   - Follow established error message patterns
   - Maintain consistent terminology
   - Use consistent command examples

4. **Safety**
   - Never expose secrets in error messages
   - Mask sensitive data (API keys, tokens, passwords)
   - Sanitize file paths if they contain user-specific info
   - Warn about potentially destructive operations

## Acceptance Criteria

- [ ] All checkpoint/resume error messages include suggested next steps
- [ ] Errors show specific context (paths, IDs, values)
- [ ] Secrets and sensitive data are masked in all error messages
- [ ] Error message format is consistent across all error types
- [ ] Users can resolve 80% of errors without consulting documentation
- [ ] Error messages pass accessibility review (clear, concise, actionable)
- [ ] All error message changes have corresponding tests
- [ ] Documentation is updated with error troubleshooting guide
- [ ] Code review confirms error messages are helpful and professional

## Technical Details

### Implementation Approach

1. **Create Error Types with Context**
   ```rust
   // src/cook/workflow/checkpoint/errors.rs
   #[derive(Debug, thiserror::Error)]
   pub enum CheckpointError {
       #[error("Checkpoint not found for session {session_id}")]
       NotFound {
           session_id: String,
           checkpoint_dir: PathBuf,
           suggestion: String,
       },

       #[error("Workflow file not found at {workflow_path}")]
       WorkflowFileNotFound {
           workflow_path: PathBuf,
           session_id: String,
           suggestion: String,
       },

       #[error("Checkpoint version {checkpoint_version} is not supported (current: {current_version})")]
       VersionMismatch {
           checkpoint_version: u32,
           current_version: u32,
           suggestion: String,
       },

       #[error("Workflow has changed since checkpoint (hash mismatch)")]
       WorkflowHashMismatch {
           expected_hash: String,
           actual_hash: String,
           checkpoint_steps: usize,
           current_steps: usize,
           suggestion: String,
       },
   }
   ```

2. **Error Display Implementation**
   ```rust
   impl std::fmt::Display for CheckpointError {
       fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
           match self {
               CheckpointError::NotFound { session_id, checkpoint_dir, suggestion } => {
                   writeln!(f, "Checkpoint not found for session: {}", session_id)?;
                   writeln!(f)?;
                   writeln!(f, "Possible causes:")?;
                   writeln!(f, "  • Checkpoint storage was cleared")?;
                   writeln!(f, "  • Session ID is incorrect")?;
                   writeln!(f, "  • Workflow failed before checkpoint could be saved")?;
                   writeln!(f)?;
                   writeln!(f, "Checkpoint directory: {}", checkpoint_dir.display())?;
                   writeln!(f)?;
                   writeln!(f, "Suggestions:")?;
                   writeln!(f, "  • List available sessions: prodigy sessions list --status Failed")?;
                   writeln!(f, "  • Check checkpoint directory: ls -la {}", checkpoint_dir.display())?;
                   write!(f, "  • {}", suggestion)
               }
               // ... other error types
           }
       }
   }
   ```

3. **Contextual Error Creation**
   ```rust
   // In checkpoint loading code
   pub async fn load_checkpoint(session_id: &str) -> Result<WorkflowCheckpoint, CheckpointError> {
       let checkpoint_path = get_checkpoint_path(session_id);

       if !checkpoint_path.exists() {
           return Err(CheckpointError::NotFound {
               session_id: session_id.to_string(),
               checkpoint_dir: checkpoint_path.parent().unwrap().to_path_buf(),
               suggestion: format!(
                   "Verify session ID is correct (format: session-XXXXXXXXXX)"
               ),
           });
       }

       // ... rest of loading logic
   }
   ```

### Architecture Changes

**New Error Module Structure**:
```
src/cook/workflow/checkpoint/
  ├── errors.rs                    // Checkpoint-specific errors
  ├── error_formatting.rs          // Error display helpers
  └── error_suggestions.rs         // Context-aware suggestions
```

**Error Formatting Helpers**:
```rust
// src/cook/workflow/checkpoint/error_formatting.rs

/// Mask secrets in error messages
pub fn mask_secret(value: &str) -> String {
    if value.len() <= 4 {
        "***".to_string()
    } else {
        format!("{}***", &value[..4])
    }
}

/// Format file path for display (relative to home if possible)
pub fn format_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(relative) = path.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}

/// Create suggestion based on error context
pub fn suggest_next_step(error_type: ErrorType, context: &ErrorContext) -> String {
    match error_type {
        ErrorType::CheckpointNotFound => {
            "Run 'prodigy checkpoints list' to see available checkpoints".to_string()
        }
        ErrorType::WorkflowFileNotFound => {
            format!(
                "Use 'prodigy resume {} --workflow-path <path>' to specify workflow location",
                context.session_id
            )
        }
        // ... other error types
    }
}
```

### Error Message Examples

#### Before (Generic):
```
Error: Failed to load checkpoint

Error: anyhow::Error: File not found

Error: Version mismatch
```

#### After (Specific and Actionable):

**Checkpoint Not Found**:
```
Checkpoint not found for session: session-abc123

Possible causes:
  • Checkpoint storage was cleared
  • Session ID is incorrect
  • Workflow failed before checkpoint could be saved

Checkpoint directory: ~/.prodigy/state/prodigy/checkpoints/

Suggestions:
  • List available sessions: prodigy sessions list --status Failed
  • Check checkpoint directory: ls -la ~/.prodigy/state/prodigy/checkpoints/
  • Verify session ID format (should be: session-XXXXXXXXXX)
```

**Workflow File Not Found**:
```
Workflow file not found at: ~/projects/prodigy/my-workflow.yml

The workflow file may have been moved, renamed, or deleted since the checkpoint was created.

Session ID: session-abc123
Checkpoint created: 2025-01-11 10:30:00 UTC

Suggestions:
  • Check if file exists: ls -la ~/projects/prodigy/my-workflow.yml
  • Search for workflow: find ~/projects/prodigy -name "*.yml"
  • Specify different path: prodigy resume session-abc123 --workflow-path <path>
  • View checkpoint details: prodigy checkpoints show session-abc123
```

**Version Incompatibility**:
```
Checkpoint version 5 is not supported (current version: 3)

This checkpoint was created by a newer version of Prodigy.
You need to upgrade Prodigy to resume from this checkpoint.

Checkpoint: ~/.prodigy/state/prodigy/checkpoints/session-abc123.json
Created: 2025-01-11 10:30:00 UTC

Suggestions:
  • Upgrade Prodigy: cargo install prodigy --force
  • Check current version: prodigy --version
  • View changelog: https://github.com/prodigy/releases

Note: Checkpoints created by newer versions cannot be loaded by older versions.
```

**Workflow Hash Mismatch**:
```
Workflow has changed since checkpoint was created

The workflow file has been modified, which may cause resume to behave unexpectedly.

Changes detected:
  • Step count changed: 5 → 7 (added 2 steps)
  • Workflow hash: abc123 → def456

Checkpoint: session-abc123
Workflow: ~/projects/prodigy/workflow.yml
Created: 2025-01-11 10:30:00 UTC

Suggestions:
  • Review workflow changes: git diff HEAD~1 workflow.yml
  • Restore original: git checkout HEAD~1 workflow.yml
  • Force resume (may skip/fail steps): prodigy resume session-abc123 --force

Warning: Force resume may produce unexpected results if workflow structure changed significantly.
```

**Resume Lock Held**:
```
Resume already in progress for session: session-abc123

Another process is currently resuming this workflow:
  • Process ID: 12345
  • Hostname: macbook-pro.local
  • Lock acquired: 2025-01-11 10:30:00 UTC (5 minutes ago)

Suggestions:
  • Wait for the other process to complete
  • Check if process is still running: ps aux | grep 12345
  • Force resume (if other process crashed): prodigy resume session-abc123 --force
  • View lock details: cat ~/.prodigy/resume_locks/session-abc123.lock

Warning: Using --force while another process is running may cause data corruption.
```

### Integration Points

1. **Error Propagation**
   - Use `thiserror` for structured errors
   - Convert to `anyhow` at boundaries
   - Preserve error context through propagation
   - Add context with `.context()` and `.with_context()`

2. **CLI Error Display**
   - Format errors consistently in CLI output
   - Use color coding for severity (red for errors, yellow for warnings)
   - Display suggestions prominently
   - Provide option for verbose error output (-vv)

3. **Logging Integration**
   - Log full error details (including stack trace)
   - Log error context for debugging
   - Include correlation IDs for tracing
   - Sanitize logs (remove secrets)

## Dependencies

### Prerequisites
- **Spec 134**: MapReduce Checkpoint and Resume (provides error contexts to enhance)
- **Spec 159**: MapReduce Resume CLI (provides CLI error display integration)

### Affected Components
- `src/cook/workflow/checkpoint/errors.rs` - New error types
- `src/cook/workflow/checkpoint.rs` - Error creation sites
- `src/cook/workflow/resume.rs` - Resume error handling
- `src/cli/commands/resume.rs` - CLI error display
- `src/cook/execution/mapreduce_resume.rs` - MapReduce error handling

### External Dependencies
- `thiserror` - For structured error types (already in use)
- `dirs` - For home directory detection in path formatting
- `colored` - For colored CLI output (optional enhancement)

## Testing Strategy

### Unit Tests
- Test error message formatting
- Test secret masking
- Test path formatting (relative to home)
- Test suggestion generation

### Integration Tests
- Test error messages in actual failure scenarios
- Verify error context is preserved through error chain
- Test CLI error display formatting
- Validate error messages contain expected elements

### User Acceptance Tests
- Review error messages with non-technical users
- Verify error messages are understandable
- Confirm suggestions are actionable
- Test that users can resolve errors following suggestions

### Example Test:
```rust
#[test]
fn test_checkpoint_not_found_error_message() {
    let error = CheckpointError::NotFound {
        session_id: "session-abc123".to_string(),
        checkpoint_dir: PathBuf::from("/home/user/.prodigy/checkpoints"),
        suggestion: "Verify session ID is correct".to_string(),
    };

    let message = error.to_string();

    // Verify all required elements present
    assert!(message.contains("session-abc123"));
    assert!(message.contains("Possible causes"));
    assert!(message.contains("Suggestions"));
    assert!(message.contains("prodigy sessions list"));
    assert!(message.contains("/home/user/.prodigy/checkpoints"));
}

#[test]
fn test_secret_masking() {
    let secret = "sk-abc123def456ghi789";
    let masked = mask_secret(&secret);

    // Verify secret is masked
    assert_eq!(masked, "sk-a***");
    assert!(!masked.contains("abc123"));
}
```

## Documentation Requirements

### Code Documentation
- Document error types with examples
- Add inline comments explaining error context requirements
- Document error formatting helpers
- Include usage examples in module documentation

### User Documentation
- Create troubleshooting guide with common errors
- Add error message reference to documentation
- Include examples of error resolution workflows
- Document `--force` flag risks and when to use it

### Architecture Updates
- Document error handling strategy
- Explain error type hierarchy
- Document error context requirements
- Include error handling best practices

## Implementation Notes

### Error Message Guidelines

1. **Structure**:
   - Start with clear statement of what went wrong
   - Provide context (IDs, paths, values)
   - List possible causes
   - Suggest concrete next steps
   - Include warnings for dangerous operations

2. **Language**:
   - Use active voice ("Checkpoint not found" vs "Checkpoint could not be found")
   - Be concise but complete
   - Avoid jargon unless necessary
   - Use bullet points for multiple items

3. **Formatting**:
   - Use blank lines to separate sections
   - Indent sub-items consistently
   - Use bold/color for emphasis (in CLI)
   - Include example commands in monospace

4. **Safety**:
   - Always mask secrets
   - Warn before destructive operations
   - Validate user actions when using `--force`
   - Sanitize paths if they contain sensitive info

### Error Context Best Practices

When creating errors, always include:
- Primary identifier (session ID, job ID, etc.)
- Relevant paths (checkpoint dir, workflow file)
- Timestamps (when created, last modified)
- Comparison values (expected vs actual)
- Actionable next steps

Example:
```rust
Err(CheckpointError::WorkflowFileNotFound {
    workflow_path: workflow_path.clone(),
    session_id: session_id.to_string(),
    suggestion: format!(
        "Specify location with: prodigy resume {} --workflow-path <path>",
        session_id
    ),
}.into())
```

## Migration and Compatibility

### Breaking Changes
- Error types change from `anyhow::Error` to structured types
- This is an internal change, not a breaking API change
- CLI error output format improves (minor visual change)

### Migration Requirements
- Update error creation sites to use new error types
- Convert to `anyhow::Error` at public API boundaries
- Update tests to expect new error message format

### Compatibility Considerations
- Old error messages preserved for backward compatibility where needed
- New error types wrap old errors when appropriate
- Error message improvements are additive (no information removed)

## Success Metrics

- 80% of users resolve errors without support (up from ~50%)
- Average time to resolve errors decreases by 40%
- Support requests related to resume errors decrease by 60%
- User satisfaction with error messages >4.0/5.0 (measured via survey)
- 100% of error messages include actionable suggestions
- Zero error messages expose secrets or sensitive data

## Future Enhancements (Out of Scope)

- Interactive error recovery (prompt user for next action)
- Error message localization (i18n)
- Context-sensitive help (man pages for specific errors)
- Error analytics (track common errors for UX improvement)
- Automated error resolution (self-healing for common issues)
- Error message A/B testing to optimize clarity
