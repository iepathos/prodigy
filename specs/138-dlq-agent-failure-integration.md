---
number: 138
title: DLQ Integration for Failed MapReduce Agents
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-11
---

# Specification 138: DLQ Integration for Failed MapReduce Agents

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The Dead Letter Queue (DLQ) system exists in Prodigy but is not integrated with the MapReduce agent failure path. When agents fail during execution, they are tracked in `MapReduceJobState.failed_agents` but never added to the DLQ. This means:

- `prodigy dlq retry` has no items to retry
- Resume with `include_dlq_items: true` does nothing
- Failed work items are effectively lost after checkpoint
- No audit trail of failures for debugging

The DLQ infrastructure is complete and functional, but the orchestrator doesn't call `dlq.add()` when agents fail.

## Objective

Wire the MapReduce agent failure path to populate the Dead Letter Queue, enabling failed item tracking, retry functionality, and comprehensive failure auditing.

## Requirements

### Functional Requirements

- **FR1**: When an agent fails, automatically add the failed work item to the DLQ
- **FR2**: Capture complete failure context including error message, agent ID, and execution duration
- **FR3**: Preserve Claude JSON log location in DLQ entry for debugging
- **FR4**: Mark DLQ items as reprocess_eligible based on error type
- **FR5**: Create unique error signatures for failure pattern analysis
- **FR6**: Preserve worktree artifacts for manual debugging when configured

### Non-Functional Requirements

- **NFR1**: DLQ insertion must not cause agent processing to fail
- **NFR2**: Pure function for AgentResult → DeadLetteredItem conversion (fully testable)
- **NFR3**: No `unwrap()` or `panic!()` calls in production code
- **NFR4**: Graceful handling of DLQ write failures (log warning, continue execution)
- **NFR5**: Minimal performance impact (<5ms overhead per failed agent)

## Acceptance Criteria

- [ ] Pure function `agent_result_to_dlq_item()` converts AgentResult to DeadLetteredItem
- [ ] Orchestrator calls DLQ add on AgentStatus::Failed
- [ ] Orchestrator calls DLQ add on AgentStatus::Timeout
- [ ] DLQ entry includes json_log_location from AgentResult
- [ ] DLQ entry includes worktree_path for debugging
- [ ] Error signature is generated from error message
- [ ] Unit test: Pure conversion function with various error types
- [ ] Unit test: Success status returns None (no DLQ entry)
- [ ] Integration test: Failed agent appears in DLQ
- [ ] Integration test: `prodigy dlq retry` processes failed items
- [ ] Integration test: Resume with `include_dlq_items: true` loads failures
- [ ] All tests pass without modification
- [ ] No unwrap() or panic!() in new production code

## Technical Details

### Implementation Approach

**Step 1: Pure Conversion Function**

Create a pure function that converts `AgentResult` to `Option<DeadLetteredItem>`:

```rust
// src/cook/execution/mapreduce/dlq_integration.rs
pub fn agent_result_to_dlq_item(
    result: &AgentResult,
    work_item: &Value,
    attempt_number: u32,
) -> Option<DeadLetteredItem> {
    match &result.status {
        AgentStatus::Failed(error_msg) | AgentStatus::Timeout => {
            Some(DeadLetteredItem {
                item_id: result.item_id.clone(),
                item_data: work_item.clone(),
                first_attempt: Utc::now(),
                last_attempt: Utc::now(),
                failure_count: 1,
                failure_history: vec![create_failure_detail(result, attempt_number)],
                error_signature: create_error_signature(error_msg),
                worktree_artifacts: extract_worktree_artifacts(result),
                reprocess_eligible: is_reprocessable(result),
                manual_review_required: requires_manual_review(result),
            })
        }
        _ => None,
    }
}
```

**Step 2: Helper Pure Functions**

```rust
fn create_failure_detail(result: &AgentResult, attempt: u32) -> FailureDetail {
    let error_msg = match &result.status {
        AgentStatus::Failed(msg) => msg.clone(),
        AgentStatus::Timeout => "Agent execution timed out".to_string(),
        _ => "Unknown error".to_string(),
    };

    FailureDetail {
        attempt_number: attempt,
        timestamp: Utc::now(),
        error_type: classify_error(&error_msg),
        error_message: error_msg,
        stack_trace: result.error.clone(),
        agent_id: format!("agent-{}", result.item_id),
        step_failed: "agent_execution".to_string(),
        duration_ms: result.duration.as_millis() as u64,
        json_log_location: result.json_log_location.clone(),
    }
}

fn create_error_signature(error_msg: &str) -> String {
    // Generate consistent signature for pattern analysis
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(error_msg.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn classify_error(error_msg: &str) -> ErrorType {
    if error_msg.contains("timeout") || error_msg.contains("timed out") {
        ErrorType::Timeout
    } else if error_msg.contains("permission") || error_msg.contains("access denied") {
        ErrorType::PermissionError
    } else {
        ErrorType::CommandFailed {
            command: "agent_execution".to_string(),
            exit_code: None,
        }
    }
}

fn extract_worktree_artifacts(result: &AgentResult) -> Option<WorktreeArtifacts> {
    result.worktree_path.as_ref().map(|path| WorktreeArtifacts {
        worktree_path: path.clone(),
        preserved: true,
    })
}

fn is_reprocessable(result: &AgentResult) -> bool {
    // Timeouts and temporary errors are reprocessable
    matches!(
        result.status,
        AgentStatus::Timeout | AgentStatus::Failed(_)
    )
}

fn requires_manual_review(result: &AgentResult) -> bool {
    // Permission errors or critical failures need manual review
    if let AgentStatus::Failed(msg) = &result.status {
        msg.contains("permission") || msg.contains("critical")
    } else {
        false
    }
}
```

