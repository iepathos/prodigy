---
number: 101
title: Git Context Variables for Workflows
category: foundation
priority: high
status: draft
dependencies: []
created: 2024-01-18
---

# Specification 101: Git Context Variables for Workflows

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, Prodigy workflows that need to track file changes must rely on shell commands to determine what files were created, modified, or deleted during workflow execution. This leads to complex, error-prone shell scripting within workflows, as seen in the spec.yml workflow that tries to extract spec numbers using shell commands like `ls -t specs/*.md | head -1 | sed 's/specs\\///' | sed 's/-.*//'`.

Prodigy already tracks git commits for each workflow step and has access to the git repository state. By exposing this information as workflow variables, we can significantly simplify workflows and make them more robust and maintainable.

## Objective

Implement automatic git change tracking for workflow steps and expose file change information as context variables that can be referenced within workflow commands, eliminating the need for manual git diff commands and complex shell scripting.

## Requirements

### Functional Requirements

1. **Automatic Change Detection**
   - Track all file changes (added, modified, deleted) for each workflow step
   - Calculate changes by comparing git state before and after step execution
   - Handle both staged and committed changes appropriately
   - Support tracking changes across multiple commits within a step

2. **Context Variable Exposure**
   - Expose file change information as interpolatable variables
   - Provide different granularities of change information
   - Support both current step and cumulative workflow changes
   - Enable filtering by file patterns or directories

3. **Variable Types**
   - `${step.files_added}` - Files added in current step
   - `${step.files_modified}` - Files modified in current step
   - `${step.files_deleted}` - Files deleted in current step
   - `${step.files_changed}` - All changed files in current step
   - `${workflow.files_added}` - Files added since workflow start
   - `${workflow.files_modified}` - Files modified since workflow start
   - `${workflow.files_deleted}` - Files deleted since workflow start
   - `${workflow.files_changed}` - All changed files since workflow start
   - `${step.commits}` - Commit SHAs created in current step
   - `${step.commit_count}` - Number of commits in current step
   - `${step.insertions}` - Lines inserted in current step
   - `${step.deletions}` - Lines deleted in current step

4. **Format Options**
   - Space-separated list for command arguments
   - Newline-separated for multiline contexts
   - JSON array for structured data needs
   - Pattern matching support (e.g., `${step.files_added:*.md}`)

5. **Performance Requirements**
   - Lazy evaluation - only calculate when variables are used
   - Cache results within step execution
   - Efficient git operations using libgit2 or git CLI

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing workflows must continue to function
   - New variables should not conflict with existing ones
   - Graceful handling when no git repository exists

2. **Error Handling**
   - Clear error messages when git operations fail
   - Fallback to empty values when appropriate
   - Log warnings for edge cases

3. **Documentation**
   - Update workflow documentation with new variables
   - Provide migration guide for existing workflows
   - Include examples of common use cases

## Acceptance Criteria

- [ ] File change tracking implemented for all workflow steps
- [ ] Context variables exposed and interpolatable in workflow commands
- [ ] All specified variable types available and functional
- [ ] Pattern filtering works for file variables
- [ ] Performance impact minimal (< 100ms per step)
- [ ] Existing workflows continue to function without modification
- [ ] Documentation updated with variable reference
- [ ] Integration tests cover all variable types
- [ ] spec.yml workflow refactored to use new variables
- [ ] implement.yml workflow updated to use new variables

## Technical Details

### Implementation Approach

1. **Git Integration Layer**
   - Extend existing git module with change detection functions
   - Use git2 crate for efficient repository operations
   - Implement caching layer for repeated queries

2. **Workflow Context Enhancement**
   - Extend WorkflowContext struct with git tracking fields
   - Add GitChangeTracker component to workflow executor
   - Implement lazy evaluation for variable resolution

3. **Variable Resolution**
   - Extend interpolation engine to recognize new variables
   - Add formatters for different output formats
   - Implement pattern matching for file filters

### Architecture Changes

1. **New Components**
   ```rust
   // src/cook/workflow/git_context.rs
   pub struct GitChangeTracker {
       repo: Repository,
       workflow_start_commit: String,
       step_changes: HashMap<String, StepChanges>,
   }

   pub struct StepChanges {
       files_added: Vec<String>,
       files_modified: Vec<String>,
       files_deleted: Vec<String>,
       commits: Vec<String>,
       insertions: usize,
       deletions: usize,
   }
   ```

2. **Context Extension**
   ```rust
   // Add to WorkflowContext
   pub struct WorkflowContext {
       // ... existing fields
       git_tracker: Option<GitChangeTracker>,
       current_step_changes: Option<StepChanges>,
   }
   ```

