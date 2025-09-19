---
number: 102
title: Update Spec Workflow to Use Git Context Variables
category: optimization
priority: high
status: draft
dependencies: [101]
created: 2024-01-18
---

# Specification 102: Update Spec Workflow to Use Git Context Variables

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [101 - Git Context Variables for Workflows]

## Context

The current spec.yml workflow relies on complex shell commands to extract information about newly created specification files. This approach is error-prone, difficult to maintain, and fails to handle cases where multiple specification files might be created from a single request.

With the implementation of spec 101 (Git Context Variables), we will have automatic tracking of file changes available as workflow variables. This specification defines how to update the spec workflow and its associated Claude commands to leverage these new capabilities.

The current issues include:
- Shell commands like `ls -t specs/*.md | head -1` assume only one spec is created
- Complex sed/grep pipelines are fragile and hard to understand
- No proper handling of multiple spec creation scenarios
- Validation logic is tightly coupled to single-spec assumptions

## Objective

Refactor the spec.yml workflow and associated Claude commands to use git context variables, enabling proper support for multiple specification creation and eliminating complex shell scripting.

## Requirements

### Functional Requirements

1. **Workflow Refactoring**
   - Replace shell-based file detection with git context variables
   - Support validation of multiple specifications in a single workflow run
   - Handle both single and multiple spec creation scenarios gracefully
   - Maintain backward compatibility with existing spec validation logic

2. **Command Updates**
   - Update `/prodigy-add-spec` to support creating multiple focused specs when needed
   - Modify `/prodigy-validate-spec-completeness` to handle multiple specs
   - Create `/prodigy-validate-all-specs` for batch validation
   - Update `/prodigy-refine-spec` to work with multiple specs

3. **Variable Usage**
   - Use `${step.files_added:specs/*.md}` to detect new spec files
   - Leverage `${step.files_modified:specs/*.md}` for refinement tracking
   - Utilize `${workflow.files_added:specs/*.md}` for final reporting
   - Support pattern-based filtering for spec file identification

4. **Validation Improvements**
   - Aggregate validation results across multiple specs
   - Provide per-spec and overall completion percentages
   - Support batch refinement of incomplete specs
   - Generate comprehensive validation reports

### Non-Functional Requirements

1. **Simplicity**
   - Workflow should be more readable and maintainable
   - Eliminate complex shell command pipelines
   - Clear separation of concerns between steps

2. **Reliability**
   - Handle edge cases (no specs created, many specs created)
   - Graceful error handling and reporting
   - Consistent behavior across different scenarios

3. **Performance**
   - Efficient batch processing of multiple specs
   - Minimize redundant git operations
   - Optimize validation loops

## Acceptance Criteria

- [ ] spec.yml workflow updated to use git context variables
- [ ] All shell-based file detection removed from workflow
- [ ] Support for multiple spec creation implemented
- [ ] `/prodigy-add-spec` can create multiple specs when appropriate
- [ ] `/prodigy-validate-all-specs` command created and functional
- [ ] Validation works correctly for 1-N specifications
- [ ] Refinement process handles multiple specs efficiently
- [ ] Final reporting shows all created specifications
- [ ] Integration tests cover single and multiple spec scenarios
- [ ] Documentation updated with new workflow patterns

## Technical Details

### Implementation Approach

1. **Phase 1: Command Updates**
   - Enhance `/prodigy-add-spec` to detect when splitting is needed
   - Create `/prodigy-validate-all-specs` for batch validation
   - Update existing validation commands to support lists

2. **Phase 2: Workflow Refactoring**
   - Replace shell commands with git context variables
   - Implement proper validation flow for multiple specs
   - Add comprehensive reporting

3. **Phase 3: Testing and Documentation**
   - Create test cases for various scenarios
   - Update command documentation
   - Provide migration examples

### Architecture Changes

#### Updated Workflow Structure
```yaml
# Step 1: Generate initial specification(s)
- claude: "/prodigy-add-spec $ARG"
  commit_required: true

# Step 2: Validate all created specifications
- claude: "/prodigy-validate-all-specs ${step.files_added:specs/*.md} --original '$ARG'"
  validate:
    result_file: ".prodigy/spec-validation.json"
    threshold: 100
    on_incomplete:
      claude: "/prodigy-refine-specs ${validation.incomplete_specs} --gaps ${validation.gaps}"
      max_attempts: 5
      commit_required: true

# Step 3: Final reporting
- claude: "/prodigy-report-specs ${workflow.files_added:specs/*.md}"
```

#### Command Modifications

1. **Enhanced /prodigy-add-spec**
   - Analyze request complexity
   - Determine if splitting is beneficial
   - Create multiple focused specs when appropriate
   - Return list of created files

2. **New /prodigy-validate-all-specs**
   ```
   Usage: /prodigy-validate-all-specs <spec-files> --original <description>

   Input: Space-separated list of spec files
   Output: Aggregated validation JSON with per-spec and overall results
   ```

3. **Updated /prodigy-refine-specs**
   ```
   Usage: /prodigy-refine-specs <spec-list> --gaps <validation-gaps>

   Batch refinement of multiple specifications
   ```

