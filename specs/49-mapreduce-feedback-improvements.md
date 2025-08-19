---
number: 49
title: MapReduce Workflow Feedback Improvements
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-08-19
---

# Specification 49: MapReduce Workflow Feedback Improvements

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current MapReduce workflow implementation provides minimal feedback during execution, making it difficult for users to understand progress and debug issues. The feedback shows:
- A static timer `[00:00:00]` that doesn't increment
- Generic item identifiers like `item_0`, `item_1` that lack descriptive context
- No visibility into what commands each agent is executing (Claude commands, shell commands, etc.)
- Limited information about setup phase progress

This lack of feedback makes it challenging to:
- Monitor progress during long-running workflows
- Identify which items are being processed
- Debug failures when they occur
- Understand what operations are happening in real-time

## Objective

Enhance the MapReduce workflow execution feedback to provide real-time, informative progress updates that help users understand exactly what's happening during workflow execution, including dynamic timers, descriptive item identification, and detailed agent operation visibility.

## Requirements

### Functional Requirements

#### Timer and Progress Updates
- Display a dynamically updating timer that increments in real-time during execution
- Update progress bars at least every 100ms to show elapsed time
- Show both elapsed time and estimated time remaining when possible
- Maintain thread-safe progress bar updates from multiple async tasks

#### Item Identification
- Extract and display meaningful identifiers from JSON work items
- Use common identifier fields in priority order: `id`, `name`, `title`, `path`, `file`
- Format items descriptively: `Processing auth_module` instead of `Processing item_0`
- Fall back to index-based naming (`item_0`) only when no identifier is found
- Truncate long identifiers to maintain readable output

#### Agent Operation Visibility
- Display current operation type for each agent: `[setup]`, `[claude]`, `[shell]`, `[test]`
- Show the specific command being executed: `Agent 1: [shell] just coverage-lcov`
- Include step progress within agent execution: `Agent 1: Step 2/5 - Running tests`
- Update agent status in real-time as operations change
- Show retry attempts when operations fail: `Agent 2: Retrying (attempt 2/3)`

#### Setup Phase Feedback
- Display individual setup commands as they execute
- Show command type and description: `Setup step 1/2: shell: just coverage-lcov`
- Display stdout/stderr for failed setup steps to aid debugging
- Add progress spinners for long-running setup commands
- Provide clear error messages when setup fails

### Non-Functional Requirements

#### Performance
- Timer updates must not impact MapReduce execution performance
- Progress bar updates should use minimal CPU resources
- Maintain responsive feedback even with many parallel agents

#### Compatibility
- Ensure progress bars work correctly in different terminal environments
- Support both interactive and non-interactive terminal modes
- Gracefully degrade in CI/CD environments without TTY

#### User Experience
- Keep output concise while being informative
- Use color coding to indicate agent states (green=success, yellow=running, red=failed)
- Clean up progress bars properly on completion or error
- Provide summary statistics after execution

## Acceptance Criteria

- [ ] Timer shows real-time elapsed time that updates at least every second
- [ ] Work items display meaningful identifiers extracted from JSON data
- [ ] Each agent shows its current operation type and specific command
- [ ] Setup phase displays individual command progress with descriptive labels
- [ ] Failed setup steps show stdout/stderr for debugging
- [ ] Progress bars are properly cleaned up after execution
- [ ] Output remains readable with up to 10 parallel agents
- [ ] Performance impact of progress updates is less than 1% CPU usage
- [ ] Works correctly in both interactive terminals and CI environments
- [ ] Retry attempts are clearly indicated in agent status

## Technical Details

### Implementation Approach

#### Progress Bar Architecture
- Use `indicatif::MultiProgress` for managing multiple progress bars
- Create one overall progress bar showing total item completion
- Create individual progress bars for each active agent
- Use `enable_steady_tick(100)` for smooth timer updates
- Implement proper cleanup with `finish_and_clear()` methods

#### Timer Implementation
```rust
// Add periodic timer updates
let progress_clone = progress.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        progress_clone.overall_bar.tick();
        if progress_clone.is_finished() {
            break;
        }
    }
});
```

#### Item Identification Logic
```rust
fn extract_item_identifier(item: &Value, index: usize) -> String {
    // Priority order for identifier fields
    let id_fields = ["id", "name", "title", "path", "file", "key"];
    
    if let Value::Object(obj) = item {
        for field in &id_fields {
            if let Some(Value::String(s)) = obj.get(*field) {
                return truncate_identifier(s, 30);
            }
        }
    }
    
    // Fallback to index
    format!("item_{}", index)
}
```

