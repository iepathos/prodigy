# Evidence for Command Types Chapter

## Source Definitions Found
- WorkflowStepCommand struct: src/config/command.rs:320-401 (user-facing YAML)
- WorkflowStep struct: src/cook/workflow/executor/data_structures.rs:35-157 (internal execution)
- OnFailureConfig enum: src/cook/workflow/on_failure.rs:67-115
- HandlerStrategy enum: src/cook/workflow/on_failure.rs:9-22
- FailureHandlerConfig struct: src/cook/workflow/on_failure.rs:24-49

## Critical Field Availability Issues

### Fields NOT in WorkflowStepCommand (cannot use in YAML):
- `cwd` / `working_dir` - Only in WorkflowStep:99
- `env` - Only in WorkflowStep:103
- `on_exit_code` - Only in WorkflowStep:119

### Fields IN WorkflowStepCommand (can use in YAML):
- All 24 fields documented in WorkflowStepCommand:320-401
- Notably includes: capture_output, on_failure, on_success, timeout, when, capture_format, capture_streams, output_file

## Test Examples Found
- No YAML examples using cwd, env, or on_exit_code found
- Comprehensive search of examples/ and workflows/ directories: 0 matches
- These fields exist only in internal WorkflowStep representation

## Configuration Examples Found
- All examples in book/src/commands.md use fields from WorkflowStepCommand
- Examples showing cwd, env, on_exit_code are INCORRECT per struct definition

## Validation Results
✓ WorkflowStepCommand fields verified against struct definition
✓ OnFailureConfig strategy enum variants verified (Recovery, Fallback, Cleanup, Custom)
✓ HandlerStrategy fields verified (strategy, timeout, capture, fail_workflow, handler_failure_fatal)
✗ cwd, env, on_exit_code documented but NOT available in user-facing YAML
✗ These fields only exist in internal WorkflowStep (execution layer)

## Discovery Notes
- WorkflowStepCommand = User-facing YAML deserialization struct
- WorkflowStep = Internal execution representation with additional fields
- Gap exists: working_dir, env, on_exit_code are internal-only
- Documentation incorrectly shows these as user-available fields

## Fix Strategy
1. Remove cwd, env, on_exit_code examples from main documentation
2. Add note explaining these are internal fields, not user-facing
3. Move these sections to "Technical Notes" or "Planned Features"
4. Add OnFailureConfig strategy field documentation (verified available)
5. Add capture field to main Command Fields table
6. Clarify capture_streams is planned/internal-only