**Step 3: Orchestrator Integration**

Add DLQ handling to orchestrator's agent completion handler:

```rust
// src/cook/execution/mapreduce/coordination/*.rs
pub async fn handle_agent_completion(
    result: AgentResult,
    work_item: &Value,
    state_manager: &Arc<dyn JobStateManager>,
    dlq: &Arc<DeadLetterQueue>,
    attempt_number: u32,
) -> Result<()> {
    // Update state first
    state_manager
        .update_agent_result(&result)
        .await
        .context("Failed to update agent result in state")?;

    // If failed, add to DLQ (graceful failure)
    if let Some(dlq_item) = agent_result_to_dlq_item(&result, work_item, attempt_number) {
        if let Err(e) = dlq.add(dlq_item).await {
            // Don't fail the workflow, just log
            warn!(
                "Failed to add item {} to DLQ: {}. Item tracking may be incomplete.",
                result.item_id, e
            );
        } else {
            info!(
                "Added failed item {} to DLQ (attempt {})",
                result.item_id, attempt_number
            );
        }
    }

    Ok(())
}
```

### Architecture Changes

**New Module**: `src/cook/execution/mapreduce/dlq_integration.rs`
- Contains all pure conversion functions
- No I/O, fully testable
- Exports `agent_result_to_dlq_item()` as public API

**Modified Modules**:
- `src/cook/execution/mapreduce/coordination/*.rs` - Add DLQ handling
- `src/cook/execution/mapreduce/orchestrator.rs` - Call handle_agent_completion

### Data Structures

No new data structures required. Uses existing:
- `AgentResult` (input)
- `DeadLetteredItem` (output)
- `FailureDetail`
- `ErrorType`
- `WorktreeArtifacts`

### APIs and Interfaces

**New Public Function**:
```rust
pub fn agent_result_to_dlq_item(
    result: &AgentResult,
    work_item: &Value,
    attempt_number: u32,
) -> Option<DeadLetteredItem>
```

**Modified Behavior**:
- MapReduce orchestrator now populates DLQ on agent failure
- `prodigy dlq retry` can now process failed MapReduce items
- Resume with `include_dlq_items: true` now includes actual failures

## Dependencies

- **Prerequisites**: None (DLQ system already exists)
- **Affected Components**:
  - MapReduce orchestrator
  - Agent execution flow
  - DLQ system (consumers only)
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

**Test File**: `src/cook/execution/mapreduce/dlq_integration_tests.rs`

```rust
#[test]
fn test_agent_result_to_dlq_item_failed() {
    let result = create_failed_agent_result("Test error");
    let work_item = json!({"id": 1, "data": "test"});

    let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

    assert!(dlq_item.is_some());
    let item = dlq_item.unwrap();
    assert_eq!(item.item_id, "item-1");
    assert_eq!(item.failure_count, 1);
    assert_eq!(item.failure_history.len(), 1);
    assert!(item.reprocess_eligible);
}

#[test]
fn test_agent_result_to_dlq_item_timeout() {
    let result = create_timeout_agent_result();
    let work_item = json!({"id": 2, "data": "test"});

    let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

    assert!(dlq_item.is_some());
    let item = dlq_item.unwrap();
    assert!(item.reprocess_eligible);
    assert_eq!(item.error_signature.len(), 16);
}

#[test]
fn test_agent_result_to_dlq_item_success_returns_none() {
    let result = create_successful_agent_result();
    let work_item = json!({"id": 3, "data": "test"});

    let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

    assert!(dlq_item.is_none());
}

#[test]
fn test_error_signature_consistency() {
    let msg = "Test error message";
    let sig1 = create_error_signature(msg);
    let sig2 = create_error_signature(msg);

    assert_eq!(sig1, sig2);
    assert_eq!(sig1.len(), 16);
}

#[test]
fn test_classify_error_timeout() {
    let error = "Operation timed out after 30s";
    let error_type = classify_error(error);

    assert!(matches!(error_type, ErrorType::Timeout));
}

#[test]
fn test_classify_error_permission() {
    let error = "Permission denied: cannot write to file";
    let error_type = classify_error(error);

    assert!(matches!(error_type, ErrorType::PermissionError));
}
```