#### Agent Status Updates
```rust
enum AgentOperation {
    Setup(String),      // Setup phase command
    Claude(String),     // Claude command
    Shell(String),      // Shell command
    Test(String),       // Test command
    Handler(String),    // Handler command
    Idle,              // Waiting for work
    Complete,          // Finished processing
}

fn format_agent_status(agent_index: usize, op: &AgentOperation, step: Option<(usize, usize)>) -> String {
    let step_info = step.map(|(current, total)| format!(" Step {}/{}", current, total))
        .unwrap_or_default();
    
    match op {
        AgentOperation::Claude(cmd) => format!("Agent {:2}: [claude]{} {}", agent_index + 1, step_info, cmd),
        AgentOperation::Shell(cmd) => format!("Agent {:2}: [shell]{} {}", agent_index + 1, step_info, cmd),
        // ... other operations
    }
}
```

### Architecture Changes

#### ProgressTracker Enhancement
- Add `tick_handle: Option<JoinHandle<()>>` for timer task
- Add `start_time: Instant` for elapsed time calculation
- Add `is_finished: Arc<AtomicBool>` for cleanup coordination
- Implement `update_agent_operation()` method for detailed status

#### AgentContext Extension
- Add `current_step: usize` and `total_steps: usize` fields
- Add `current_operation: AgentOperation` field
- Track retry attempts for display

#### MapReduceExecutor Updates
- Spawn timer update task in `execute_map_phase()`
- Update `run_agent()` to report detailed operation status
- Enhance `execute_agent_commands()` to track step progress
- Improve `execute_single_step()` to update operation type

### Data Structures

```rust
struct EnhancedProgressTracker {
    multi_progress: MultiProgress,
    overall_bar: ProgressBar,
    agent_bars: Vec<ProgressBar>,
    tick_handle: Option<JoinHandle<()>>,
    start_time: Instant,
    is_finished: Arc<AtomicBool>,
    agent_operations: Arc<RwLock<Vec<AgentOperation>>>,
}

impl EnhancedProgressTracker {
    fn new(total_items: usize, max_parallel: usize) -> Self;
    fn start_timer(&self);
    fn update_agent_operation(&self, agent_index: usize, operation: AgentOperation);
    fn update_agent_progress(&self, agent_index: usize, current_step: usize, total_steps: usize);
    fn finish(&self, message: &str);
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/execution/mapreduce.rs` - Main implementation
  - `src/cook/workflow/executor.rs` - Setup phase display
  - `src/cook/orchestrator.rs` - Workflow initialization
- **External Dependencies**: 
  - `indicatif` crate (already in use)
  - `tokio` for async timer tasks (already in use)

## Testing Strategy

### Unit Tests
- Test item identifier extraction with various JSON structures
- Verify timer update task lifecycle
- Test agent operation formatting
- Validate progress bar cleanup

### Integration Tests
- Test with workflows containing 1, 5, and 10 parallel agents
- Verify output in both TTY and non-TTY environments
- Test with various item data structures
- Validate retry attempt display

### Performance Tests
- Measure CPU usage of timer updates with 10 agents
- Verify no memory leaks from progress bar handles
- Test with long-running workflows (>10 minutes)

### User Acceptance
- Clear, informative output during workflow execution
- Ability to identify which items are being processed
- Understanding of current operations per agent
- Useful error messages when setup fails

## Documentation Requirements

### Code Documentation
- Document new ProgressTracker methods and fields
- Add examples of item identifier extraction
- Document agent operation state machine

### User Documentation
- Update workflow documentation with example output
- Add troubleshooting section for progress display issues
- Document environment variables affecting display

### Architecture Updates
- Update ARCHITECTURE.md with enhanced feedback system
- Document progress bar threading model

## Implementation Notes

### Phase 1: Timer Updates (Priority: Critical)
- Implement periodic timer updates with tokio task
- Ensure thread-safe progress bar access
- Add proper cleanup on completion

### Phase 2: Item Identification (Priority: High)
- Implement intelligent identifier extraction
- Add truncation for long identifiers
- Maintain backwards compatibility with index fallback

### Phase 3: Agent Operation Visibility (Priority: High)
- Track and display current operation per agent
- Show step progress within workflows
- Add retry attempt indicators

### Phase 4: Setup Phase Enhancement (Priority: Medium)
- Improve setup command display
- Add progress indicators for long operations
- Show error details on failure

## Migration and Compatibility

### Breaking Changes
- None - all changes are additive improvements

### Compatibility Considerations
- Maintain support for non-TTY environments
- Ensure CI/CD pipelines continue to work
- Preserve existing log output format options

### Performance Considerations
- Timer updates use separate async task to avoid blocking
- Progress bar updates batched to minimize overhead
- Careful memory management of progress bar handles

## Success Metrics

- User satisfaction with workflow visibility
- Reduction in debugging time for failed workflows
- Improved ability to monitor long-running workflows
- No measurable performance impact on execution time