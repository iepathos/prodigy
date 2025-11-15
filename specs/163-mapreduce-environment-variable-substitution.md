---
number: 163
title: MapReduce Environment Variable Substitution in Agent Commands
category: foundation
priority: critical
status: draft
dependencies: [120]
created: 2025-11-15
---

# Specification 163: MapReduce Environment Variable Substitution in Agent Commands

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 120 (MapReduce Environment Variables)

## Context

A critical bug has been discovered in the MapReduce workflow engine where environment variables defined in a workflow's `env:` section are not being substituted when passed to agent Claude commands in the map phase. Variables are passed as literal unexpanded strings (e.g., `${BLOG_POST}`) instead of their actual values (e.g., `content/blog/mkdocs-drift-automation.md`), causing catastrophic workflow failures with a 67% failure rate in production use.

### Problem Analysis

The bug manifests in the following way:

**Expected behavior:**
```
Executing Claude command: /cross-post-adapt --blog-post content/blog/mkdocs-drift-automation.md --site-url https://entropicdrift.com
```

**Actual behavior:**
```
Executing Claude command: /cross-post-adapt --blog-post ${BLOG_POST} --site-url ${SITE_URL}
```

### Root Cause

The variable substitution failure occurs in `src/cook/execution/mapreduce/map_phase.rs:160` in the `create_agent_context()` function. This function only extracts variables from the work item itself but does not include workflow-level environment variables from the `env:` section of the workflow configuration.

**Current implementation:**
```rust
fn create_agent_context(agent_id: &str, item: &Value, index: usize) -> AgentContext {
    let mut context = AgentContext::new(...);

    // Add item variables
    context.variables = extract_item_variables(item);  // ❌ Only item fields
    context.variables.insert("ITEM_INDEX".to_string(), index.to_string());

    context
}
```

**Missing:**
- Workflow-level `env:` variables (e.g., `BLOG_POST`, `SITE_URL`, `SITE_NAME`)
- Positional arguments resolved from `$1`, `$2`, etc.
- Profile-specific environment variables
- Secret variables (with proper masking)

### Impact

This bug has caused significant production failures:

| Failure Type | Frequency | Description |
|--------------|-----------|-------------|
| Total execution failure | 33% (2/6) | Agents cannot resolve variables, produce no commits |
| Wrong input file processing | 33% (2/6) | Agents infer wrong files from context, modify incorrect data |
| Accidental success | 33% (2/6) | Agents somehow correctly infer intended values (unreliable) |

**Overall success rate: 33%**
**Failure rate: 67%**

This makes parameterized MapReduce workflows unreliable and defeats their fundamental value proposition.

## Objective

Fix the environment variable substitution bug in MapReduce agent commands to ensure all workflow-level environment variables are properly expanded before being passed to agent commands in all phases (setup, map, reduce).

## Requirements

### Functional Requirements

1. **Workflow Environment Variables Must Be Available to All Phases:**
   - Setup phase commands must have access to workflow `env:` variables
   - Map phase agent commands must have access to workflow `env:` variables
   - Reduce phase commands must have access to workflow `env:` variables
   - Merge phase commands must have access to workflow `env:` variables

2. **Variable Resolution Order:**
   - Positional arguments (`$1`, `$2`, etc.) are resolved from `--args` flag
   - Environment variables from `env:` section are resolved
   - Profile-specific variables are applied (if `--profile` is specified)
   - Secret variables are included (with masking)
   - Variables are substituted into command templates before execution

3. **Variable Scope and Precedence:**
   - Item-specific variables (e.g., `${item.name}`) have highest precedence
   - Workflow environment variables (e.g., `${BLOG_POST}`) have medium precedence
   - System environment variables have lowest precedence
   - Agent-local variables (e.g., captured outputs) override workflow variables

4. **Variable Interpolation:**
   - All variable references (`${VAR}` and `$VAR`) are expanded before command execution
   - Nested variable references are resolved recursively
   - Missing required variables cause clear error messages
   - Optional variables default to empty string or configured default

### Non-Functional Requirements

1. **Performance:**
   - Variable resolution must not significantly impact agent startup time
   - Variable context building should be done once per agent, not per command
   - Large environment variable sets (>100 variables) should not cause memory issues

2. **Security:**
   - Secret variables must remain masked in all logs
   - Variable values must not leak through error messages
   - Environment variables must not be accessible across agent boundaries

3. **Backward Compatibility:**
   - Existing workflows without `env:` section continue to work
   - Item-only variable workflows are unaffected
   - Setup and reduce phases that already work correctly remain functional

