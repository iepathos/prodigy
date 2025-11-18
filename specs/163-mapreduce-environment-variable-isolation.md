---
number: 163
title: MapReduce Environment Variable Isolation
category: parallel
priority: critical
status: draft
dependencies: []
created: 2025-11-18
---

# Specification 163: MapReduce Environment Variable Isolation

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

MapReduce agents are processing incorrect input files due to environment variable contamination from previous workflow executions. When running a workflow with 6 parallel agents, **3 out of 6 agents processed files from previous workflow runs** instead of the file specified in the current invocation's positional argument (`$1`).

This is a **critical data integrity issue** that causes agents to silently produce incorrect outputs without any error indication.

### Evidence of the Bug

From a real workflow execution processing `content/blog/rethinking-code-quality-analysis.md`:

| Agent | Platform | Expected Post | Actual Post | Status |
|-------|----------|---------------|-------------|---------|
| agent_0 | devto | rethinking-code-quality-analysis | rethinking-code-quality-analysis | ✓ Correct |
| agent_1 | hashnode | rethinking-code-quality-analysis | **debtmap-vs-competitors** | ✗ **Wrong** |
| agent_2 | linkedin | rethinking-code-quality-analysis | **debtmap-vs-competitors** | ✗ **Wrong** |
| agent_3 | reddit | rethinking-code-quality-analysis | **prodigy-docs-automation** | ✗ **Wrong** |
| agent_4 | medium | rethinking-code-quality-analysis | rethinking-code-quality-analysis | ✓ Correct |
| agent_5 | substack | rethinking-code-quality-analysis | rethinking-code-quality-analysis | ✓ Correct |

**Result**: 50% failure rate with silent data corruption.

### Impact

- **Data Integrity**: Agents silently produce incorrect outputs by processing wrong input files
- **Silent Failure**: No errors or warnings - workflow reports success despite corruption
- **Reliability**: 50% failure rate makes MapReduce workflows unreliable for production use
- **Debugging Difficulty**: Without detailed logging, the issue is not visible until manual inspection

### Current Implementation Issues

From analysis of `src/cook/execution/mapreduce/map_phase.rs:167-198`:

```rust
fn create_agent_context(
    agent_id: &str,
    item: &Value,
    index: usize,
    workflow_env: &HashMap<String, String>,
) -> AgentContext {
    // ...
    // Start with workflow environment variables (lowest precedence)
    context.variables = workflow_env.clone();

    // Add item variables (these override workflow env)
    let item_vars = extract_item_variables(item);
    context.variables.extend(item_vars);

    // Add agent-specific variables (highest precedence)
    context.variables.insert("ITEM_INDEX".to_string(), index.to_string());

    context
}
```

**Problem**: The `workflow_env` parameter is being contaminated with values from previous workflow executions, causing agents to receive stale variable values.

## Objective

Ensure complete environment variable isolation between MapReduce agent executions and workflow runs, preventing variable contamination and guaranteeing data integrity.

## Requirements

### Functional Requirements

1. **Complete Isolation**: Each agent execution must have a completely isolated environment with no contamination from:
   - Previous agent executions in the same workflow run
   - Previous workflow runs with different input values
   - Parent process environment variables (unless explicitly inherited)

2. **Explicit Inheritance**: Only explicitly specified environment variables should be inherited from:
   - Workflow `env:` section
   - Positional arguments (`$1`, `$2`, etc.)
   - System environment variables marked for inheritance

3. **Variable Validation**: Before agent execution, validate that:
   - All required environment variables are set
   - Variable values match expected types/formats
   - File paths in variables exist and are accessible

4. **Debug Logging**: Log environment variable values at each stage:
   - Initial workflow environment setup
   - Per-agent environment creation
   - Variable interpolation during command execution

5. **Error Detection**: Fail fast with clear error messages when:
   - Required variables are missing
   - Variable values are invalid
   - File paths don't exist

