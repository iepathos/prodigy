---
number: 135
title: Fix map.results Variable Interpolation in write_file Commands
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-10-19
---

# Specification 135: Fix map.results Variable Interpolation in write_file Commands

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The MapReduce reduce phase currently fails when workflows attempt to use `${map.results}` in write_file commands. This occurs because:

1. **E2BIG Error Prevention**: To avoid "Argument list too long" errors (E2BIG, error 7), `map.results` was intentionally excluded from the environment variables HashMap passed to reduce phase commands
2. **Variable Interpolation Gap**: The `execute_step_in_agent_worktree()` function uses the limited HashMap for ALL interpolation, including write_file commands
3. **Silent Failure**: When `${map.results}` is interpolated with non-strict mode, it remains as the literal string `"${map.results}"`, which then fails JSON validation with "Invalid JSON content"

### Error Flow

```
Reduce Phase Step:
  write_file:
    path: ".prodigy/map-results.json"
    content: "${map.results}"  # Variable reference
    format: json

↓ execute_step_in_agent_worktree() called with HashMap
↓ HashMap only contains: map.successful, map.failed, map.total
↓ map.results is MISSING (excluded to avoid E2BIG)
↓ Interpolation in non-strict mode leaves "${map.results}" as-is
↓ write_file receives content = "${map.results}" (literal string)
↓ serde_json::from_str("${map.results}") fails
↓ Error: "Invalid JSON content"
```

### Root Cause

The separation of concerns between environment variables (which have size limits) and variable interpolation (which should have access to all data) was not properly maintained. The `execute_step_in_agent_worktree()` function uses the same limited HashMap for both purposes.

## Objective

Enable write_file commands in the reduce phase to successfully interpolate `${map.results}` and other large variables, while maintaining E2BIG error prevention for shell and Claude commands.

## Requirements

### Functional Requirements

1. **write_file Variable Access**: write_file commands must have access to `map.results` for interpolation
2. **Environment Variable Limits**: Shell and Claude commands must continue to avoid E2BIG errors by excluding large variables from environment
3. **Backward Compatibility**: Existing workflows using scalar variables (map.successful, map.failed, map.total) must continue to work
4. **Error Messages**: Clear error messages when variables are unavailable for interpolation
5. **Format Validation**: JSON/YAML format validation must occur AFTER successful variable interpolation

### Non-Functional Requirements

1. **Performance**: Minimal performance impact from dual variable storage
2. **Memory Efficiency**: Avoid duplicating large data structures unnecessarily
3. **Maintainability**: Clear separation between environment variables and interpolation context
4. **Testability**: Behavior must be covered by unit and integration tests

## Acceptance Criteria

- [ ] write_file commands in reduce phase successfully interpolate `${map.results}` with full agent result data
- [ ] JSON format validation parses the interpolated content correctly (not the literal template string)
- [ ] Shell commands in reduce phase do NOT receive `map.results` as environment variable (E2BIG prevention maintained)
- [ ] Claude commands in reduce phase do NOT receive `map.results` as environment variable (E2BIG prevention maintained)
- [ ] Scalar variables (map.successful, map.failed, map.total) remain available to all command types
- [ ] Unit test validates write_file receives full interpolation context with map.results
- [ ] Unit test validates shell commands receive limited environment without map.results
- [ ] Integration test demonstrates successful write_file with ${map.results} in reduce phase
- [ ] Error messages clearly indicate when variables are missing vs. when JSON parsing fails
- [ ] Existing MapReduce workflows continue to work without modification

## Technical Details

### Implementation Approach

**1. Separate Variable Contexts**

Create two distinct variable sources in `execute_step_in_agent_worktree()`:

```rust
async fn execute_step_in_agent_worktree(
    worktree_path: &Path,
    step: &WorkflowStep,
    variables: &HashMap<String, String>,  // Limited: for env vars
    full_context: Option<&InterpolationContext>,  // Full: for interpolation
    // ... other params
) -> MapReduceResult<StepResult>
```

**2. Interpolation Context Priority**

Use a fallback strategy for variable resolution:
- If `full_context` is provided, use it for interpolation (write_file)
- Otherwise, build InterpolationContext from `variables` HashMap (shell/claude)