### Integration Tests

**Test File**: `tests/dlq_agent_integration_test.rs`

```rust
#[tokio::test]
async fn test_failed_agent_populates_dlq() {
    // Setup MapReduce job with item that will fail
    // Execute job
    // Verify DLQ contains failed item
    // Verify failure details are complete
}

#[tokio::test]
async fn test_dlq_retry_processes_failed_items() {
    // Create job with failures in DLQ
    // Run `prodigy dlq retry <job_id>`
    // Verify items are reprocessed
    // Verify DLQ is updated (removed on success, kept on re-failure)
}

#[tokio::test]
async fn test_resume_includes_dlq_items() {
    // Create interrupted job with DLQ items
    // Resume with include_dlq_items: true
    // Verify DLQ items are included in work queue
}

#[tokio::test]
async fn test_json_log_location_preserved_in_dlq() {
    // Run job with failing agent
    // Check DLQ entry
    // Verify json_log_location is present and valid
}
```

### Performance Tests

```rust
#[tokio::test]
async fn test_dlq_integration_performance() {
    // Process 100 failed agents
    // Measure time for DLQ insertions
    // Assert <5ms average overhead per failure
}
```

## Documentation Requirements

### Code Documentation

- Add module-level documentation to `dlq_integration.rs`
- Document all public functions with examples
- Add inline comments explaining error classification logic

### User Documentation

Update `CLAUDE.md`:

```markdown
## DLQ Integration with Failed Agents

When MapReduce agents fail, they are automatically added to the Dead Letter Queue (DLQ):

**Automatic DLQ Population:**
- Agent failures (AgentStatus::Failed)
- Agent timeouts (AgentStatus::Timeout)
- Includes full error context and Claude JSON log location

**DLQ Entry Contents:**
- Work item data
- Error message and classification
- Execution duration
- Claude JSON log path (for debugging)
- Worktree path (if preserved)
- Error signature (for pattern analysis)

**Retry Failed Items:**
```bash
# List failed items
prodigy dlq show <job_id>

# Retry all failed items
prodigy dlq retry <job_id>

# Resume job including DLQ items
prodigy resume <job_id>  # include_dlq_items: true by default
```

**Debugging Failed Agents:**
1. Find JSON log location in DLQ entry
2. View Claude interaction: `cat <json_log_location> | jq`
3. Inspect worktree if preserved: `cd <worktree_path>`
```

### Architecture Updates

Add to architecture documentation:

```markdown
## MapReduce Agent Failure Handling

**Flow:**
1. Agent executes work item
2. On failure (error or timeout):
   - AgentResult status set to Failed/Timeout
   - State manager updates failed_agents
   - Pure conversion: AgentResult → DeadLetteredItem
   - DLQ.add() called (graceful failure)
   - Event logged: AgentFailed
3. DLQ entry preserved for retry and debugging

**Pure Functions:**
- `agent_result_to_dlq_item()` - Main conversion
- `create_failure_detail()` - Build FailureDetail
- `create_error_signature()` - Consistent error hashing
- `classify_error()` - ErrorType classification
```

## Implementation Notes

### Error Handling Best Practices

1. **No Unwrap/Panic**: All fallible operations use `?` or explicit error handling
2. **Graceful DLQ Failures**: DLQ write failures log warnings but don't fail workflow
3. **Context Propagation**: Use `.context()` to add error details

### Testing Checklist

- [ ] All pure functions tested in isolation
- [ ] Integration test for complete flow
- [ ] Test with various error types
- [ ] Test DLQ retry integration
- [ ] Test resume with DLQ items
- [ ] Performance test for overhead

### Gotchas

- **Timing**: Use `Utc::now()` for timestamps, not system time
- **Error Signatures**: Use SHA256 hash, not raw error messages
- **Worktree Preservation**: Check configuration before preserving artifacts
- **Attempt Numbers**: Track across retries for accurate failure count

## Migration and Compatibility

### Breaking Changes

None. This is additive functionality.

### Compatibility Considerations

- Existing MapReduce jobs will start populating DLQ on next failure
- Old DLQ entries (if any exist) remain valid
- Resume behavior enhanced but backward compatible

### Migration Steps

1. Deploy new code
2. No database migrations required
3. Existing failed_agents in state remain tracked
4. New failures automatically populate DLQ

### Rollback Plan

If issues arise:
1. Code is backward compatible (DLQ population is optional)
2. Remove orchestrator DLQ calls
3. Pure functions remain for future use
4. No data cleanup required
