---
number: 104
title: Automatic Git Initialization on Prodigy Init
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-09-19
---

# Specification 104: Automatic Git Initialization on Prodigy Init

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Currently, when running `prodigy init` in a directory that is not a git repository, the command fails with an error message indicating that the directory must be a git repository. This creates unnecessary friction for users who want to use Prodigy in new projects or directories where git hasn't been initialized yet.

Additionally, the error messages still reference "MMM" (the old project name) instead of "Prodigy", creating confusion about the actual tool being used.

## Objective

Enhance the `prodigy init` command to automatically initialize a git repository when executed in a non-git directory, provided git is installed on the system. This will improve the user experience by removing an unnecessary manual step and make Prodigy more accessible for new projects. Additionally, update all references from "MMM" to "Prodigy" throughout the codebase.

## Requirements

### Functional Requirements
- Detect when `prodigy init` is run in a non-git directory
- Check if git is installed on the system
- Automatically run `git init` when both conditions are met
- Proceed with normal initialization after git repository is created
- Update all error messages and user-facing text to use "Prodigy" instead of "MMM"
- Preserve existing behavior when run in an already initialized git repository

### Non-Functional Requirements
- Provide clear feedback to users about the automatic git initialization
- Maintain backward compatibility with existing workflows
- Ensure error messages are helpful and actionable
- Keep initialization process efficient and fast

## Acceptance Criteria

- [ ] Running `prodigy init` in a non-git directory automatically initializes a git repository
- [ ] Appropriate message is displayed when git repository is automatically created
- [ ] Command fails gracefully with helpful error if git is not installed
- [ ] All references to "MMM" in error messages and output are replaced with "Prodigy"
- [ ] Existing functionality is preserved when running in an already initialized git repository
- [ ] Unit tests cover the new git initialization logic
- [ ] Integration tests verify end-to-end behavior in both git and non-git directories
- [ ] Documentation is updated to reflect the new automatic initialization feature

## Technical Details

### Implementation Approach
1. Modify the initialization logic to check git repository status
2. Add git installation detection using `which git` or similar command
3. Execute `git init` programmatically when conditions are met
4. Update all string literals containing "MMM" to "Prodigy"
5. Add appropriate logging and user feedback for the initialization process

### Architecture Changes
- Modify `prodigy init` command handler
- Add git detection and initialization utilities
- Update error handling and messaging throughout

### Data Structures
No new data structures required, but existing error types may need updates.

### APIs and Interfaces
- No external API changes
- Internal initialization flow will be modified
- Error messages interface will be updated

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - CLI initialization module
  - Error messaging system
  - Command validation logic
- **External Dependencies**:
  - Git command-line tool (runtime check)

## Testing Strategy

- **Unit Tests**:
  - Test git detection logic
  - Test initialization in various directory states
  - Verify error handling when git is not available
  - Verify all MMM references are updated
- **Integration Tests**:
  - End-to-end test of `prodigy init` in non-git directory
  - Test with and without git installed
  - Verify proper messaging and feedback
- **Performance Tests**:
  - Ensure initialization time is not significantly impacted
- **User Acceptance**:
  - Test with users new to Prodigy
  - Verify improved workflow for new project setup

## Documentation Requirements

- **Code Documentation**:
  - Document the automatic initialization logic
  - Add comments explaining git detection approach
- **User Documentation**:
  - Update README to mention automatic git initialization
  - Update getting started guide
  - Add FAQ entry about git requirements
- **Architecture Updates**:
  - Document the initialization flow changes

## Implementation Notes

- Consider using standard library functions for checking git availability
- Ensure proper error handling for edge cases (e.g., permission issues)
- Consider adding a `--no-git-init` flag for users who want to opt out
- The git initialization should be silent unless there's an error
- Log the automatic initialization at appropriate verbosity level

## Migration and Compatibility

- No breaking changes for existing users
- Existing git repositories will continue to work as before
- The change is purely additive, enhancing the initialization experience
- No data migration required
- All existing workflows remain compatible