**3. Environment Variable Separation**

- **Shell commands**: Use `variables` HashMap → converted to env vars (limited, no map.results)
- **Claude commands**: Use `variables` HashMap → converted to env vars (limited, no map.results)
- **write_file commands**: Use `full_context` if provided → full interpolation access (includes map.results)

### Architecture Changes

**File**: `src/cook/execution/mapreduce/coordination/executor.rs`

**Changes to `execute_step_in_agent_worktree()`**:

```rust
// Before: Single HashMap for everything
async fn execute_step_in_agent_worktree(
    worktree_path: &Path,
    step: &WorkflowStep,
    variables: &HashMap<String, String>,
    ...
) -> MapReduceResult<StepResult>

// After: Separate limited vars and full context
async fn execute_step_in_agent_worktree(
    worktree_path: &Path,
    step: &WorkflowStep,
    variables: &HashMap<String, String>,      // Limited for env vars
    full_context: Option<&InterpolationContext>,  // Full for interpolation
    ...
) -> MapReduceResult<StepResult>
```

**Changes to reduce phase execution**:

```rust
async fn execute_reduce_phase(
    &self,
    env: &ExecutionEnvironment,
    reduce: &ReducePhase,
    map_results: &[AgentResult],
) -> MapReduceResult<()> {
    // Create LIMITED variables for env (scalar values only)
    let mut variables = HashMap::new();
    variables.insert("map.successful".to_string(), summary.successful.to_string());
    variables.insert("map.failed".to_string(), summary.failed.to_string());
    variables.insert("map.total".to_string(), summary.total.to_string());

    // Create FULL interpolation context (includes map.results)
    let full_context = build_reduce_interpolation_context(map_results, &summary)?;

    for step in &reduce.commands {
        let step_result = Self::execute_step_in_agent_worktree(
            &env.working_dir,
            step,
            &variables,              // Limited HashMap
            Some(&full_context),     // Full context for write_file
            ...
        ).await?;
    }
}
```

**Helper function for building full context**:

```rust
fn build_reduce_interpolation_context(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> MapReduceResult<InterpolationContext> {
    let mut context = InterpolationContext::new();

    // Add scalar summary
    context.set("map.successful", json!(summary.successful));
    context.set("map.failed", json!(summary.failed));
    context.set("map.total", json!(summary.total));

    // Add full results as JSON value
    let results_value = serde_json::to_value(map_results)
        .map_err(|e| MapReduceError::ProcessingError(
            format!("Failed to serialize map results: {}", e)
        ))?;
    context.set("map.results", results_value);

    Ok(context)
}
```

**Changes within `execute_step_in_agent_worktree()`**:

```rust
// Build interpolation context with priority fallback
let interp_context = if let Some(full_ctx) = full_context {
    // Use provided full context (for write_file)
    full_ctx.clone()
} else {
    // Build from limited variables HashMap (for shell/claude)
    let mut ctx = InterpolationContext::new();
    // ... existing HashMap → InterpolationContext conversion
    ctx
};

// Use interp_context for ALL interpolation
// (write_file, shell, claude all use same context, but source differs)
```

### Data Structures

**InterpolationContext Enhancement** (if needed):

No changes required to `InterpolationContext` structure itself. The existing structure already supports nested JSON values including arrays of objects.

### APIs and Interfaces

**Modified Function Signature**:

```rust
// src/cook/execution/mapreduce/coordination/executor.rs
async fn execute_step_in_agent_worktree(
    worktree_path: &Path,
    step: &WorkflowStep,
    variables: &HashMap<String, String>,           // Existing
    full_context: Option<&InterpolationContext>,   // NEW parameter
    _env: &ExecutionEnvironment,
    claude_executor: &Arc<dyn ClaudeExecutor>,
    subprocess: &Arc<SubprocessManager>,
) -> MapReduceResult<StepResult>
```

**Call Sites to Update**:

1. **Map phase agent execution** (line ~1013):
   ```rust
   Self::execute_step_in_agent_worktree(
       handle.worktree_path(),
       step,
       &variables,
       None,  // Map phase doesn't need full context
       ...
   )
   ```