### Data Structures

```rust
// Variable formats
pub enum VariableFormat {
    SpaceSeparated,    // Default
    NewlineSeparated,  // For multiline
    JsonArray,         // For structured data
    Comma,             // For CSV-like output
}

// Pattern matching
pub struct FilePattern {
    pattern: String,
    matcher: glob::Pattern,
}
```

### APIs and Interfaces

```rust
impl GitChangeTracker {
    /// Get changes for current step
    pub fn get_step_changes(&self, step_id: &str) -> Result<StepChanges>;

    /// Get cumulative workflow changes
    pub fn get_workflow_changes(&self) -> Result<StepChanges>;

    /// Filter files by pattern
    pub fn filter_files(&self, files: &[String], pattern: &str) -> Vec<String>;
}

impl WorkflowContext {
    /// Resolve git context variable
    pub fn resolve_git_variable(&self, var: &str) -> Result<String>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - Workflow executor (src/cook/workflow/executor.rs)
  - Context interpolation (src/cook/workflow/context.rs)
  - Git module (src/git/mod.rs)
- **External Dependencies**:
  - git2 crate (already in use)
  - glob crate for pattern matching

## Testing Strategy

- **Unit Tests**:
  - Test GitChangeTracker with mock repositories
  - Verify variable resolution with different formats
  - Test pattern matching functionality

- **Integration Tests**:
  - Create test workflows using new variables
  - Verify correct tracking across multiple steps
  - Test with different git scenarios (merge commits, etc.)

- **Performance Tests**:
  - Measure overhead of change tracking
  - Test with large repositories
  - Verify caching effectiveness

- **User Acceptance**:
  - Refactor spec.yml to use new variables
  - Update example workflows
  - Gather feedback from workflow authors

## Documentation Requirements

- **Code Documentation**:
  - Document all new public APIs
  - Add examples to function documentation
  - Include performance considerations

- **User Documentation**:
  - Add "Git Context Variables" section to workflow guide
  - Provide migration examples from shell commands
  - Include common patterns and use cases

- **Architecture Updates**:
  - Update ARCHITECTURE.md with git tracking component
  - Document variable resolution flow
  - Add sequence diagrams for change tracking

## Implementation Notes

1. **Edge Cases**:
   - Handle workflows running outside git repositories
   - Deal with uncommitted changes appropriately
   - Support workflows that don't create commits

2. **Performance Optimizations**:
   - Cache git operations within step execution
   - Use git plumbing commands for efficiency
   - Batch file status queries

3. **Future Extensions**:
   - Add variables for file content (e.g., `${step.files_added_content}`)
   - Support for diff statistics per file
   - Integration with validation system for automatic file tracking

## Migration and Compatibility

1. **Backward Compatibility**:
   - All existing workflows continue to function
   - New variables are opt-in
   - No breaking changes to existing APIs

2. **Migration Path**:
   - Provide automated migration tool for common patterns
   - Document manual migration steps
   - Offer both old and new approaches during transition period

3. **Deprecation Strategy**:
   - Mark shell-based file tracking as deprecated
   - Provide warnings for complex shell commands that could use variables
   - Remove deprecated patterns in next major version

## Example Usage

### Before (Current Approach)
```yaml
# Step 2: Extract spec number from the most recent spec file
- shell: "ls -t specs/*.md | head -1 | sed 's/specs\\///' | sed 's/-.*//' | tr -d '\n'"
  capture_output: spec_number

# Step 3: Validate spec
- claude: "/prodigy-validate-spec ${shell.spec_number}"
```

### After (With Git Context Variables)
```yaml
# Step 2: Validate newly created specs
- claude: "/prodigy-validate-spec ${step.files_added:specs/*.md}"
```

### Complex Example
```yaml
# Validate all markdown files added or modified in this step
- claude: "/validate-docs ${step.files_added:*.md} ${step.files_modified:*.md}"

# Process each new test file
- foreach: "${step.files_added:*_test.rs}"
  commands:
    - shell: "cargo test --test ${item}"

# Report workflow summary
- shell: |
    echo "Workflow Summary:"
    echo "Files added: ${workflow.files_added}"
    echo "Files modified: ${workflow.files_modified}"
    echo "Total commits: ${workflow.commit_count}"
    echo "Lines changed: +${workflow.insertions} -${workflow.deletions}"
```

## Success Metrics

- Reduction in workflow complexity (measured by line count)
- Elimination of git-related shell commands in workflows
- Improved workflow reliability (fewer failures due to shell scripting errors)
- Positive user feedback on simplified workflow authoring
- Performance overhead < 100ms per workflow step