4. **Observability:**
   - Log when variables are resolved (at debug level)
   - Log when variable substitution fails with clear error message
   - Include variable context in agent execution events

## Acceptance Criteria

- [ ] **AC1:** Workflow environment variables are included in agent context creation
  - `create_agent_context()` accepts workflow env variables as parameter
  - Workflow env variables are merged into `AgentContext.variables`
  - Positional arguments (`$1`, `$2`) are resolved and included

- [ ] **AC2:** Variable substitution works in all MapReduce phases
  - Setup phase: Environment variables are expanded in shell and Claude commands
  - Map phase: Agent commands receive fully expanded variable values
  - Reduce phase: Environment and map result variables are both available
  - Merge phase: Merge-specific and workflow variables are available

- [ ] **AC3:** Variable precedence is correctly implemented
  - Item variables override workflow environment variables
  - Workflow env variables override system environment variables
  - Agent-local variables (captured outputs) override all others

- [ ] **AC4:** Secret masking continues to work
  - Variables marked as `secret: true` are masked in logs
  - Secret values are not included in error messages
  - Secret variables are included in agent context with masking metadata

- [ ] **AC5:** Error handling for missing variables
  - Missing required variables produce clear error messages
  - Error message includes variable name and where it was referenced
  - Agent fails gracefully with helpful diagnostic information

- [ ] **AC6:** All tests pass
  - Existing tests for variable interpolation continue to pass
  - New tests verify workflow env variables are available to agents
  - Integration test reproduces the bug report scenario and verifies fix

- [ ] **AC7:** Bug report scenario is resolved
  - Test workflow with `env:` section and positional arguments
  - Verify all 6 agents create correct output files
  - Verify 100% success rate (not 33%)
  - Verify no agents process wrong input files

## Technical Details

### Implementation Approach

The fix requires changes to several components:

#### 1. Update `create_agent_context()` Signature

**File:** `src/cook/execution/mapreduce/map_phase.rs:160`

Change the function signature to accept workflow environment variables:

```rust
fn create_agent_context(
    agent_id: &str,
    item: &Value,
    index: usize,
    workflow_env: &HashMap<String, String>,  // ← Add this parameter
) -> AgentContext {
    let mut context = AgentContext::new(...);

    // First, add workflow environment variables
    context.variables = workflow_env.clone();

    // Then, add item variables (these override workflow env)
    let item_vars = extract_item_variables(item);
    context.variables.extend(item_vars);

    // Finally, add agent-specific variables
    context.variables.insert("ITEM_INDEX".to_string(), index.to_string());

    context
}
```

#### 2. Thread Workflow Environment Through Call Stack

**Files:**
- `src/cook/execution/mapreduce/coordination/executor.rs`
- `src/cook/execution/mapreduce/phases/coordinator.rs`
- Any other code that calls `create_agent_context()`

Ensure workflow environment variables are passed through:

```rust
// In MapReduceConfig or similar
pub struct MapReduceConfig {
    // ... existing fields ...
    pub env: Option<HashMap<String, String>>,
}

// When spawning agents
let workflow_env = resolve_workflow_env(&config)?;
let context = create_agent_context(agent_id, item, index, &workflow_env);
```

#### 3. Resolve Positional Arguments

**File:** `src/cook/execution/mapreduce/mod.rs` or similar workflow parsing

When parsing workflow, resolve `$1`, `$2`, etc. from `--args`:

```rust
fn resolve_positional_args(
    env: &HashMap<String, String>,
    args: &[String],
) -> Result<HashMap<String, String>> {
    let mut resolved = env.clone();

    for (key, value) in env {
        if value.starts_with('$') && value.len() > 1 {
            if let Ok(index) = value[1..].parse::<usize>() {
                if index > 0 && index <= args.len() {
                    resolved.insert(key.clone(), args[index - 1].clone());
                }
            }
        }
    }

    Ok(resolved)
}
```

#### 4. Update Setup Phase

**File:** `src/cook/execution/mapreduce/phases/setup.rs` (if needed)

Ensure setup phase also receives workflow env variables (may already work correctly based on bug report evidence).

#### 5. Update Reduce Phase Context

**File:** `src/cook/execution/mapreduce/reduce_phase.rs:88`

Ensure reduce phase context includes workflow env variables:

```rust
pub fn create_reduce_context(
    map_results: &[AgentResult],
    config: &ReducePhaseConfig,
    workflow_env: &HashMap<String, String>,  // ← Add this parameter
) -> AgentContext {
    let mut context = AgentContext::new(...);

    // Add workflow environment variables first
    context.variables = workflow_env.clone();

    // Add aggregate statistics (these override workflow env if conflicts)
    add_aggregate_statistics(&mut context, map_results);

    context
}
```