2. **Reduce phase execution** (line ~1541):
   ```rust
   Self::execute_step_in_agent_worktree(
       &env.working_dir,
       step,
       &variables,
       Some(&full_context),  // Reduce phase provides full context
       ...
   )
   ```

## Dependencies

- **Prerequisites**: None (bug fix to existing functionality)
- **Affected Components**:
  - `src/cook/execution/mapreduce/coordination/executor.rs` - Primary implementation
  - `src/cook/execution/interpolation.rs` - No changes, uses existing functionality
  - `src/cook/workflow/executor/commands.rs` - No changes, write_file logic unchanged
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

**File**: `src/cook/execution/mapreduce/coordination/executor_tests.rs` (or new test file)

**Test 1: write_file Receives Full Interpolation Context**
```rust
#[tokio::test]
async fn test_write_file_has_map_results_in_reduce_phase() {
    // Setup: Create mock AgentResults with test data
    let map_results = vec![
        AgentResult::success("item_0".into(), Some("out0".into()), Duration::from_secs(10)),
        AgentResult::success("item_1".into(), Some("out1".into()), Duration::from_secs(15)),
    ];

    // Setup: Create write_file step using ${map.results}
    let step = WorkflowStep {
        write_file: Some(WriteFileConfig {
            path: "output.json".into(),
            content: "${map.results}".into(),
            format: WriteFileFormat::Json,
            ..Default::default()
        }),
        ..Default::default()
    };

    // Setup: Create limited variables (no map.results)
    let mut variables = HashMap::new();
    variables.insert("map.successful".into(), "2".into());

    // Setup: Create full context with map.results
    let mut full_context = InterpolationContext::new();
    let results_json = serde_json::to_value(&map_results).unwrap();
    full_context.set("map.results", results_json);

    // Execute with full context
    let result = execute_step_in_agent_worktree(
        temp_dir.path(),
        &step,
        &variables,
        Some(&full_context),  // Full context provided
        ...
    ).await;

    // Verify: Step succeeds
    assert!(result.is_ok());
    assert!(result.unwrap().success);

    // Verify: File contains valid JSON with agent results
    let content = fs::read_to_string(temp_dir.path().join("output.json")).unwrap();
    let parsed: Vec<AgentResult> = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].item_id, "item_0");
}
```

**Test 2: Shell Commands Do Not Receive map.results in Environment**
```rust
#[tokio::test]
async fn test_shell_command_no_map_results_in_env() {
    // Setup: Create shell step that would fail if map.results is in env
    let step = WorkflowStep {
        shell: Some("echo \"Map successful: ${map.successful}\"".into()),
        ..Default::default()
    };

    // Setup: Limited variables only
    let mut variables = HashMap::new();
    variables.insert("map.successful".into(), "5".into());

    // Execute WITHOUT full context (simulates map phase or env var usage)
    let result = execute_step_in_agent_worktree(
        temp_dir.path(),
        &step,
        &variables,
        None,  // No full context
        ...
    ).await;

    // Verify: Succeeds with limited variables
    assert!(result.is_ok());
    assert!(result.unwrap().stdout.contains("Map successful: 5"));

    // Verify: Environment size is reasonable (no E2BIG)
    // This is implicit - if test passes, env wasn't too large
}
```

**Test 3: write_file Without Full Context Falls Back to HashMap**
```rust
#[tokio::test]
async fn test_write_file_fallback_to_hashmap_variables() {
    // Setup: write_file using scalar variable
    let step = WorkflowStep {
        write_file: Some(WriteFileConfig {
            path: "summary.txt".into(),
            content: "Successful: ${map.successful}".into(),
            format: WriteFileFormat::Text,
            ..Default::default()
        }),
        ..Default::default()
    };

    // Setup: Limited variables
    let mut variables = HashMap::new();
    variables.insert("map.successful".into(), "10".into());

    // Execute without full context (map phase scenario)
    let result = execute_step_in_agent_worktree(
        temp_dir.path(),
        &step,
        &variables,
        None,  // No full context - should fall back to variables
        ...
    ).await;

    // Verify: Succeeds using HashMap variables
    assert!(result.is_ok());
    let content = fs::read_to_string(temp_dir.path().join("summary.txt")).unwrap();
    assert_eq!(content, "Successful: 10");
}
```