### Data Structures

```json
// Aggregated validation result
{
  "overall_completion_percentage": 95.0,
  "all_focused": true,
  "specs": [
    {
      "file": "specs/102-update-spec-workflow.md",
      "completion_percentage": 100.0,
      "is_focused": true,
      "gaps": {}
    },
    {
      "file": "specs/103-another-spec.md",
      "completion_percentage": 90.0,
      "is_focused": true,
      "gaps": {
        "missing_tests": {
          "description": "Test strategy not defined",
          "severity": "medium"
        }
      }
    }
  ],
  "incomplete_specs": ["specs/103-another-spec.md"],
  "validation_timestamp": "2024-01-18T12:00:00Z"
}
```

### APIs and Interfaces

```yaml
# Workflow context variables (from spec 101)
${step.files_added}          # All files added in step
${step.files_added:*.md}     # Only markdown files added
${step.files_added:specs/*}  # Only files in specs directory

# Command interfaces
/prodigy-add-spec <description>
  # May create 1-N spec files based on complexity

/prodigy-validate-all-specs <files> --original <description>
  # Validates multiple specs, returns aggregated results

/prodigy-refine-specs <files> --gaps <gap-json>
  # Refines multiple specs based on validation gaps
```

## Dependencies

- **Prerequisites**:
  - Spec 101 (Git Context Variables) must be implemented
- **Affected Components**:
  - workflows/spec.yml
  - .claude/commands/prodigy-add-spec.md
  - .claude/commands/prodigy-validate-spec-completeness.md
  - .claude/commands/prodigy-refine-spec.md
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test variable interpolation with spec file patterns
  - Verify batch validation logic
  - Test aggregation of multiple validation results

- **Integration Tests**:
  - Single spec creation workflow
  - Multiple spec creation workflow
  - Mixed validation results (some complete, some incomplete)
  - Refinement iterations with multiple specs

- **Performance Tests**:
  - Validate 10+ specs in single workflow
  - Measure overhead vs. shell commands
  - Test with large spec files

- **User Acceptance**:
  - Run workflow with various feature descriptions
  - Verify all specs are properly validated
  - Confirm refinement process works smoothly

## Documentation Requirements

- **Code Documentation**:
  - Document new command parameters
  - Explain validation aggregation logic
  - Provide examples of multi-spec handling

- **User Documentation**:
  - Update workflow guide with new patterns
  - Provide before/after comparisons
  - Include troubleshooting guide

- **Migration Guide**:
  - Step-by-step migration from old workflow
  - Common patterns and replacements
  - Testing recommendations

## Implementation Notes

1. **Backward Compatibility**:
   - Ensure single-spec workflows still work
   - Support existing validation result format
   - Gradual migration path

2. **Edge Cases**:
   - No specs created (error condition)
   - Very large number of specs (performance)
   - Partial failures during creation
   - Network interruptions during validation

3. **Future Enhancements**:
   - Parallel validation of multiple specs
   - Incremental refinement strategies
   - Smart spec splitting heuristics

## Migration and Compatibility

1. **Migration Steps**:
   - Install spec 101 implementation
   - Update workflow files
   - Update Claude commands
   - Test with existing spec descriptions
   - Remove deprecated shell commands

2. **Rollback Plan**:
   - Keep backup of original workflow
   - Document fallback procedures
   - Maintain shell command compatibility temporarily

3. **Deprecation Timeline**:
   - Week 1-2: Deploy new workflow alongside old
   - Week 3-4: Migrate active users
   - Week 5-6: Deprecate old workflow
   - Week 8: Remove old implementation

## Example Usage

### Before (Current Implementation)
```yaml
# Complex shell commands to extract spec number
- shell: "ls -t specs/*.md | head -1 | sed 's/specs\\///' | sed 's/-.*//' | tr -d '\n'"
  capture_output: spec_number

- claude: "/prodigy-validate-spec-completeness ${shell.spec_number} $ARG"
```

### After (With Git Context Variables)
```yaml
# Clean, simple variable usage
- claude: "/prodigy-validate-all-specs ${step.files_added:specs/*.md} --original '$ARG'"
```

### Multiple Spec Creation Example
```yaml
# User requests: "Add authentication, authorization, and session management"
# System creates three focused specs

- claude: "/prodigy-add-spec $ARG"
  # Creates: specs/102-authentication.md
  #          specs/103-authorization.md
  #          specs/104-session-management.md

- claude: "/prodigy-validate-all-specs ${step.files_added:specs/*.md}"
  # Validates all three specs, returns aggregated results

- claude: "/prodigy-report-specs ${workflow.files_added:specs/*.md}"
  # Reports: "Created 3 specifications: 102, 103, 104"
```

## Success Metrics

- 80% reduction in workflow complexity (line count)
- Zero shell commands for file detection in spec.yml
- Support for N specifications (tested up to 10)
- 100% backward compatibility maintained
- Positive user feedback on simplified workflow
- Reduced workflow failures due to shell scripting errors