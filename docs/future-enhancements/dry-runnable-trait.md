# DryRunnable Trait - Future Enhancement

## Overview

This document describes a future enhancement to unify dry-run behavior across the Prodigy codebase through a generic `DryRunnable` trait.

## Current State

As of spec 85 implementation, dry-run functionality is implemented using boolean flags (`dry_run: bool`) scattered across multiple modules:

- CLI commands (`src/cli/events.rs`, `src/main.rs`)
- Command handlers (`src/commands/handlers/*.rs`)
- Workflow execution (`src/cook/workflow/executor.rs`)
- Worktree management (`src/worktree/manager.rs`)
- Various utility modules

## Proposed Enhancement

### Phase 3: Generic DryRunnable Trait

Create a unified trait that provides consistent dry-run behavior:

```rust
trait DryRunnable {
    /// Execute in dry-run mode, returning preview of actions
    fn dry_run(&self) -> Result<DryRunPreview>;

    /// Check if this operation supports dry-run mode
    fn supports_dry_run(&self) -> bool {
        true
    }

    /// Execute the actual operation (non-dry-run)
    fn execute(&mut self) -> Result<()>;
}

struct DryRunPreview {
    /// Human-readable description of what would happen
    pub description: String,

    /// Structured data about the operation
    pub details: serde_json::Value,

    /// Estimated impact (files changed, commands run, etc.)
    pub impact: Impact,
}

struct Impact {
    pub files_modified: Vec<PathBuf>,
    pub commands_executed: Vec<String>,
    pub git_operations: Vec<String>,
    pub estimated_duration: Option<Duration>,
}
```

### Benefits

1. **Consistency**: Unified interface for dry-run across all components
2. **Type Safety**: Compile-time guarantees for dry-run support
3. **Better Testing**: Mock implementations for testing dry-run paths
4. **Documentation**: Clear contract for what dry-run means in each context
5. **Composability**: Complex operations can compose simpler dry-runnable components

### Implementation Areas

#### Command Handlers
```rust
impl DryRunnable for ClaudeHandler {
    fn dry_run(&self) -> Result<DryRunPreview> {
        // Return preview of Claude command execution
    }
}
```

#### Workflow Executor
```rust
impl DryRunnable for WorkflowExecutor {
    fn dry_run(&self) -> Result<DryRunPreview> {
        // Aggregate dry-run previews from all steps
    }
}
```

#### Event Cleanup
```rust
impl DryRunnable for EventCleaner {
    fn dry_run(&self) -> Result<DryRunPreview> {
        // Show retention analysis without deleting
    }
}
```

### Migration Strategy

1. Define the trait and core types
2. Implement for one subsystem (e.g., event cleanup)
3. Gradually migrate other subsystems
4. Remove boolean `dry_run` parameters in favor of trait methods
5. Update CLI to use trait-based dry-run

### Considerations

- Backward compatibility during migration
- Performance impact of preview generation
- Serialization format for preview data
- Integration with existing logging and output formats

## Timeline

This enhancement is marked for Phase 3 (post-MVP) implementation. Current dry-run functionality using boolean flags is sufficient for MVP and immediate needs.

## Related Work

- Spec 85: CLI dry-run mode (completed with boolean flag approach)
- Event retention analysis (already supports dry-run preview)
- Workflow validation (conceptually similar preview mechanism)