**Test 4: Error Message for Missing Variable**
```rust
#[tokio::test]
async fn test_write_file_missing_variable_error_message() {
    // Setup: write_file using undefined variable in strict mode
    let step = WorkflowStep {
        write_file: Some(WriteFileConfig {
            path: "output.json".into(),
            content: r#"{"result": "${undefined.variable}"}"#.into(),
            format: WriteFileFormat::Json,
            ..Default::default()
        }),
        ..Default::default()
    };

    let variables = HashMap::new();
    let full_context = InterpolationContext::new();

    // Execute with strict interpolation
    let result = execute_step_in_agent_worktree_strict(
        temp_dir.path(),
        &step,
        &variables,
        Some(&full_context),
        ...
    ).await;

    // Verify: Fails with clear variable error (NOT JSON parsing error)
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Variable interpolation failed"));
    assert!(err_msg.contains("undefined.variable"));
    assert!(!err_msg.contains("Invalid JSON"));  // Should NOT be JSON error
}
```

### Integration Tests

**File**: `tests/mapreduce_write_file_integration_test.rs`

**Test: Full Workflow with write_file in Reduce Phase**

```rust
#[tokio::test]
async fn test_mapreduce_write_file_map_results() {
    // Create a minimal MapReduce workflow with write_file in reduce
    let workflow_yaml = r#"
name: test-write-file-reduce
mode: mapreduce

setup:
  - shell: "echo '[{\"id\": \"item1\"}, {\"id\": \"item2\"}]' > items.json"

map:
  input: "items.json"
  json_path: "$.[]"
  agent_template:
    - shell: "echo 'Processed ${item.id}'"
  max_parallel: 2

reduce:
  - write_file:
      path: ".prodigy/map-results.json"
      content: "${map.results}"
      format: json
      create_dirs: true
  - shell: "cat .prodigy/map-results.json"
"#;

    // Execute workflow
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("workflow.yml"), workflow_yaml).unwrap();

    let result = run_workflow(
        temp_dir.path().join("workflow.yml"),
        &temp_dir.path(),
    ).await;

    // Verify: Workflow succeeds
    assert!(result.is_ok(), "Workflow should succeed: {:?}", result.err());

    // Verify: map-results.json exists and contains valid JSON
    let results_path = temp_dir.path().join(".prodigy/map-results.json");
    assert!(results_path.exists(), "Results file should exist");

    let content = fs::read_to_string(&results_path).unwrap();
    let parsed: Vec<AgentResult> = serde_json::from_str(&content)
        .expect("Results should be valid JSON array of AgentResults");

    // Verify: Contains expected agent results
    assert_eq!(parsed.len(), 2, "Should have 2 agent results");
    assert_eq!(parsed[0].item_id, "item_0");
    assert_eq!(parsed[1].item_id, "item_1");
    assert!(parsed[0].is_success());
    assert!(parsed[1].is_success());
}
```

### Performance Tests

**Test: Large map.results Does Not Cause E2BIG**

```rust
#[tokio::test]
async fn test_large_map_results_no_e2big_error() {
    // Create workflow with many map items (100+)
    // Each item produces substantial output
    // Verify:
    // 1. write_file succeeds with full results
    // 2. Shell commands succeed (no E2BIG from env vars)
    // 3. Total execution completes successfully
}
```

### User Acceptance

**Manual Test**: Run the failing debtmap workflow from the bug report
```bash
prodigy run workflows/debtmap-reduce.yml
```

**Expected Outcome**:
- Workflow completes successfully
- `.prodigy/map-results.json` is created with valid JSON
- File contains array of AgentResult objects with all map phase results
- Reduce phase commands execute without "Invalid JSON content" error

## Documentation Requirements

### Code Documentation