### Non-Functional Requirements

1. **Performance**: Environment isolation should add minimal overhead (<5ms per agent)
2. **Reliability**: 100% success rate for variable passing (no contamination)
3. **Debuggability**: Clear audit trail of variable values throughout execution
4. **Backward Compatibility**: Existing workflows should work without modification

## Acceptance Criteria

- [ ] **Zero Contamination**: Running the same workflow multiple times with different inputs shows 0% contamination rate across 100 test runs
- [ ] **Parallel Isolation**: Running 10 agents in parallel with different variable values shows each agent receives correct values
- [ ] **Sequential Isolation**: Running workflows back-to-back with different inputs shows no cross-contamination
- [ ] **Variable Validation**: Invalid variable values (missing files, wrong formats) fail immediately with clear error messages
- [ ] **Debug Logging**: Running with `RUST_LOG=debug` shows complete variable trace for each agent
- [ ] **Regression Tests**: All existing MapReduce integration tests pass
- [ ] **Performance**: MapReduce benchmark shows <5ms overhead per agent for environment setup
- [ ] **Documentation**: Environment variable handling is documented in ARCHITECTURE.md

## Technical Details

### Implementation Approach

#### 1. Pure Function for Environment Creation

Create a pure function that builds agent environment from explicit inputs only:

```rust
/// Create isolated environment for agent execution
///
/// This is a pure function - output depends only on inputs, no global state.
fn create_isolated_agent_environment(
    workflow_env: &HashMap<String, String>,
    item: &Value,
    agent_index: usize,
) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Step 1: Add workflow environment (from workflow YAML env: section)
    // These are explicitly defined in the workflow
    for (key, value) in workflow_env {
        env.insert(key.clone(), value.clone());
    }

    // Step 2: Add item-specific variables (from JSON input)
    // These override workflow env if there are conflicts
    if let Some(item_obj) = item.as_object() {
        for (key, value) in item_obj {
            if let Some(s) = value.as_str() {
                env.insert(format!("item.{}", key), s.to_string());
            }
        }
    }

    // Step 3: Add agent metadata (highest precedence)
    env.insert("ITEM_INDEX".to_string(), agent_index.to_string());
    env.insert("item_id".to_string(), format!("agent-{}", agent_index));

    env
}
```

#### 2. Environment Validation

Add validation before agent execution:

```rust
/// Validate agent environment before execution
fn validate_agent_environment(
    env: &HashMap<String, String>,
    required_vars: &[String],
) -> Result<(), MapReduceError> {
    // Check required variables are present
    for var in required_vars {
        if !env.contains_key(var) {
            return Err(MapReduceError::MissingVariable {
                variable: var.clone(),
            });
        }
    }

    // Validate file paths if variable names suggest files
    for (key, value) in env {
        if key.ends_with("_FILE") || key.ends_with("_PATH") {
            if !std::path::Path::new(value).exists() {
                return Err(MapReduceError::InvalidVariableValue {
                    variable: key.clone(),
                    value: value.clone(),
                    reason: "File does not exist".to_string(),
                });
            }
        }
    }

    Ok(())
}
```

#### 3. Comprehensive Debug Logging

Add structured logging at each stage:

```rust
use tracing::{debug, info};

// In execute_with_state function
debug!(
    workflow_env = ?workflow_env,
    "Workflow environment before agent creation"
);

// In create_agent_context
let env = create_isolated_agent_environment(workflow_env, item, index);
debug!(
    agent_id = %agent_id,
    agent_env = ?env,
    "Created isolated environment for agent"
);

// Validate before execution
validate_agent_environment(&env, &required_vars)?;
info!(
    agent_id = %agent_id,
    "Environment validation passed"
);
```

#### 4. Integration Testing

Add comprehensive tests to verify isolation:

