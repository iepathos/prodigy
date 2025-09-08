---
number: 62
title: Verbose Variable Output for Workflow Execution
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-01-08
---

# Specification 62: Verbose Variable Output for Workflow Execution

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

When Prodigy executes workflows, it logs various messages about the execution progress, including which steps are being run and which commands are being executed. However, when workflows use variables (like `$ARG`, `${validation.gaps}`, `${item.field}`, etc.), the actual values of these variables are not displayed in the logs. This makes it difficult to debug workflows or understand exactly what data is being passed between steps, especially in complex workflows with multiple variable substitutions.

Currently, running with the `-v` (verbose) flag provides additional output, but does not include the resolved variable values. Users must infer what values are being used based on context or add debug steps to explicitly output variable values.

## Objective

Enhance Prodigy's verbose logging mode to display the actual values of variables when they are used in workflow commands, making it easier to debug and understand workflow execution flow.

## Requirements

### Functional Requirements
- When running with `-v` flag, display the resolved values of all variables used in commands
- Show variable values in a clear, readable format that distinguishes them from regular log output
- Support all variable types: `$ARG`, `${...}` interpolations, and environment variables
- Display variable values at the point of substitution, just before command execution
- Handle complex variable values (arrays, objects) with appropriate formatting
- Ensure sensitive data can be masked if needed (e.g., tokens, passwords)

### Non-Functional Requirements
- Minimal performance impact when verbose mode is not enabled
- Clear, consistent formatting that integrates well with existing log output
- Variable output should not interfere with command execution or parsing
- Support for large variable values without cluttering the output

## Acceptance Criteria

- [ ] Running with `-v` flag shows variable values for all command executions
- [ ] Variable output clearly indicates variable name and resolved value
- [ ] Complex data structures (JSON objects, arrays) are formatted readably
- [ ] Variable logging works for all variable types (`$ARG`, `${...}`, environment)
- [ ] Non-verbose mode remains unchanged with no performance impact
- [ ] Variable values are shown at the correct point in execution flow
- [ ] Large variable values are truncated appropriately with indication
- [ ] Sensitive values can be masked when needed
- [ ] Documentation is updated to explain verbose variable output

## Technical Details

### Implementation Approach
1. Enhance the command execution logging in `src/workflow/executor.rs`
2. Add variable resolution tracking to capture name-value pairs
3. Format and display variable values when verbose mode is active
4. Implement smart formatting for different data types

### Architecture Changes
- Modify `CommandExecutor` to track variable substitutions
- Enhance `Logger` to support variable value formatting
- Add configuration for sensitive variable masking

### Data Structures
```rust
struct VariableResolution {
    name: String,
    raw_expression: String,
    resolved_value: String,
    is_sensitive: bool,
}
```

### APIs and Interfaces
- No external API changes
- Internal logging interface enhanced with variable tracking
- Configuration option for sensitive variable patterns

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - Workflow executor
  - Command runner
  - Logging subsystem
  - Variable interpolation engine
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test variable resolution tracking and formatting
- **Integration Tests**: Verify variable output in complete workflow runs
- **Performance Tests**: Ensure no impact when verbose mode is disabled
- **User Acceptance**: Test with real workflows containing complex variables

## Documentation Requirements

- **Code Documentation**: Document new variable tracking methods
- **User Documentation**: Update CLI help and README with verbose variable output examples
- **Architecture Updates**: None required

## Implementation Notes

Example output format when running with `-v`:
```
🔄 Executing step 1/3: claude: /prodigy-implement-spec $ARG
   📊 Variable $ARG = "98"
🔄 Running validation (Claude): /prodigy-validate-spec 98 --output .prodigy/validation-result.json
⚠️ Validation incomplete: 95.0% complete (threshold: 100.0%)
ℹ️ Attempting to complete implementation (attempt 1/3)
🔄 Running recovery step: claude: /prodigy-complete-spec $ARG --gaps ${validation.gaps}
   📊 Variable $ARG = "98"
   📊 Variable ${validation.gaps} = ["missing test coverage", "incomplete error handling"]
```

Consider using different formatting for different types:
- Simple values: `Variable $ARG = "98"`
- Arrays: Pretty-printed JSON or comma-separated list
- Objects: Indented JSON format
- Large values: Truncated with `... (showing first 200 chars)`

## Migration and Compatibility

No breaking changes. The feature is opt-in via the existing `-v` flag, maintaining full backward compatibility. Existing workflows and scripts will continue to function identically unless verbose mode is explicitly enabled.