#### 6. Update Agent Command Executor

**File:** `src/cook/execution/mapreduce/agent_command_executor.rs:165`

Ensure agent context variables are used during interpolation:

```rust
async fn execute_all_steps(
    &self,
    steps: &[WorkflowStep],
    agent_context: &AgentContext,
    env: &ExecutionEnvironment,
    job_id: &str,
) -> MapReduceResult<Vec<StepResult>> {
    // ...

    // Build interpolation context from agent variables
    let mut vars_map = HashMap::new();

    // ✅ Include ALL agent context variables, not just empty item object
    for (key, value) in &agent_context.variables {
        vars_map.insert(key.clone(), serde_json::Value::String(value.clone()));
    }

    let interp_context = InterpolationContext {
        variables: vars_map,
        parent: None,
    };

    // Execute steps with full variable context
    // ...
}
```

### Architecture Changes

**Before (broken):**
```
Workflow YAML
  ├─ env: {BLOG_POST: "$1", SITE_URL: "..."}
  └─ map:
      └─ agent_template:
          └─ claude: "/cmd --file ${BLOG_POST}"

┌──────────────────┐
│ create_agent_    │
│ context()        │
│ ↓                │
│ item vars only   │  ❌ Missing workflow env
└──────────────────┘
          ↓
┌──────────────────┐
│ Agent executes:  │
│ /cmd --file      │
│   ${BLOG_POST}   │  ❌ Literal, unexpanded
└──────────────────┘
```

**After (fixed):**
```
Workflow YAML
  ├─ env: {BLOG_POST: "$1", SITE_URL: "..."}
  └─ map:
      └─ agent_template:
          └─ claude: "/cmd --file ${BLOG_POST}"

┌──────────────────────────┐
│ Resolve workflow env     │
│ $1 → "path/to/file.md"   │
│ ↓                        │
│ workflow_env = {         │
│   BLOG_POST: "path/..."] │
│   SITE_URL: "https://..." │
│ }                        │
└──────────────────────────┘
          ↓
┌──────────────────────────┐
│ create_agent_context(    │
│   agent_id,              │
│   item,                  │
│   index,                 │
│   workflow_env  ← ✅     │
│ )                        │
│ ↓                        │
│ context.variables = {    │
│   ...workflow_env,       │
│   ...item_vars,          │
│   ITEM_INDEX: "0"        │
│ }                        │
└──────────────────────────┘
          ↓
┌──────────────────────────┐
│ Interpolate command      │
│ ${BLOG_POST} → expanded  │
│ ↓                        │
│ Agent executes:          │
│ /cmd --file              │
│   path/to/file.md  ✅    │
└──────────────────────────┘
```

### Data Structures

**No new data structures needed.** The fix uses existing structures:

- `AgentContext.variables: HashMap<String, String>` - Already exists
- `MapReduceWorkflowConfig.env: Option<HashMap<String, String>>` - Already exists (Spec 120)
- `InterpolationContext` - Already handles variable expansion

### APIs and Interfaces

#### Modified Function Signatures

```rust
// map_phase.rs
fn create_agent_context(
    agent_id: &str,
    item: &Value,
    index: usize,
    workflow_env: &HashMap<String, String>,  // ← New parameter
) -> AgentContext

// reduce_phase.rs
pub fn create_reduce_context(
    map_results: &[AgentResult],
    config: &ReducePhaseConfig,
    workflow_env: &HashMap<String, String>,  // ← New parameter
) -> AgentContext
```

#### New Helper Function

```rust
// Environment variable resolution with positional args
pub fn resolve_workflow_environment(
    env: &HashMap<String, String>,
    args: &[String],
) -> Result<HashMap<String, String>>
```

## Dependencies

### Prerequisites

- **Spec 120**: MapReduce Environment Variables support must be implemented
  - Workflow parsing already supports `env:` section
  - Secret masking infrastructure exists
  - Profile support exists

### Affected Components

The following components will be modified:

1. **`src/cook/execution/mapreduce/map_phase.rs`**
   - `create_agent_context()` function signature
   - Callers of `create_agent_context()`

2. **`src/cook/execution/mapreduce/reduce_phase.rs`**
   - `create_reduce_context()` function signature
   - Context building logic

3. **`src/cook/execution/mapreduce/agent_command_executor.rs`**
   - `execute_all_steps()` to use full agent variable context
   - Variable interpolation context building

4. **`src/cook/execution/mapreduce/coordination/executor.rs`**
   - Workflow environment resolution
   - Passing env to agent context creation