1. **Function Documentation**: Update docstring for `execute_step_in_agent_worktree()` to explain dual variable context:
   ```rust
   /// Execute a step in an agent's worktree with variable interpolation
   ///
   /// # Arguments
   ///
   /// * `variables` - Limited scalar variables for environment variable export.
   ///   Excludes large data like `map.results` to prevent E2BIG errors.
   /// * `full_context` - Optional full interpolation context including large
   ///   variables. Used for write_file commands to enable `${map.results}`.
   ///   If None, falls back to building context from `variables` HashMap.
   ///
   /// # Variable Context Strategy
   ///
   /// - **Shell/Claude commands**: Use `variables` HashMap → converted to env vars
   /// - **write_file commands**: Use `full_context` if provided for interpolation
   /// - **Fallback**: If no `full_context`, build from `variables` (map phase)
   ```

2. **Inline Comments**: Add comments explaining E2BIG prevention strategy:
   ```rust
   // Create LIMITED variables for environment (prevent E2BIG errors)
   // map.results excluded because it can be >1MB with many agents
   let mut variables = HashMap::new();

   // Create FULL interpolation context for write_file commands
   // Includes map.results since interpolation doesn't use env vars
   let full_context = build_reduce_interpolation_context(...)?;
   ```

### User Documentation

**Update**: `book/src/variables.md`

Add section explaining `${map.results}` usage in reduce phase:

```markdown
### Using map.results in Reduce Phase

The `${map.results}` variable contains the complete array of agent results from the map phase. Due to its potential size (>1MB with many agents), it has special handling:

**✅ Supported in write_file commands**:
```yaml
reduce:
  - write_file:
      path: ".prodigy/results.json"
      content: "${map.results}"
      format: json
```

**✅ Supported in Claude commands** (via file reference):
```yaml
reduce:
  - write_file:
      path: ".prodigy/results.json"
      content: "${map.results}"
      format: json
  - claude: "/analyze-results --file .prodigy/results.json"
```

**⚠️ Not recommended in shell command interpolation**:
```yaml
reduce:
  # DON'T: Too large for environment variables
  - shell: "echo '${map.results}'"  # May cause errors

  # DO: Use write_file first, then reference the file
  - write_file:
      path: ".prodigy/results.json"
      content: "${map.results}"
  - shell: "jq '.[] | .item_id' .prodigy/results.json"
```

**Why this matters**: Shell and Claude commands use environment variables, which have size limits (~1MB on macOS). The `${map.results}` variable can exceed this limit, causing "Argument list too long" errors. Use write_file to save results to a file, then reference the file path.
```

### Architecture Updates

No ARCHITECTURE.md updates needed - this is a bug fix maintaining existing design principles.

## Implementation Notes

### Key Considerations

1. **Non-Strict Interpolation**: The interpolation engine runs in non-strict mode by default, which means undefined variables are left as literal placeholders (e.g., `"${undefined}"`). This is why the bug manifests as "Invalid JSON content" rather than "Variable not found".

2. **Backward Compatibility**: The `full_context` parameter is `Option<&InterpolationContext>`, allowing existing call sites (map phase) to pass `None` and maintain current behavior.

3. **Memory Efficiency**: The `full_context` is built once per reduce phase execution and passed by reference to avoid cloning large data structures.

4. **Error Context**: When interpolation fails, the error should mention the variable name, not just "Invalid JSON". This helps users distinguish between variable interpolation errors and actual JSON syntax errors.

### Gotchas

1. **Interpolation vs Environment**: The fix separates variable *interpolation* (string substitution in command text) from *environment variable export* (process env). write_file only needs interpolation.

2. **SerDe Compatibility**: AgentResult must remain JSON-serializable. The test suite validates this with `serde_json::to_value(&agent_result)`.

3. **Test Data Size**: Integration tests should use realistic data sizes to catch potential E2BIG issues in shell/Claude commands.

## Migration and Compatibility

### Breaking Changes

None. This is a bug fix with no API changes visible to workflow authors.

### Workflow Compatibility

**Before** (fails):
```yaml
reduce:
  - write_file:
      path: "results.json"
      content: "${map.results}"
      format: json
```
Error: "Invalid JSON content"

**After** (succeeds):
```yaml
reduce:
  - write_file:
      path: "results.json"
      content: "${map.results}"
      format: json
```
File created with valid JSON containing all agent results.

### Deprecations

None.

### Migration Steps

No migration required. Existing workflows continue to work, and previously-failing workflows with `${map.results}` in write_file will now succeed.
