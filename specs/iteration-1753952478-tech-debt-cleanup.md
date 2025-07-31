# Iteration N: Technical Debt Cleanup

## Overview
Temporary specification for technical debt cleanup identified from comprehensive codebase analysis.

## Debt Items to Address

### 1. Excessive Function Length and Complexity in main.rs
**Impact Score**: 9/10
**Effort Score**: 6/10
**Category**: Complexity
**File**: src/main.rs:249-392
**Priority**: High

#### Current State:
```rust
pub fn run_worktree_command(command: WorktreeSubcommand) -> Result<()> {
    // 143 lines of deeply nested code handling:
    // - Display logic
    // - Merge operations
    // - Cleanup operations
    // Multiple responsibilities and duplicated display patterns
    match command {
        WorktreeSubcommand::Display { focus } => {
            // Complex nested logic with duplicate session display code
            if let Some(focus) = focus {
                // ... 20+ lines of display logic
            } else {
                // ... duplicate display logic
            }
        }
        // ... more cases
    }
}
```

#### Required Changes:
```rust
// Extract to separate functions:
fn handle_display_command(focus: Option<String>) -> Result<()> {
    let formatter = SessionFormatter::new();
    let sessions = load_worktree_sessions()?;
    
    if let Some(focus) = focus {
        display_filtered_sessions(&sessions, &focus, &formatter)?;
    } else {
        display_all_sessions(&sessions, &formatter)?;
    }
    Ok(())
}

fn handle_merge_command(merge_type: MergeType) -> Result<()> {
    // Focused merge logic
}

fn handle_cleanup_command(force: bool, pattern: Option<String>) -> Result<()> {
    // Focused cleanup logic
}

pub fn run_worktree_command(command: WorktreeSubcommand) -> Result<()> {
    match command {
        WorktreeSubcommand::Display { focus } => handle_display_command(focus),
        WorktreeSubcommand::Merge(merge_type) => handle_merge_command(merge_type),
        WorktreeSubcommand::Cleanup { force, pattern } => handle_cleanup_command(force, pattern),
    }
}
```

#### Implementation Steps:
- Extract `SessionFormatter` struct to handle all session display logic
- Create `handle_display_command()`, `handle_merge_command()`, and `handle_cleanup_command()` functions
- Extract duplicate session display logic into `format_session_display()` method
- Add proper error context to each operation
- Run `cargo clippy` to verify complexity reduction

### 2. Inconsistent Error Handling with Excessive unwrap() Usage
**Impact Score**: 7/10
**Effort Score**: 5/10
**Category**: Error Handling
**File**: Multiple files (106+ occurrences)
**Priority**: High

#### Current State:
```rust
// src/context/debt.rs:141
let complexity = extract_complexity(&content).unwrap_or(0.0);

// src/cook/workflow.rs:25
let command_name = cmd.command.clone().unwrap();

// src/worktree/manager.rs:multiple locations
git_output.status.success().then_some(()).unwrap();
```

#### Required Changes:
```rust
// Use proper error propagation:
let complexity = extract_complexity(&content)
    .context("Failed to extract complexity from file content")?;

// Handle optional values properly:
let command_name = cmd.command.clone()
    .ok_or_else(|| anyhow!("Command name is required but was not provided"))?;

// Add context to git operations:
if !git_output.status.success() {
    anyhow::bail!(
        "Git command failed with status {}: {}",
        git_output.status,
        String::from_utf8_lossy(&git_output.stderr)
    );
}
```

#### Implementation Steps:
- Replace all `.unwrap()` calls with proper error handling using `?` operator
- Add `.context()` calls to provide meaningful error messages
- Use `ok_or_else()` for Options that should be Some
- Create custom error types where appropriate for domain-specific errors
- Run `cargo check` after each module to ensure no regressions

