---
number: 168
title: Stillwater Error Context Preservation
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-11-23
---

# Specification 168: Stillwater Error Context Preservation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy's error messages frequently lack context about what operations were being performed when failures occurred. This is particularly problematic in MapReduce workflows where errors bubble up through multiple layers:

**Current State**:
```
Error: File not found: process.sh
```

**Problem**: No indication of:
- Which MapReduce job failed
- Which work item was being processed
- What phase of execution (setup, map, reduce)
- What operation was attempted
- Full operation trail leading to failure

This makes debugging DLQ failures extremely difficult and time-consuming.

## Objective

Implement automatic error context preservation using Stillwater's `ContextError<E>` type, providing comprehensive error trails that show the complete operation context when failures occur.

## Requirements

### Functional Requirements

1. **Automatic Context Accumulation**
   - Preserve error context as errors propagate through call stack
   - Display context trail with clear visual formatting
   - Maintain underlying error for pattern matching

2. **Context at Key Boundaries**
   - Command execution (shell, Claude, test)
   - Work item processing
   - Phase transitions (setup → map → reduce)
   - File I/O operations
   - Git operations

3. **DLQ Integration**
   - Store full context trail in DLQ items
   - Display context when viewing failed items
   - Include Claude JSON log location in context

### Non-Functional Requirements

1. **Zero Runtime Overhead**: Context wrapping should add no performance cost
2. **Type Safety**: Preserve underlying error types for pattern matching
3. **Backward Compatibility**: Existing error handling code continues to work
4. **Clarity**: Context messages should be concise and actionable

## Acceptance Criteria

- [ ] `ContextError<E>` wrapper type implemented with Stillwater integration
- [ ] `.context()` extension method available on all `Result` types
- [ ] Error context added to all command execution paths
- [ ] Error context added to all MapReduce agent operations
- [ ] Error context added to all file I/O operations
- [ ] Error context added to all git operations
- [ ] DLQ schema updated to store context trails
- [ ] DLQ display shows full error context with formatting
- [ ] `prodigy dlq show` command displays context trails
- [ ] Error messages show context with `->` separators
- [ ] Underlying errors remain accessible for pattern matching
- [ ] No performance regression in error-free paths
- [ ] 15+ integration tests verify context preservation
- [ ] Documentation updated with error context architecture

## Technical Details

### Implementation Approach

**Phase 1: Context Error Type**
```rust
// src/cook/error/context.rs

use stillwater::ContextError;

/// Extension trait for adding context to Results
pub trait ResultExt<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T, ContextError<E>>;
    fn with_context<F>(self, f: F) -> Result<T, ContextError<E>>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T, ContextError<E>> {
        self.map_err(|e| ContextError::new(e).context(msg))
    }

    fn with_context<F>(self, f: F) -> Result<T, ContextError<E>>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ContextError::new(e).context(f()))
    }
}

/// Alias for context-aware results
pub type ContextResult<T, E> = Result<T, ContextError<E>>;
```

**Phase 2: Command Execution Context**
```rust
// src/cook/workflow/executor/commands.rs

use crate::cook::error::ResultExt;

pub async fn execute_shell_command(
    cmd: &ShellCommand,
    ctx: &ExecutionContext,
) -> ContextResult<CommandOutput, CommandError> {
    prepare_shell_environment(cmd)
        .context("Preparing shell environment")?;

    interpolate_command_variables(cmd, ctx)
        .context("Interpolating command variables")?;

    run_subprocess(&cmd.command)
        .await
        .with_context(|| format!("Executing shell command: {}", cmd.command))?;

    Ok(output)
}
```

**Phase 3: MapReduce Agent Context**
```rust
// src/cook/execution/mapreduce/agent/execution.rs

pub async fn execute_agent(
    item: WorkItem,
    config: &AgentConfig,
) -> ContextResult<AgentResult, AgentError> {
    create_agent_worktree(&item.id)
        .await
        .with_context(|| format!("Creating worktree for item '{}'", item.id))?;

    interpolate_agent_commands(&item)
        .context("Interpolating agent commands")?;

    execute_agent_commands(&item)
        .await
        .with_context(|| format!("Executing commands for item '{}'", item.id))?;

    validate_agent_commits(&item)
        .with_context(|| format!("Validating commits for item '{}'", item.id))?;

    Ok(result)
}
```

