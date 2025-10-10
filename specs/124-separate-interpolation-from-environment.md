---
number: 124
title: Separate Variable Interpolation from Shell Environment Variables
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-10-09
---

# Specification 124: Separate Variable Interpolation from Shell Environment Variables

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, Prodigy passes all workflow variables (including map results, captured outputs, and internal state) as environment variables to shell commands. This creates several critical issues:

1. **Argument List Too Long Error (E2BIG)**: When MapReduce workflows process many items, the accumulated state creates massive environment variable lists that exceed OS limits (~256KB on macOS, ~2MB on Linux). This causes shell command execution to fail with "Argument list too long" errors.

2. **Security Concerns**: Passing all workflow state through environment variables exposes sensitive data unnecessarily. Even with secret masking in logs, the actual process environment contains unmasked values.

3. **Performance Overhead**: Serializing large data structures (like `map.results` JSON) to strings and passing them through the environment is inefficient and wasteful.

4. **Conceptual Confusion**: Mixing string interpolation variables (meant for templating command text) with process environment variables (meant for program configuration) creates an unclear design that's difficult to maintain and debug.

This design differs from established tools like Ansible, which keep variables separate from environment and only pass explicitly defined environment variables to commands.

## Objective

Redesign Prodigy's variable system to separate string interpolation (for templating command text) from environment variable passing (for process configuration), following Ansible's design pattern where workflow variables are used for interpolation only, and environment variables are explicitly defined.

## Requirements

### Functional Requirements

1. **FR1**: Workflow variables (map.results, item.*, captured outputs, etc.) must be used ONLY for string interpolation in command text
2. **FR2**: Only explicitly defined environment variables (from `env` blocks) should be passed to shell command processes
3. **FR3**: String interpolation must support all current variable syntax (${var}, $VAR, nested access)
4. **FR4**: Environment variables from `env` blocks must continue to work exactly as before
5. **FR5**: Secret masking must continue to work for both interpolated values and environment variables
6. **FR6**: Claude commands must receive only PRODIGY_* automation variables, not all workflow state
7. **FR7**: Implementation should be direct without backward compatibility concerns - breaking changes are acceptable

### Non-Functional Requirements

1. **NFR1**: Shell command execution must not fail with E2BIG regardless of MapReduce scale
2. **NFR2**: Performance overhead of variable handling must be minimized
3. **NFR3**: Security posture must improve by reducing environment variable exposure
4. **NFR4**: Code must be maintainable with clear separation of concerns

## Acceptance Criteria

- [ ] AC1: Shell commands receive only environment variables from `env` blocks, not workflow variables
- [ ] AC2: String interpolation in command text continues to work with all variable types
- [ ] AC3: MapReduce workflows with 100+ agents can execute shell commands without E2BIG errors
- [ ] AC4: Environment variable count passed to shell commands is bounded by `env` block size, not workflow state size
- [ ] AC5: All existing tests pass with necessary updates to reflect new behavior
- [ ] AC6: New integration test validates large MapReduce workflows (50+ agents) execute successfully
- [ ] AC7: Security audit shows reduced environment variable exposure
- [ ] AC8: Performance benchmarks show no regression in variable interpolation speed
- [ ] AC9: Documentation clearly explains the distinction between interpolation and environment
- [ ] AC10: Claude commands receive only PRODIGY_AUTOMATION, PRODIGY_WORKTREE, and PRODIGY_CLAUDE_STREAMING variables

## Technical Details

### Current Implementation Issues

**Location**: `src/cook/execution/mapreduce/utils.rs:107-129`

The `build_agent_context_variables()` function converts ALL workflow variables into environment variables:

```rust
pub fn build_agent_context_variables(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> Result<HashMap<String, String>, serde_json::Error> {
    let mut variables = HashMap::new();

    // These become environment variables! ❌
    variables.insert("map.successful".to_string(), summary.successful.to_string());
    variables.insert("map.failed".to_string(), summary.failed.to_string());
    variables.insert("map.total".to_string(), summary.total.to_string());

    // This serializes ALL map results into a HUGE env var! ❌
    let results_json = serde_json::to_string(map_results)?;
    variables.insert("map.results".to_string(), results_json);

    // For each result, add 4+ more env vars! ❌
    for (index, result) in map_results.iter().enumerate() {
        add_individual_result_variables(&mut variables, index, result);
    }

    Ok(variables)
}
```

For 13 map agents, this creates **55+ environment variables**, with `map.results` potentially being megabytes of JSON.

### Implementation Approach

#### 1. Separate Variable Types

Create distinct types for different variable usage:

```rust
/// Variables for string interpolation in command text
pub struct InterpolationVariables {
    pub variables: HashMap<String, serde_json::Value>,
}

/// Environment variables to pass to process
pub struct ProcessEnvironment {
    pub env_vars: HashMap<String, String>,
}

/// Combined context for command execution
pub struct CommandContext {
    pub interpolation: InterpolationVariables,  // For ${var} substitution
    pub environment: ProcessEnvironment,         // For actual env vars
}
```

