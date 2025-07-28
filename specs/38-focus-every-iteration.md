# Specification 38: Focus Directive on Every Iteration

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: 20

## Context

Currently, the focus directive (specified via `--focus` flag) is only passed to the `/mmm-code-review` command on the first iteration of the improvement loop. This behavior is implemented in multiple places in the codebase where the focus is conditionally set based on `iteration == 1`:

```rust
let focus_for_iteration = if iteration == 1 {
    cmd.focus.as_deref()
} else {
    None
};
```

This limitation means that subsequent iterations lose the context of what aspect the user wants to focus on, potentially causing the improvement process to drift away from the intended focus area after the first iteration.

## Objective

Modify the improvement loop to pass the focus directive to every iteration of the improvement process, ensuring consistent focus throughout the entire improvement session.

## Requirements

### Functional Requirements
- Pass the focus directive to `/mmm-code-review` on every iteration, not just the first
- Maintain the same focus throughout the entire improvement session
- Ensure focus is consistently applied in all execution paths (standard, worktree, mapping)
- No change to the CLI interface - existing `--focus` flag behavior remains the same

### Non-Functional Requirements
- No performance impact from passing focus on every iteration
- Maintain backward compatibility with existing workflows
- Clear code that makes the intent obvious

## Acceptance Criteria

- [ ] Focus directive is passed on every iteration in standard improvement loop
- [ ] Focus directive is passed on every iteration in worktree-based improvement
- [ ] Focus directive is passed on every iteration when using mapping/batch mode
- [ ] Focus directive is passed on every iteration in configurable workflows
- [ ] All existing tests continue to pass
- [ ] New tests verify focus is passed on multiple iterations

## Technical Details

### Implementation Approach

The implementation requires updating four locations in `src/cook/mod.rs` where the focus directive is conditionally applied:

1. **In `run_improvement_loop` (around line 507)**:
   ```rust
   // Change from:
   let focus_for_iteration = if iteration == 1 {
       cmd.focus.as_deref()
   } else {
       None
   };
   
   // To:
   let focus_for_iteration = cmd.focus.as_deref();
   ```

2. **In `run_without_worktree_with_vars` (around line 720)**:
   ```rust
   // Change from:
   let focus_for_iteration = if iteration == 1 {
       cmd.focus.as_deref()
   } else {
       None
   };
   
   // To:
   let focus_for_iteration = cmd.focus.as_deref();
   ```

3. **In `run_without_worktree_with_vars` legacy workflow (around line 748)**:
   ```rust
   // Change from:
   let focus_for_iteration = if iteration == 1 {
       cmd.focus.as_deref()
   } else {
       None
   };
   
   // To:
   let focus_for_iteration = cmd.focus.as_deref();
   ```

4. **In `run_improvement_loop_with_variables` (around line 1250)**:
   ```rust
   // Change from:
   let focus_for_iteration = if iteration == 1 {
       cmd.focus.as_deref()
   } else {
       None
   };
   
   // To:
   let focus_for_iteration = cmd.focus.as_deref();
   ```

### Architecture Changes

- No architectural changes required
- Simple code modification in existing improvement loop logic
- No new modules or dependencies needed

### Example Behavior

Before:
```bash
$ mmm cook --focus "performance" --max-iterations 3

🔄 Iteration 1/3...
📊 Focus: performance
🤖 Running /mmm-code-review... (with MMM_FOCUS=performance)

🔄 Iteration 2/3...
🤖 Running /mmm-code-review... (without focus)

🔄 Iteration 3/3...
🤖 Running /mmm-code-review... (without focus)
```

After:
```bash
$ mmm cook --focus "performance" --max-iterations 3

🔄 Iteration 1/3...
📊 Focus: performance
🤖 Running /mmm-code-review... (with MMM_FOCUS=performance)

🔄 Iteration 2/3...
🤖 Running /mmm-code-review... (with MMM_FOCUS=performance)

🔄 Iteration 3/3...
🤖 Running /mmm-code-review... (with MMM_FOCUS=performance)
```

## Dependencies

- **Prerequisites**: 
  - Spec 20 (Focus-directed improvements) - Base focus functionality
- **Affected Components**: 
  - `src/cook/mod.rs` - Improvement loop logic
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Verify focus is passed on multiple iterations
  - Test with and without focus directive
  - Ensure focus is passed in all execution modes
- **Integration Tests**: 
  - Full improvement cycle with focus verification
  - Worktree mode with focus
  - Mapping mode with focus
- **Manual Testing**: 
  - Run multiple iterations with verbose output
  - Verify MMM_FOCUS environment variable is set on each iteration
  - Test with different focus values

## Documentation Requirements

- **Code Documentation**: 
  - Update comments to explain focus is passed on every iteration
  - Document the rationale for consistent focus
- **User Documentation**: 
  - Update documentation to clarify focus applies to all iterations
  - Add examples showing multi-iteration focused improvements
- **Architecture Updates**: 
  - Note in ARCHITECTURE.md that focus is consistent across iterations

## Implementation Notes

1. **Simplicity**: This is a straightforward change - remove the conditional logic and always pass the focus
2. **Consistency**: Ensures all iterations stay focused on the user's intended improvement area
3. **No Breaking Changes**: Existing behavior is enhanced, not changed
4. **Testing**: Can verify by checking environment variables in verbose mode

## Migration and Compatibility

- No migration required - this is an enhancement to existing behavior
- Fully backward compatible - users without `--focus` see no change
- Users with `--focus` get improved, more consistent behavior
- No configuration changes needed

## Success Metrics

- Focus directive consistently applied across all iterations
- User feedback indicates improvements stay on-topic throughout session
- No regression in existing functionality
- Improved user satisfaction with focused improvement sessions

## Future Enhancements

- Allow focus to be modified mid-session via configuration
- Support multiple focus areas with priority weighting
- Dynamic focus adjustment based on improvement progress
- Focus inheritance in nested improvement sessions