**Phase 4: DLQ Integration**
```rust
// src/cook/execution/dlq/mod.rs

use stillwater::ContextError;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeadLetteredItem {
    pub item_id: String,
    pub error: String,
    pub error_context: Vec<String>,  // NEW: Full context trail
    pub error_type: String,
    pub json_log_location: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub retry_count: usize,
}

impl DeadLetteredItem {
    pub fn from_error<E: Display>(
        item_id: String,
        error: ContextError<E>,
        json_log: Option<String>,
    ) -> Self {
        Self {
            item_id,
            error: error.inner().to_string(),
            error_context: error.context_trail().to_vec(),  // Extract trail
            error_type: std::any::type_name::<E>().to_string(),
            json_log_location: json_log,
            timestamp: Utc::now(),
            retry_count: 0,
        }
    }
}
```

**Phase 5: Error Display**
```rust
// src/cook/error/display.rs

impl Display for ContextError<ProdigyError> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Show base error
        writeln!(f, "Error: {}", self.inner())?;

        // Show context trail
        if !self.context_trail().is_empty() {
            writeln!(f)?;
            writeln!(f, "Context:")?;
            for context in self.context_trail() {
                writeln!(f, "  -> {}", context)?;
            }
        }

        Ok(())
    }
}
```

### Architecture Changes

**New Module Structure**:
```
src/cook/error/
├── mod.rs          (existing - main error types)
├── context.rs      (NEW - ContextError integration)
├── display.rs      (NEW - error formatting)
└── ext.rs          (NEW - ResultExt trait)
```

**Error Type Hierarchy**:
```
ContextError<ProdigyError>
    ├─ Context trail: Vec<String>
    └─ Inner error: ProdigyError
        ├─ CommandError
        ├─ AgentError
        ├─ WorkflowError
        └─ ... (existing variants)
```

### Data Structures

```rust
/// Enhanced DLQ item with context
#[derive(Debug, Serialize, Deserialize)]
pub struct DeadLetteredItem {
    pub item_id: String,
    pub error: String,
    pub error_context: Vec<String>,  // NEW
    pub error_type: String,
    pub json_log_location: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub retry_count: usize,
    pub manual_review_required: bool,
}
```

### APIs and Interfaces

**Public API - Extension Trait**:
```rust
pub trait ResultExt<T, E> {
    /// Add static context to error
    fn context(self, msg: impl Into<String>) -> Result<T, ContextError<E>>;

    /// Add dynamic context to error
    fn with_context<F>(self, f: F) -> Result<T, ContextError<E>>
    where
        F: FnOnce() -> String;
}
```

**Usage Pattern**:
```rust
// Static context
do_something()
    .context("Operation description")?;

// Dynamic context (lazy evaluation)
do_something_with_id(id)
    .with_context(|| format!("Processing item {}", id))?;
```

## Dependencies

### Prerequisites
- Stillwater library with `ContextError<E>` type
- Understanding of error wrapping and context preservation

### Affected Components
- All command execution paths (`cook/workflow/executor/`)
- All MapReduce agent operations (`cook/execution/mapreduce/`)
- DLQ storage and display (`cook/execution/dlq/`)
- Error display throughout codebase

### External Dependencies
- `stillwater = "0.1"` (ContextError type)

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_context_preservation() {
    fn inner() -> Result<(), String> {
        Err("base error".to_string())
    }

    fn middle() -> ContextResult<(), String> {
        inner().context("middle operation")
    }

    fn outer() -> ContextResult<(), String> {
        middle().context("outer operation")
    }

    let result = outer();
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert_eq!(error.inner(), "base error");
    assert_eq!(error.context_trail().len(), 2);
    assert!(error.context_trail().contains(&"middle operation".to_string()));
    assert!(error.context_trail().contains(&"outer operation".to_string()));
}