```rust
#[tokio::test]
async fn test_agent_environment_isolation_across_runs() {
    // Run 1: Process file A
    let workflow_env_1 = HashMap::from([
        ("BLOG_POST".to_string(), "content/blog/post-a.md".to_string()),
    ]);
    let results_1 = execute_mapreduce_workflow(workflow_env_1).await.unwrap();
    assert_all_agents_processed(&results_1, "post-a.md");

    // Run 2: Process file B (different input)
    let workflow_env_2 = HashMap::from([
        ("BLOG_POST".to_string(), "content/blog/post-b.md".to_string()),
    ]);
    let results_2 = execute_mapreduce_workflow(workflow_env_2).await.unwrap();

    // Assert: NO contamination - all agents must process post-b
    assert_all_agents_processed(&results_2, "post-b.md");
    assert_no_agent_processed(&results_2, "post-a.md");
}

#[tokio::test]
async fn test_parallel_agent_isolation() {
    // Create 6 agents with different BLOG_POST values
    let items = vec![
        json!({"name": "agent0", "file": "post-0.md"}),
        json!({"name": "agent1", "file": "post-1.md"}),
        json!({"name": "agent2", "file": "post-2.md"}),
        json!({"name": "agent3", "file": "post-3.md"}),
        json!({"name": "agent4", "file": "post-4.md"}),
        json!({"name": "agent5", "file": "post-5.md"}),
    ];

    let results = execute_map_phase(items, 3).await.unwrap();

    // Each agent should see its own file value
    for (i, result) in results.iter().enumerate() {
        assert_eq!(
            result.env_var("item.file"),
            Some(&format!("post-{}.md", i))
        );
    }
}
```

### Architecture Changes

#### Modified Files

1. **`src/cook/execution/mapreduce/map_phase.rs`**
   - Replace `create_agent_context` with pure function approach
   - Add environment validation before agent execution
   - Add comprehensive debug logging

2. **`src/cook/execution/mapreduce/agent_command_executor.rs`**
   - Ensure no environment contamination during command execution
   - Add logging of interpolated variable values

3. **`src/cook/execution/mapreduce/types.rs`**
   - Add new error types: `MissingVariable`, `InvalidVariableValue`

4. **`tests/mapreduce_env_integration_test.rs`**
   - Add comprehensive environment isolation tests
   - Add cross-run contamination tests
   - Add parallel agent isolation tests

#### New Error Types

```rust
pub enum MapReduceError {
    // ... existing errors ...

    MissingVariable {
        variable: String,
    },

    InvalidVariableValue {
        variable: String,
        value: String,
        reason: String,
    },
}
```

### Data Structures

No new data structures needed - using existing `HashMap<String, String>` for environment variables.

### APIs and Interfaces

No public API changes - all changes are internal to MapReduce execution.

## Dependencies

### Prerequisites

None - this is a bug fix for existing MapReduce functionality.

### Affected Components

- **MapReduce Executor** (`src/cook/execution/mapreduce/`)
- **Agent Execution** (`src/cook/execution/mapreduce/agent_command_executor.rs`)
- **Integration Tests** (`tests/mapreduce_env_integration_test.rs`)

### External Dependencies

No new external dependencies required.

## Testing Strategy

### Unit Tests

1. **Environment Creation Tests**
   - Test `create_isolated_agent_environment` with various inputs
   - Verify variable precedence (workflow < item < agent)
   - Test edge cases (empty env, conflicting vars)

2. **Validation Tests**
   - Test `validate_agent_environment` with valid/invalid inputs
   - Test file path validation
   - Test missing required variables

### Integration Tests

1. **Cross-Run Isolation**
   - Run workflow multiple times with different inputs
   - Verify no contamination between runs
   - Measure contamination rate (must be 0%)

2. **Parallel Agent Isolation**
   - Run multiple agents in parallel with different variable values
   - Verify each agent sees correct values
   - Test with max_parallel=1, 2, 5, 10

3. **Sequential Workflow Isolation**
   - Run workflows back-to-back
   - Verify second run doesn't see first run's variables

