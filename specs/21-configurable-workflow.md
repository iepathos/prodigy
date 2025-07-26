# MMM Configurable Workflow Specification

## Overview

Currently, MMM (Memento Mori Manager) has a hardcoded workflow sequence:
1. `mmm-code-review` - Generate improvement spec
2. `mmm-implement-spec` - Implement the improvements
3. `mmm-lint` - Apply linting/formatting

This specification proposes making the workflow configurable while maintaining the simplicity and zero-configuration philosophy of MMM.

## Goals

1. **Flexibility**: Allow users to customize the improvement workflow
2. **Extensibility**: Enable adding custom commands to the workflow
3. **Simplicity**: Maintain zero-configuration default behavior
4. **Clarity**: Make workflow steps visible and understandable

## Design

### Configuration File

Introduce an optional `.mmm.toml` configuration file that users can place in their project root:

```toml
# .mmm.toml
[workflow]
# Default workflow if not specified
steps = [
    { command = "mmm-code-review", name = "Code Review" },
    { command = "mmm-implement-spec", name = "Implementation", args = ["${SPEC_ID}"] },
    { command = "mmm-lint", name = "Linting" }
]

# Optional: Maximum iterations (default: 10)
max_iterations = 10

# Optional: Continue on step failure (default: false)
continue_on_error = false

# Optional: Custom extractor for dynamic values
[workflow.extractors]
SPEC_ID = { from = "git", pattern = "iteration-([^\\s]+)" }
```

### Alternative Workflow Examples

#### Security-Focused Workflow
```toml
[workflow]
steps = [
    { command = "mmm-security-scan", name = "Security Scan" },
    { command = "mmm-fix-vulnerabilities", name = "Fix Vulnerabilities", args = ["${SCAN_RESULTS}"] },
    { command = "mmm-lint", name = "Linting" },
    { command = "mmm-security-verify", name = "Verify Fixes" }
]
```

#### Test-Driven Workflow
```toml
[workflow]
steps = [
    { command = "mmm-coverage", name = "Coverage Report" },
    { command = "mmm-add-tests", name = "Add Missing Tests", args = ["${COVERAGE_GAPS}"] },
    { command = "mmm-code-review", name = "Code Review" },
    { command = "mmm-implement-spec", name = "Implementation", args = ["${SPEC_ID}"] }
]
```

#### Documentation Workflow
```toml
[workflow]
steps = [
    { command = "mmm-doc-check", name = "Documentation Check" },
    { command = "mmm-generate-docs", name = "Generate Documentation" },
    { command = "mmm-lint", name = "Format Documentation" }
]
```

### Implementation Details

1. **Configuration Loading**:
   - Check for `.mmm.toml` in project root
   - If not found, use default workflow
   - Validate configuration structure

2. **Step Execution**:
   - Each step runs via `claude --dangerously-skip-permissions --print /<command> [args]`
   - Support for argument interpolation (e.g., `${SPEC_ID}`)
   - Pass `MMM_AUTOMATION=true` environment variable

3. **Data Extraction**:
   - Support extracting values from git commits, files, or command output
   - Use regex patterns for flexible extraction
   - Store extracted values for use in subsequent steps

4. **Error Handling**:
   - By default, stop on first error
   - Optional `continue_on_error` to proceed despite failures
   - Clear error reporting for each step

### Code Changes Required

1. **Add Configuration Module** (`src/config.rs`):
   ```rust
   pub struct WorkflowConfig {
       pub steps: Vec<WorkflowStep>,
       pub max_iterations: u32,
       pub continue_on_error: bool,
       pub extractors: HashMap<String, Extractor>,
   }
   
   pub struct WorkflowStep {
       pub command: String,
       pub name: String,
       pub args: Vec<String>,
   }
   ```

2. **Update `improve::run`**:
   - Load configuration from `.mmm.toml`
   - Execute steps according to configuration
   - Handle dynamic argument interpolation

3. **Add Extractor System**:
   - Git commit message extraction
   - File content extraction
   - Command output extraction

### Migration Path

1. **Phase 1**: Implement configuration system with default behavior unchanged
2. **Phase 2**: Add example configurations in documentation
3. **Phase 3**: Support for custom Claude commands in workflows

### Benefits

1. **Customization**: Teams can tailor MMM to their specific needs
2. **Reusability**: Share workflow configurations across projects
3. **Experimentation**: Easy to try different improvement strategies
4. **Integration**: Combine MMM with custom tooling

### Backwards Compatibility

- No configuration file = current behavior
- Existing CLI arguments continue to work
- Default workflow matches current implementation

### Future Extensions

1. **Conditional Steps**: Run steps based on conditions
2. **Parallel Execution**: Run independent steps concurrently
3. **Workflow Templates**: Pre-built workflows for common scenarios
4. **Plugin System**: Allow external commands in workflows