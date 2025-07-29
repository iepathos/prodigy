# Specification 42: Fix Ignored Integration Tests

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The project has two integration tests that are currently marked as `#[ignore]` in `tests/cook_iteration_tests.rs`:

1. `test_cook_stops_early_when_no_changes` - Tests that the cook command stops early when no changes are found by git
2. `test_focus_applied_every_iteration` - Tests that focus directive is applied on every iteration, not just the first

These tests are critical for ensuring proper behavior of the cook command but are currently ignored because they require "more complex mocking" according to the comments. However, the issue isn't really about complexity - it's about properly simulating the conditions these tests need to verify.

## Objective

Enable these ignored tests by implementing proper test infrastructure that allows us to:
1. Simulate the "no changes found" condition in git after an iteration
2. Track focus directive application across multiple iterations

## Requirements

### Functional Requirements
- Make `test_cook_stops_early_when_no_changes` pass by properly simulating the no-changes condition
- Make `test_focus_applied_every_iteration` pass by tracking focus application
- Maintain test isolation and determinism
- Ensure tests run reliably in CI/CD environments

### Non-Functional Requirements
- Tests should run quickly (< 1 second each)
- No external dependencies or network calls
- Clear test output for debugging failures
- Minimal changes to production code

## Acceptance Criteria

- [ ] Remove `#[ignore]` attribute from both tests
- [ ] `test_cook_stops_early_when_no_changes` passes reliably
- [ ] `test_focus_applied_every_iteration` passes reliably
- [ ] All existing tests continue to pass
- [ ] Tests work in both local and CI environments
- [ ] No changes to production behavior

## Technical Details

### Implementation Approach

1. **For `test_cook_stops_early_when_no_changes`**:
   - The test needs to simulate a condition where git shows no changes after the first iteration
   - This happens when `git diff` returns empty after the cook step
   - We can achieve this by ensuring mock commands don't modify any files

2. **For `test_focus_applied_every_iteration`**:
   - The test needs to verify that the focus directive is passed to every iteration
   - Currently using `MMM_TRACK_FOCUS` environment variable approach
   - Need to implement actual tracking in test mode

### Architecture Changes
None - only test infrastructure changes needed

### Data Structures
No new data structures required

### APIs and Interfaces
No API changes - only test utilities

## Dependencies

- **Prerequisites**: None
- **Affected Components**: Integration tests only
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Not applicable (fixing integration tests)
- **Integration Tests**: The two tests being fixed
- **Performance Tests**: Ensure tests complete quickly
- **User Acceptance**: Tests should provide clear failure messages

## Documentation Requirements

- **Code Documentation**: Document test helpers and mocking approach
- **User Documentation**: Not applicable
- **Architecture Updates**: Not needed

## Implementation Notes

The key insight is that these tests don't need complex mocking - they need:

1. **Deterministic git behavior**: Ensure git operations in tests produce expected results
2. **Proper test mode handling**: The `MMM_TEST_MODE` environment variable should enable necessary test behaviors
3. **Focus tracking**: Simple file-based tracking when in test mode

The tests are well-structured; they just need the supporting infrastructure to work properly.

## Migration and Compatibility

No migration needed - only test improvements