5. **`src/config/mapreduce.rs`**
   - May need helper for resolving positional arguments

### External Dependencies

None. All required infrastructure already exists.

## Testing Strategy

### Unit Tests

1. **Test `create_agent_context()` with workflow env**
   ```rust
   #[test]
   fn test_create_agent_context_includes_workflow_env() {
       let workflow_env = HashMap::from([
           ("PROJECT_NAME".to_string(), "test-project".to_string()),
           ("VERSION".to_string(), "1.0.0".to_string()),
       ]);
       let item = json!({"name": "item1", "priority": 5});

       let context = create_agent_context("agent-1", &item, 0, &workflow_env);

       // Workflow env should be present
       assert_eq!(context.variables.get("PROJECT_NAME"), Some(&"test-project".to_string()));
       assert_eq!(context.variables.get("VERSION"), Some(&"1.0.0".to_string()));

       // Item vars should override
       assert_eq!(context.variables.get("item.name"), Some(&"item1".to_string()));
   }
   ```

2. **Test variable precedence**
   ```rust
   #[test]
   fn test_variable_precedence() {
       let workflow_env = HashMap::from([
           ("name".to_string(), "workflow-value".to_string()),
       ]);
       let item = json!({"name": "item-value"});

       let context = create_agent_context("agent-1", &item, 0, &workflow_env);

       // Item variable should win
       assert_eq!(context.variables.get("item.name"), Some(&"item-value".to_string()));
   }
   ```

3. **Test positional argument resolution**
   ```rust
   #[test]
   fn test_resolve_positional_arguments() {
       let env = HashMap::from([
           ("BLOG_POST".to_string(), "$1".to_string()),
           ("OUTPUT_DIR".to_string(), "$2".to_string()),
       ]);
       let args = vec!["content/blog/test.md".to_string(), "output".to_string()];

       let resolved = resolve_workflow_environment(&env, &args).unwrap();

       assert_eq!(resolved.get("BLOG_POST"), Some(&"content/blog/test.md".to_string()));
       assert_eq!(resolved.get("OUTPUT_DIR"), Some(&"output".to_string()));
   }
   ```

### Integration Tests

1. **Reproduce bug report scenario**
   ```rust
   #[tokio::test]
   async fn test_mapreduce_env_variable_substitution_bug_163() {
       // Create workflow with env variables and positional args
       let workflow = r#"
       name: test-env-substitution
       mode: mapreduce

       env:
         BLOG_POST: "$1"
         SITE_URL: "https://example.com"
         OUTPUT_DIR: "output"

       map:
         input: '[{"platform": "dev"}, {"platform": "prod"}]'
         json_path: "$[*]"
         agent_template:
           - claude: "echo Processing ${BLOG_POST} for ${item.platform} to ${OUTPUT_DIR}"
       "#;

       // Run with positional argument
       let result = run_workflow(workflow, &["content/blog/test.md"]).await.unwrap();

       // All agents should succeed (100% success rate, not 33%)
       assert_eq!(result.successful_agents, 2);
       assert_eq!(result.failed_agents, 0);

       // Verify commands received expanded values, not literals
       for agent in &result.agent_logs {
           assert!(!agent.command.contains("${BLOG_POST}"));
           assert!(agent.command.contains("content/blog/test.md"));
       }
   }
   ```

2. **Test all phases have env access**
   ```rust
   #[tokio::test]
   async fn test_all_phases_receive_workflow_env() {
       let workflow = r#"
       name: test-all-phases
       mode: mapreduce

       env:
         TEST_VAR: "test-value"

       setup:
         - shell: "echo Setup: ${TEST_VAR} > setup-output.txt"

       map:
         input: '[{"id": 1}]'
         json_path: "$[*]"
         agent_template:
           - claude: "echo Map: ${TEST_VAR}"

       reduce:
         - shell: "echo Reduce: ${TEST_VAR} > reduce-output.txt"
       "#;

       run_workflow(workflow, &[]).await.unwrap();

       // Verify all phases expanded variables correctly
       assert_file_contains("setup-output.txt", "Setup: test-value");
       assert_file_contains("reduce-output.txt", "Reduce: test-value");
   }
   ```