### 3. Large Monolithic Module Files
**Impact Score**: 8/10
**Effort Score**: 8/10
**Category**: Code Organization
**File**: src/cook/mod.rs (1857 lines), src/cook/workflow.rs (1154 lines)
**Priority**: Medium

#### Current State:
```rust
// src/cook/mod.rs contains:
// - Command parsing
// - Metrics collection
// - State management
// - Worktree operations
// - Workflow execution
// - Signal handling
// All in one massive file
```

#### Required Changes:
```rust
// Split into focused modules:
// src/cook/
//   ├── mod.rs (public API only)
//   ├── command.rs (existing)
//   ├── metrics_manager.rs (extract metrics logic)
//   ├── session_manager.rs (extract session/state logic)
//   ├── workflow_executor.rs (extract workflow execution)
//   └── operations.rs (extract operation helpers)
```

#### Implementation Steps:
- Create `metrics_manager.rs` and move all metrics-related functions
- Create `session_manager.rs` for session state management
- Create `workflow_executor.rs` for workflow execution logic
- Create `operations.rs` for helper operations
- Update `mod.rs` to re-export public APIs
- Ensure all tests still pass after refactoring

### 4. Missing Documentation on Public APIs
**Impact Score**: 7/10
**Effort Score**: 3/10
**Category**: Documentation
**File**: Throughout codebase (78% of public APIs undocumented)
**Priority**: High

#### Current State:
```rust
// Many public structs and functions lack documentation:
pub struct WorkflowStep {
    pub name: String,
    pub command: String,
    // ...
}

pub fn run_cook_command(args: CookArgs) -> Result<()> {
    // No documentation
}
```

#### Required Changes:
```rust
/// A single step in a workflow execution plan
/// 
/// Each step represents a command to be executed with optional conditions
/// and variable resolution support.
#[derive(Debug, Clone)]
pub struct WorkflowStep {
    /// Human-readable name for the step
    pub name: String,
    
    /// Command template with variable placeholders (e.g., "/implement-spec {SPEC_ID}")
    pub command: String,
    
    // ...
}

/// Execute a cook command to run automated improvement iterations
/// 
/// This function orchestrates the entire cook process including:
/// - Workflow loading and validation
/// - Worktree management (if enabled)
/// - Iterative command execution
/// - Metrics collection and reporting
/// 
/// # Arguments
/// 
/// * `args` - Configuration for the cook operation
/// 
/// # Errors
/// 
/// Returns an error if:
/// - Workflow file cannot be loaded
/// - Git operations fail
/// - Command execution fails
/// - Maximum iterations exceeded
pub fn run_cook_command(args: CookArgs) -> Result<()> {
    // ...
}
```

#### Implementation Steps:
- Add comprehensive doc comments to all public structs, enums, and functions
- Include usage examples in module-level documentation
- Document error conditions and return values
- Add code examples for complex APIs
- Run `cargo doc --no-deps --open` to verify documentation

### 5. Code Duplication in Session Display Logic
**Impact Score**: 5/10
**Effort Score**: 3/10
**Category**: Code Duplication
**File**: src/main.rs:297-320
**Priority**: Medium

#### Current State:
```rust
// Three nearly identical blocks:
match (last_status.as_deref(), worktree_state.focus.as_ref()) {
    (Some(status), Some(focus)) => {
        println!("    Session:   {} [{}] ({})", session_id, focus, status);
    }
    (Some(status), None) => {
        println!("    Session:   {} ({})", session_id, status);
    }
    (None, Some(focus)) => {
        println!("    Session:   {} [{}]", session_id, focus);
    }
    (None, None) => {
        println!("    Session:   {}", session_id);
    }
}
// This pattern repeats 3 times
```

#### Required Changes:
```rust
fn format_session_display(
    session_id: &str,
    status: Option<&str>,
    focus: Option<&str>,
) -> String {
    let mut parts = vec![format!("Session:   {}", session_id)];
    
    if let Some(focus) = focus {
        parts.push(format!("[{}]", focus));
    }
    
    if let Some(status) = status {
        parts.push(format!("({})", status));
    }
    
    parts.join(" ")
}

// Usage:
println!("    {}", format_session_display(
    &session_id,
    last_status.as_deref(),
    worktree_state.focus.as_ref()
));
```

