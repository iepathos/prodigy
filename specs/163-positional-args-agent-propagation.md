---
number: 163
title: Positional Arguments Propagation to Agent Contexts
category: compatibility
priority: medium
status: draft
dependencies: [120]
created: 2025-11-16
---

# Specification 163: Positional Arguments Propagation to Agent Contexts

**Category**: compatibility
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 120 (Environment Variables)

## Context

MapReduce workflows support passing positional arguments via `--args`, which are accessible in the setup phase using shell positional parameter syntax (`$1`, `$2`, etc.). However, these positional parameters are not available in map phase agent shells, even though they work correctly in the setup phase. This creates an inconsistent user experience and causes confusion.

The root cause is that positional parameters are shell-specific and don't export like environment variables. When agent worktrees execute commands in separate shell sessions, they don't inherit the positional parameters from the parent workflow context.

Currently, users must work around this by defining environment variables that reference positional args:
```yaml
env:
  BLOG_POST: "$1"
```

This workaround is not intuitive and the inconsistency between setup and map phases violates the principle of least surprise.

## Objective

Ensure positional arguments passed via `--args` are consistently available across all workflow phases (setup, map, reduce) by automatically exporting them as both positional parameters and named environment variables in agent contexts.

## Requirements

### Functional Requirements

1. **Automatic Environment Variable Export**
   - When positional arguments are provided via `--args`, automatically export them as environment variables
   - Use naming convention: `ARG_1`, `ARG_2`, `ARG_3`, etc.
   - Make these variables available in all workflow phases (setup, map, reduce)
   - Preserve original positional argument values exactly (no escaping or modification)

2. **Backward Compatibility**
   - Existing workflows using `env: MY_VAR: "$1"` must continue to work
   - Environment variable interpolation (`$1` → actual value) must happen before auto-export
   - No breaking changes to existing workflow syntax

3. **Consistent Behavior Across Phases**
   - Setup phase: Both `$1` and `${ARG_1}` work
   - Map phase: Both `$1` and `${ARG_1}` work in agent shells
   - Reduce phase: Both `$1` and `${ARG_1}` work

4. **Documentation and Guidance**
   - Document the automatic `ARG_N` variable export in workflow documentation
   - Recommend using `${ARG_N}` for clarity and portability
   - Update examples to show best practices

### Non-Functional Requirements

1. **Performance**: Variable export should not add measurable overhead to workflow execution
2. **Security**: Positional arguments should respect secret masking if they contain sensitive data
3. **Maintainability**: Implementation should be straightforward and not introduce complex edge cases
4. **Testability**: All behavior must be unit testable and integration testable

## Acceptance Criteria

- [ ] Positional arguments passed via `--args` are automatically exported as `ARG_1`, `ARG_2`, etc.
- [ ] `${ARG_N}` variables are available in setup phase shell commands
- [ ] `${ARG_N}` variables are available in map phase agent shell commands
- [ ] `${ARG_N}` variables are available in reduce phase shell commands
- [ ] Original positional parameter syntax (`$1`, `$2`) continues to work in setup phase
- [ ] Environment variables defined as `MY_VAR: "$1"` continue to be interpolated correctly
- [ ] Empty or missing positional arguments result in empty `ARG_N` variables (not errors)
- [ ] Documentation includes examples using `${ARG_N}` syntax
- [ ] Unit tests validate auto-export behavior
- [ ] Integration tests verify cross-phase consistency

## Technical Details

### Implementation Approach

**Option 1: Automatic Export in Environment Builder** (Recommended)
- Modify `src/workflow/environment.rs` to detect positional arguments
- After parsing workflow `env` block, inject `ARG_N` variables
- Export happens before any shell interpolation
- Advantages: Clean, centralized, easy to test

**Option 2: Shell Wrapper Script**
- Inject shell wrapper that sets `ARG_1=$1`, `ARG_2=$2`, etc.
- Execute in each agent worktree context
- Advantages: Closer to shell semantics
- Disadvantages: More complex, harder to test

**Recommended**: Option 1 for simplicity and testability.

### Architecture Changes

**Environment Variable Processing Flow**:
```
1. Parse workflow env block
2. Extract positional args from --args flag
3. Auto-inject ARG_1, ARG_2, ... ARG_N variables
4. Perform variable interpolation (including $1, $2, etc.)
5. Export all variables to execution context
```

**Code Changes**:
- `src/workflow/environment.rs`: Add `inject_positional_args()` function
- `src/orchestrator/mod.rs`: Pass positional args to environment builder
- `src/mapreduce/agent.rs`: Ensure agent contexts inherit all env vars

### Data Structures

No new data structures required. Extend existing `HashMap<String, String>` for environment variables.

### APIs and Interfaces

**New Function**:
```rust
/// Inject positional arguments as ARG_N environment variables
fn inject_positional_args(
    env: &mut HashMap<String, String>,
    args: &[String]
) {
    for (index, arg) in args.iter().enumerate() {
        let var_name = format!("ARG_{}", index + 1);
        env.insert(var_name, arg.clone());
    }
}
```

### Edge Cases

1. **No positional arguments**: No `ARG_N` variables are injected
2. **More args than expected**: All are exported (ARG_1, ARG_2, ..., ARG_N)
3. **Fewer args than expected**: Missing args result in missing `ARG_N` vars (consistent with `$N` behavior)
4. **Empty string arguments**: Exported as empty string `ARG_N=""`
5. **Arguments with special characters**: Preserved exactly as provided

## Dependencies

- **Prerequisites**: Spec 120 (Environment Variables) - uses existing env var infrastructure
- **Affected Components**:
  - `src/workflow/environment.rs` - environment variable processing
  - `src/orchestrator/mod.rs` - workflow orchestration
  - `src/mapreduce/agent.rs` - agent execution context
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