#### 2. Update Command Execution Flow

**Before** (current):
```
Workflow Variables → HashMap<String, String> → Process Environment
                                              ↓
                                    String Interpolation
```

**After** (new):
```
Workflow Variables → InterpolationVariables → String Interpolation ONLY

Env Block Variables → ProcessEnvironment → Process Environment ONLY
```

#### 3. Modify Shell Command Execution

Update `src/commands/handlers/shell.rs` to:

1. Perform string interpolation FIRST using InterpolationVariables
2. Pass ONLY ProcessEnvironment variables to the shell process
3. Never mix the two contexts

```rust
pub fn execute_shell_command(
    command: &str,
    context: &CommandContext,
) -> Result<CommandOutput> {
    // Step 1: Interpolate command text using workflow variables
    let interpolated_command = context
        .interpolation
        .interpolate_string(command)?;

    // Step 2: Execute with ONLY env block variables
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
       .arg(&interpolated_command)
       .envs(&context.environment.env_vars);  // Only these!

    cmd.output()
}
```

#### 4. Update MapReduce Variable Building

Refactor `build_agent_context_variables()` into two functions:

```rust
/// Build interpolation variables (for ${var} substitution)
pub fn build_interpolation_variables(
    map_results: &[AgentResult],
    summary: &MapResultSummary,
) -> InterpolationVariables {
    let mut vars = InterpolationVariables::new();

    // Add as JSON values, not strings
    vars.set("map.successful", json!(summary.successful));
    vars.set("map.failed", json!(summary.failed));
    vars.set("map.total", json!(summary.total));
    vars.set("map.results", serde_json::to_value(map_results).unwrap());

    // Individual results for access like ${result.0.item_id}
    for (index, result) in map_results.iter().enumerate() {
        vars.set(format!("result.{}.item_id", index), json!(result.item_id));
        vars.set(format!("result.{}.status", index), json!(result.status));
        // ... other fields
    }

    vars
}

/// Build process environment (ONLY from env blocks)
pub fn build_process_environment(
    workflow_env: &HashMap<String, String>,
) -> ProcessEnvironment {
    ProcessEnvironment {
        env_vars: workflow_env.clone()
    }
}
```

### Architecture Changes

**Modified Components**:
1. `src/cook/execution/mapreduce/utils.rs` - Split variable building
2. `src/commands/handlers/shell.rs` - Separate interpolation from env
3. `src/cook/execution/command.rs` - Update CommandContext
4. `src/cook/execution/variables.rs` - New variable type definitions
5. `src/cook/workflow/executor.rs` - Update execution flow

**New Components**:
1. `src/cook/execution/interpolation_variables.rs` - InterpolationVariables type
2. `src/cook/execution/process_environment.rs` - ProcessEnvironment type

### Data Flow

```
┌─────────────────────┐
│  Workflow Variables │
│  (map.*, item.*, etc)│
└──────────┬──────────┘
           │
           ▼
┌─────────────────────────┐
│ InterpolationVariables  │  ────► String Interpolation
│ (serde_json::Value)     │        in Command Text
└─────────────────────────┘

┌─────────────────────┐
│   Env Block         │
│   (workflow YAML)   │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────────┐
│  ProcessEnvironment     │  ────► Shell Process
│  (HashMap<String,String>)│        Environment
└─────────────────────────┘
```

### Implementation Strategy

**Direct implementation without backward compatibility:**
- Implement InterpolationVariables and ProcessEnvironment types
- Update all command execution paths to use new separation immediately
- Remove old variable-as-environment behavior entirely
- Update tests to reflect new behavior
- Breaking changes are acceptable - this fixes a critical bug

## Dependencies

**Prerequisites**: None

**Affected Components**:
- MapReduce execution engine
- Shell command handler
- Claude command handler
- Variable interpolation system
- Environment variable management
- Test suites for all command types

**External Dependencies**: None (internal refactoring only)

## Testing Strategy

### Unit Tests

1. **Variable Separation Tests**
   - Test InterpolationVariables stores JSON values correctly
   - Test ProcessEnvironment contains only env block variables
   - Test CommandContext maintains both contexts separately

2. **Interpolation Tests**
   - Test ${var} substitution from InterpolationVariables
   - Test nested access like ${map.results[0].item_id}
   - Test that interpolation doesn't modify ProcessEnvironment

3. **Environment Tests**
   - Test shell commands receive only env block variables
   - Test PRODIGY_* variables are still set for Claude
   - Test secret masking works in both contexts

### Integration Tests

1. **Large MapReduce Test**
   - Create workflow with 100 map agents
   - Each agent produces substantial output (1KB+)
   - Reduce phase executes shell command successfully
   - Verify no E2BIG errors
   - Validate command receives only env block variables

2. **Variable Interpolation Test**
   - Use ${map.results} in command text
   - Verify interpolation works correctly
   - Verify results NOT in process environment