3. **Test secret masking still works**
   ```rust
   #[tokio::test]
   async fn test_secret_variables_remain_masked() {
       let workflow = r#"
       name: test-secrets
       mode: mapreduce

       env:
         PUBLIC_VAR: "public-value"

       secrets:
         API_KEY: "secret-key-12345"

       map:
         input: '[{"id": 1}]'
         json_path: "$[*]"
         agent_template:
           - claude: "echo API_KEY=${API_KEY}"
       "#;

       let result = run_workflow(workflow, &[]).await.unwrap();

       // Check that logs mask secret
       for log_entry in &result.execution_logs {
           assert!(!log_entry.contains("secret-key-12345"));
           assert!(log_entry.contains("***") || log_entry.contains("[REDACTED]"));
       }
   }
   ```

### Performance Tests

```rust
#[tokio::test]
async fn test_large_env_performance() {
    // Create workflow with 200 environment variables
    let mut env = HashMap::new();
    for i in 0..200 {
        env.insert(format!("VAR_{}", i), format!("value_{}", i));
    }

    let start = Instant::now();
    let context = create_agent_context("agent-1", &json!({}), 0, &env);
    let duration = start.elapsed();

    // Should complete in < 10ms even with 200 variables
    assert!(duration < Duration::from_millis(10));
    assert_eq!(context.variables.len(), 201); // 200 env + 1 ITEM_INDEX
}
```

### User Acceptance

Test the exact scenario from the bug report:

1. Create `workflows/test-cross-post-blog.yml` (simplified version of bug report workflow)
2. Run: `prodigy run workflows/test-cross-post-blog.yml --args content/blog/test-post.md`
3. Expected results:
   - All 6 agents create correct output files
   - No agents process wrong input files
   - No agents fail with unexpanded variables
   - 100% success rate

## Documentation Requirements

### Code Documentation

1. **Update function documentation:**
   - `create_agent_context()` - Document new `workflow_env` parameter
   - `create_reduce_context()` - Document new `workflow_env` parameter
   - `resolve_workflow_environment()` - Document positional arg resolution

2. **Add code comments:**
   - Variable precedence order in context building
   - Why workflow env is cloned before item vars are added
   - How positional arguments are resolved

### User Documentation

1. **Update CLAUDE.md:**
   - Add note in "Environment Variables (Spec 120)" section
   - Confirm that env variables work correctly in all phases
   - Add example showing workflow env in agent commands

2. **Add to troubleshooting section:**
   - How to debug variable substitution issues
   - How to verify variables are expanded correctly
   - Common mistakes (e.g., forgetting `env:` section)

### Architecture Documentation

1. **Update ARCHITECTURE.md (if exists):**
   - Document variable resolution flow
   - Document variable precedence rules
   - Add diagram of variable context building

## Implementation Notes

### Code Organization

- Keep pure functions pure - don't add I/O to context building
- Use builder pattern if context creation becomes complex
- Consider creating a `WorkflowEnvironment` struct to encapsulate resolution logic

### Error Handling

- Use `Result<HashMap<String, String>>` for resolution functions
- Provide clear error messages when positional args are missing
- Include variable name and reference location in error messages

### Backward Compatibility

- Default to empty HashMap if workflow has no `env:` section
- Ensure existing tests pass without modification
- Don't break workflows that don't use environment variables

### Performance Considerations

- Clone workflow env once per agent, not per command
- Use `HashMap::extend()` instead of individual inserts
- Consider lazy evaluation if env resolution is expensive

## Migration and Compatibility

### Breaking Changes

**None.** This is a bug fix that makes existing functionality work correctly.

### Migration Path

No migration needed. Existing workflows will immediately benefit from the fix.

### Compatibility Matrix

| Workflow Type | Before Fix | After Fix |
|---------------|------------|-----------|
| No `env:` section | Works | Works (no change) |
| `env:` with item vars only | Works | Works (no change) |
| `env:` with workflow vars in setup | Works | Works (no change) |
| `env:` with workflow vars in map | **Broken (67% fail)** | **Fixed (100% success)** ✅ |
| `env:` with workflow vars in reduce | May work | Works (guaranteed) |

## Rollout Plan

### Phase 1: Fix Core Issue (Critical)
- Implement variable passing to `create_agent_context()`
- Add unit tests for context building
- Verify setup/reduce phases still work

### Phase 2: Add Positional Argument Support
- Implement `resolve_workflow_environment()`
- Add tests for `$1`, `$2`, etc.
- Update documentation

### Phase 3: Integration Testing
- Create integration test reproducing bug report
- Verify 100% success rate
- Test with large env sets

### Phase 4: Release
- Merge to master
- Add changelog entry
- Close related bug reports

## Success Metrics

- **Zero occurrences** of literal `${VAR}` in agent execution logs when env variables are defined
- **100% success rate** for bug report test scenario (up from 33%)
- **All existing tests pass** without modification
- **No performance regression** in agent startup time