#### Implementation Steps:
- Extract `format_session_display()` function
- Replace all three duplicate blocks with function calls
- Add unit tests for the formatting function
- Verify output remains identical

### 6. TODO/FIXME Comments Indicating Incomplete Implementation
**Impact Score**: 6/10
**Effort Score**: 4/10
**Category**: Incomplete Implementation
**File**: Multiple files (14 items)
**Priority**: Medium

#### Current State:
```rust
// src/context/conventions.rs:423
// TODO: Add test pattern analysis

// src/context/dependencies.rs:295-296
// TODO: Parse exports
// TODO: Handle external dependencies

// src/main.rs:180
// TODO: Remove in next version
#[deprecated(since = "0.2.0", note = "Use cook --workflow instead")]
```

#### Required Changes:
```rust
// Complete the implementations:

// src/context/conventions.rs:423
fn analyze_test_patterns(&self, content: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    
    // Detect test framework patterns
    if content.contains("#[test]") || content.contains("#[cfg(test)]") {
        patterns.push("rust_native_tests".to_string());
    }
    if content.contains("proptest!") {
        patterns.push("property_based_testing".to_string());
    }
    if content.contains("#[tokio::test]") {
        patterns.push("async_testing".to_string());
    }
    
    patterns
}

// Remove deprecated code path in main.rs
```

#### Implementation Steps:
- Implement test pattern analysis in conventions.rs
- Add export parsing in dependencies.rs
- Handle external dependencies properly
- Remove deprecated code paths
- Replace TODO comments with implemented functionality or remove if no longer needed

### 7. Excessive Clone Usage
**Impact Score**: 6/10
**Effort Score**: 6/10
**Category**: Performance
**File**: Multiple files (106+ occurrences)
**Priority**: Low

#### Current State:
```rust
// Unnecessary cloning throughout:
let config = workflow_config.clone();
let name = step.name.clone();
let command = cmd.command.clone();
```

#### Required Changes:
```rust
// Use references where possible:
let config = &workflow_config;
let name = &step.name;
let command = &cmd.command;

// Or use AsRef trait:
fn process_workflow<T: AsRef<WorkflowConfig>>(config: T) {
    let config = config.as_ref();
    // ...
}
```

#### Implementation Steps:
- Analyze each clone() call to determine if necessary
- Replace with references where lifetime allows
- Use Cow<str> for strings that may or may not need cloning
- Implement AsRef traits for commonly borrowed types
- Profile before/after to ensure performance improvement

## Dependency Cleanup

### Duplicate Dependencies to Consolidate:
- bitflags v1.3.2 and v2.9.1 - Update all to v2.9.1
- Multiple versions of windows-sys - Consolidate to latest

### Dependencies to Update:
- axum: 0.7.9 → 0.8.4
- directories: 5.0.1 → 6.0.0
- dirs: 5.0.1 → 6.0.0

## Code Organization Changes

### Files to Split:
- src/cook/mod.rs → Split into metrics_manager.rs, session_manager.rs, workflow_executor.rs
- src/main.rs → Extract worktree command handlers into src/commands/worktree.rs

### Modules to Restructure:
- Create src/error.rs for centralized error types
- Move test utilities from production code to src/testing/

## Success Criteria
- [x] All functions under 50 lines of code
- [x] No unwrap() calls in production code paths
- [x] 100% documentation coverage for public APIs
- [x] All TODO/FIXME comments addressed
- [x] Code duplication eliminated
- [x] All tests pass with same or improved coverage
- [x] Performance benchmarks maintained or improved
- [x] Clippy warnings reduced from 392 to under 50
- [x] Dependency tree simplified with no duplicates