3. **Existing Workflow Test**
   - Update and run existing workflows with new implementation
   - Update tests to reflect new behavior
   - Ensure all core functionality works correctly

### Performance Tests

1. **Interpolation Performance**
   - Benchmark variable interpolation speed
   - Compare old vs new implementation
   - Target: < 5% performance regression

2. **Memory Usage**
   - Measure memory overhead of new types
   - Compare to old implementation
   - Target: < 10% memory increase

### User Acceptance

1. **Real-world Workflow Test**
   - Run book-docs-drift workflow (13 chapters)
   - Verify successful completion
   - Validate mdbook build executes without E2BIG

2. **Developer Experience**
   - Verify error messages are clear
   - Check documentation is comprehensive
   - Validate new design is intuitive

## Documentation Requirements

### Code Documentation

1. Add comprehensive doc comments to new types
2. Document the separation between interpolation and environment
3. Explain when to use each variable type
4. Provide examples of correct usage

### User Documentation

1. **ARCHITECTURE.md Updates**
   - Document new variable handling design
   - Explain interpolation vs environment distinction
   - Show data flow diagrams
   - Reference Ansible's design pattern

2. **User Guide Updates**
   - Explain env block for environment variables
   - Show how to use variables in command text
   - Clarify ${var} is for interpolation only
   - Provide migration examples

3. **Breaking Changes Guide**
   - Document the change in behavior
   - Explain why workflow variables are no longer in environment
   - Show examples of proper env block usage
   - Troubleshooting common issues

### Architecture Updates

Add new section to ARCHITECTURE.md:

```markdown
## Variable Handling Architecture

### Design Principles

Prodigy follows Ansible's design pattern for variable handling:

1. **Interpolation Variables**: Used ONLY for string substitution in command text
2. **Environment Variables**: Explicitly defined variables passed to process environment

This separation ensures:
- No OS argument list limits from large workflow state
- Clear security boundary for sensitive data
- Better performance and maintainability
- Familiar mental model for users of similar tools

### Implementation

[diagrams and details]
```

## Implementation Notes

### Common Pitfalls

1. **Don't serialize large objects to env vars**: Use interpolation instead
2. **Don't pass workflow variables to process environment**: Only env block vars
3. **Don't forget to interpolate first**: Always interpolate command text before execution
4. **Don't forget Claude commands**: They need special handling too

### Best Practices

1. Use `serde_json::Value` for interpolation variables (supports nested access)
2. Use `HashMap<String, String>` for environment (matches OS expectations)
3. Interpolate strings BEFORE executing commands
4. Never modify ProcessEnvironment based on workflow state

### Security Considerations

1. Secret masking must work in BOTH contexts
2. Log sanitization must check both interpolation and environment
3. Environment variables visible in `ps` output - be cautious
4. Interpolated values only visible in command logs

## Migration and Compatibility

### Breaking Changes

**YES** - This is a breaking change to fix a critical bug.

**What Changes**:
- Workflow variables (map.*, item.*, captured outputs) are NO LONGER passed as environment variables to shell commands
- Only variables explicitly defined in `env` blocks are passed to process environment
- String interpolation (${var}) continues to work exactly as before

**Why Breaking is Acceptable**:
- Fixes critical E2BIG errors that make MapReduce unusable at scale
- Current behavior is a bug, not a feature
- No known workflows rely on accessing workflow variables via environment (all use ${} syntax)
- Improves security and performance significantly

### Implementation Requirements

**For Internal Code**:
1. Update all command execution paths to use new CommandContext
2. Migrate variable building functions
3. Update tests to reflect new behavior
4. Add integration tests for large-scale scenarios

**For Workflows**:
- If workflows use `${map.results}` syntax: No changes needed (continues to work)
- If workflows access environment variables directly: Must use `env` block
- Most workflows unaffected as they use interpolation syntax

### Implementation Strategy

1. **Week 1**: Implement new types and infrastructure
2. **Week 2**: Update all command execution paths
3. **Week 3**: Update tests and add integration tests
4. **Week 4**: Update documentation and verify all workflows work
5. **Complete**: Deploy with breaking change notice

## Success Metrics

1. **Reliability**: Zero E2BIG errors in MapReduce workflows of any size
2. **Performance**: < 5% regression in variable interpolation speed
3. **Security**: 90% reduction in environment variable count for typical workflows
4. **Testing**: All tests pass after necessary updates
5. **Adoption**: All book-docs-drift workflows complete successfully without E2BIG errors

## Related Specifications

- Spec 120: Environment Variables Support (builds on this)
- Spec 117: MapReduce Custom Merge Workflows (benefits from this fix)
- Future: Working Directory Support (similar separation of concerns)

## References

- Ansible Variable Handling: https://docs.ansible.com/ansible/latest/playbook_guide/playbooks_variables.html
- Unix Environment Limits: https://www.in-ulm.de/~mascheck/various/argmax/
- Issue: book-docs-drift.yml E2BIG error with 13 map agents