#[test]
fn test_error_display_format() {
    let error = ContextError::new("File not found")
        .context("Reading configuration")
        .context("Initializing workflow");

    let display = format!("{}", error);

    assert!(display.contains("Error: File not found"));
    assert!(display.contains("-> Reading configuration"));
    assert!(display.contains("-> Initializing workflow"));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_command_execution_context() {
    let cmd = ShellCommand {
        command: "nonexistent-command".to_string(),
    };

    let result = execute_shell_command(&cmd, &test_context()).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Verify context includes operation details
    let context_str = format!("{}", error);
    assert!(context_str.contains("Executing shell command"));
    assert!(context_str.contains("nonexistent-command"));
}

#[tokio::test]
async fn test_dlq_stores_context() {
    let item = test_work_item();
    let error = ContextError::new(AgentError::CommandFailed)
        .context(format!("Processing item {}", item.id))
        .context("Executing map phase");

    let dlq_item = DeadLetteredItem::from_error(
        item.id.clone(),
        error,
        Some("/path/to/log.json".to_string()),
    );

    assert_eq!(dlq_item.error_context.len(), 2);
    assert!(dlq_item.error_context.contains(&"Processing item test-item".to_string()));
    assert!(dlq_item.error_context.contains(&"Executing map phase".to_string()));
}
```

### End-to-End Tests

```rust
#[tokio::test]
async fn test_mapreduce_error_context_e2e() {
    let workflow = create_failing_mapreduce_workflow();

    let result = execute_workflow(workflow).await;

    assert!(result.is_err());

    // Check DLQ for context
    let dlq_items = load_dlq_items(&workflow.job_id).await.unwrap();
    assert!(!dlq_items.is_empty());

    let first_failure = &dlq_items[0];
    assert!(!first_failure.error_context.is_empty());

    // Verify context shows operation trail
    let context = &first_failure.error_context;
    assert!(context.iter().any(|c| c.contains("map phase")));
    assert!(context.iter().any(|c| c.contains("item")));
}
```

## Documentation Requirements

### Code Documentation

```rust
/// Extension trait for adding context to errors
///
/// # Examples
///
/// ```
/// use prodigy::cook::error::ResultExt;
///
/// fn process_file(path: &Path) -> ContextResult<Data, IoError> {
///     read_file(path)
///         .context("Reading input file")?;
///
///     parse_data(&contents)
///         .with_context(|| format!("Parsing file {}", path.display()))?;
///
///     Ok(data)
/// }
/// ```
pub trait ResultExt<T, E> { ... }
```

### User Documentation

Update `CLAUDE.md`:
```markdown
## Error Context Preservation

Prodigy uses Stillwater's ContextError to preserve operation context:

**Error Format**:
```
Error: File not found: work_items.json
Context:
  -> Loading work items
  -> Preparing map phase
  -> Executing MapReduce job: process-items
```

**DLQ Integration**:
Failed MapReduce items include full context trails for debugging.

Use `prodigy dlq show <job_id>` to view error context.
```

### Architecture Updates

Add to `ARCHITECTURE.md`:
```markdown
## Error Context Architecture

### Context Preservation

All errors use `ContextError<E>` wrapper to preserve operation context:

- **Automatic**: `.context()` method adds context at each layer
- **Trails**: Full context path shown in error messages
- **Debugging**: DLQ items include complete context
- **Type-Safe**: Underlying errors remain accessible

### Best Practices

1. Add context at operation boundaries
2. Use descriptive, actionable messages
3. Include relevant IDs (item, job, session)
4. Keep context messages concise (<80 chars)

### Example

```rust
create_worktree(&session_id)
    .with_context(|| format!("Creating worktree for session {}", session_id))?;
```
```

## Implementation Notes

### Context Message Guidelines

**Good Context Messages**:
- "Executing shell command: cargo test"
- "Processing work item item-42"
- "Validating commits for agent agent-1"
- "Saving checkpoint for job job-123"

**Poor Context Messages**:
- "Error" (not descriptive)
- "Something went wrong" (not actionable)
- "In execute_command function" (too technical)

### Performance Considerations

- **Zero Cost When Successful**: No overhead if no error occurs
- **Lazy Context**: Use `.with_context(|| ...)` for expensive string formatting
- **String Allocation**: Context messages allocated only on error path

### Migration Strategy

1. **Start with Hot Paths**: Add context to frequently-failing operations first
2. **Layer by Layer**: Add context at architectural boundaries (commands, agents, phases)
3. **Test Coverage**: Verify context appears in tests
4. **Documentation**: Update error handling guidelines

## Migration and Compatibility

### Breaking Changes
None - error types remain compatible.

### New Behavior
- Error messages now include context trails
- DLQ items store additional context field
- Error display format enhanced (additive)

### Migration Path

**Phase 1: Foundation** (Week 1)
- Add ContextError wrapper type
- Create ResultExt trait
- Add basic tests

**Phase 2: Integration** (Week 1-2)
- Add context to command execution (20-30 files)
- Add context to MapReduce operations (10-15 files)
- Update DLQ schema

**Phase 3: Refinement** (Week 2)
- Improve context messages based on feedback
- Add integration tests
- Update documentation

### Backward Compatibility

- Existing error handling code continues to work
- Pattern matching on underlying errors still works
- Error display enhanced but not breaking
- DLQ format extended (new field, old fields unchanged)