### Performance Tests

1. **Overhead Measurement**
   - Benchmark environment creation time
   - Target: <5ms per agent
   - Compare before/after performance

2. **Stress Test**
   - Run 100 workflows with 10 agents each
   - Measure contamination rate (must be 0%)
   - Verify memory doesn't leak

### Regression Tests

1. **Existing MapReduce Tests**
   - All existing tests in `tests/mapreduce_*.rs` must pass
   - No behavioral changes to working workflows

2. **Real Workflow Test**
   - Test with the actual cross-post-blog workflow from bug report
   - Verify all 6 agents process correct blog post

## Documentation Requirements

### Code Documentation

- [ ] Add comprehensive doc comments to `create_isolated_agent_environment`
- [ ] Document variable precedence rules
- [ ] Add examples of proper environment setup
- [ ] Document error types and when they occur

### User Documentation

- [ ] Update MapReduce workflow documentation
- [ ] Add troubleshooting section for environment issues
- [ ] Document best practices for variable passing
- [ ] Add example workflows with environment variables

### Architecture Updates

- [ ] Add "Environment Variable Isolation" section to ARCHITECTURE.md
- [ ] Document the pure function approach for environment creation
- [ ] Explain variable precedence rules
- [ ] Document debug logging strategy

## Implementation Notes

### Root Cause

The bug occurs because `workflow_env` is likely being reused or cached across workflow runs. The `create_agent_context` function clones the `workflow_env` but if the source is contaminated, the clone is also contaminated.

### Key Insights

1. **Pure Functions**: Using pure functions for environment creation eliminates hidden state
2. **Explicit Inputs**: Only using explicit inputs prevents contamination
3. **Validation**: Early validation catches issues before they cause silent failures
4. **Logging**: Comprehensive logging makes contamination visible

### Gotchas

1. **Workflow Parser**: Ensure workflow parser creates fresh `HashMap` for each run
2. **Thread Safety**: Verify no shared mutable state between agents
3. **Caching**: Check for any caching of environment variables
4. **Shell Environment**: Ensure shell commands don't inherit contaminated environment

### Best Practices

1. Always validate environment before execution
2. Log environment at key stages (debug level)
3. Use pure functions for environment manipulation
4. Never share environment `HashMap` between agents
5. Create fresh environment for each workflow run

## Migration and Compatibility

### Breaking Changes

None - this is a bug fix with no API changes.

### Migration Path

No migration required - existing workflows will work correctly after fix.

### Compatibility

- ✅ Backward compatible with existing workflows
- ✅ No configuration changes required
- ✅ No data migration needed
- ✅ Existing tests should pass

## Success Metrics

### Quantitative Metrics

- **Contamination Rate**: 0% (measured over 100 test runs)
- **Performance Overhead**: <5ms per agent
- **Test Coverage**: >95% for environment handling code
- **Failure Detection**: 100% of invalid environments caught by validation

### Qualitative Metrics

- All agents in parallel execution receive correct variable values
- Debug logs clearly show variable flow through system
- Error messages clearly identify variable issues
- No silent failures due to environment contamination

## Implementation Phases

### Phase 1: Core Fix (Days 1-2)
- Implement pure environment creation function
- Add environment validation
- Add comprehensive logging

### Phase 2: Testing (Days 3-4)
- Write unit tests for environment creation
- Write integration tests for isolation
- Run stress tests for contamination detection

### Phase 3: Documentation (Day 5)
- Update code documentation
- Update ARCHITECTURE.md
- Add user-facing documentation

### Phase 4: Validation (Day 6)
- Run full regression test suite
- Test with real workflows from bug report
- Performance benchmarking

## References

- **Bug Report**: `prodigy-bug-report-mapreduce-variable-contamination.md`
- **Related Code**: `src/cook/execution/mapreduce/map_phase.rs:167-198`
- **Related Test**: `tests/mapreduce_env_integration_test.rs`