**Environment Variable Injection**:
```rust
#[test]
fn test_inject_positional_args() {
    let mut env = HashMap::new();
    let args = vec!["file.txt".to_string(), "output.json".to_string()];
    inject_positional_args(&mut env, &args);

    assert_eq!(env.get("ARG_1"), Some(&"file.txt".to_string()));
    assert_eq!(env.get("ARG_2"), Some(&"output.json".to_string()));
}

#[test]
fn test_empty_args() {
    let mut env = HashMap::new();
    let args: Vec<String> = vec![];
    inject_positional_args(&mut env, &args);

    assert!(env.is_empty());
}

#[test]
fn test_args_with_special_chars() {
    let mut env = HashMap::new();
    let args = vec!["path/with spaces/file.md".to_string()];
    inject_positional_args(&mut env, &args);

    assert_eq!(env.get("ARG_1"), Some(&"path/with spaces/file.md".to_string()));
}
```

### Integration Tests

**Cross-Phase Consistency**:
```yaml
# test-workflow.yml
name: test-positional-args
mode: mapreduce

setup:
  - shell: "test '${ARG_1}' = 'test-value'"
  - shell: "echo ${ARG_1} > setup-output.txt"

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - shell: "test '${ARG_1}' = 'test-value'"
    - shell: "echo Processing ${item.name} with ${ARG_1}"

reduce:
  - shell: "test '${ARG_1}' = 'test-value'"
```

Test execution:
```bash
prodigy run test-workflow.yml --args test-value
```

**Backward Compatibility**:
```yaml
# Test that existing env var syntax still works
env:
  MY_FILE: "$1"

setup:
  - shell: "test '${MY_FILE}' = 'test.txt'"
  - shell: "test '${ARG_1}' = 'test.txt'"  # Auto-injected
```

### Performance Tests

- Measure overhead of `inject_positional_args()` for 0, 1, 10, 100 args
- Verify no measurable impact on workflow execution time (<1ms)

### User Acceptance

- Verify cross-post-blog workflow from bug report works with `${ARG_1}`
- Confirm users can use either `$1` (in setup) or `${ARG_1}` (in all phases)
- Validate error messages are clear if args are missing

## Documentation Requirements

### Code Documentation

- Add inline comments explaining auto-injection logic
- Document the `ARG_N` naming convention in function docs
- Include examples in module-level documentation

### User Documentation

**Update CLAUDE.md** (MapReduce Workflow Syntax section):
```markdown
### Positional Arguments

Positional arguments passed via `--args` are automatically available as environment variables:

```bash
prodigy run workflow.yml --args file.txt output.json
```

In your workflow, access arguments using:
- `${ARG_1}` - First argument (recommended for all phases)
- `${ARG_2}` - Second argument
- `$1`, `$2` - Shell-style (works in setup phase only)

**Example:**
```yaml
setup:
  - shell: "test -f ${ARG_1}"  # Works
  - shell: "test -f $1"         # Also works in setup

map:
  agent_template:
    - shell: "process ${ARG_1}"  # Works
    - shell: "process $1"         # Does NOT work in agents
```

**Best Practice**: Always use `${ARG_N}` for consistency across all workflow phases.
```

**Update Workflow Examples**:
- Add example showing `${ARG_1}` usage
- Demonstrate cross-phase consistency
- Show best practices

### Architecture Updates

Add to ARCHITECTURE.md:
```markdown
## Environment Variable Processing

Positional arguments from `--args` are automatically exported as `ARG_N` environment variables:
1. Workflow parser extracts --args values
2. Environment builder injects ARG_1, ARG_2, etc.
3. User-defined env variables are interpolated (including $1 references)
4. All variables exported to execution contexts (setup, map, reduce)
```

## Implementation Notes

### Order of Operations

Critical: Positional arg auto-export must happen BEFORE user env var interpolation:

```yaml
# User defines:
env:
  FILE_PATH: "$1"

# Prodigy internally creates:
env:
  ARG_1: "actual-value"        # Injected first
  FILE_PATH: "actual-value"    # Interpolated using $1
```

### Error Handling

- Missing positional args: No error, corresponding `ARG_N` simply doesn't exist
- Invalid arg syntax: Shell handles it (same as current behavior)
- Conflict with user-defined `ARG_N`: User definition wins (allow override)

### Best Practices for Users

1. **Use `${ARG_N}` for portability**: Works in all phases
2. **Use `$1` only in setup**: If you're certain command only runs in setup
3. **Define semantic env vars**: `FILE_PATH: "${ARG_1}"` is clearer than bare `${ARG_1}`
4. **Document expected args**: Add comments in workflow files

## Migration and Compatibility

### Breaking Changes

None. This is a purely additive feature.

### Migration Requirements

None. Existing workflows continue to work unchanged.

### Compatibility Considerations

- **Existing workflows**: No changes required
- **User-defined `ARG_N` variables**: User definition takes precedence over auto-injection
- **Shell compatibility**: Works with bash, zsh, sh, fish (uses env vars, not shell builtins)

## Future Enhancements

Consider in future specifications:
1. **Named arguments**: `--args name=value` → `${name}`
2. **Argument validation**: Type checking, required vs optional
3. **Argument documentation**: Built-in help for workflow arguments
4. **IDE support**: Autocomplete for `${ARG_N}` variables

## References

- Bug Report: prodigy-bug-positional-args-scope.md
- Related Spec: 120 (Environment Variables)
- Cross-Post Blog Workflow: workflows/cross-post-blog.yml
