# Specification 50: Inter-Iteration Analysis Updates

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [44 (Context-Aware Project Understanding), 46 (Real Metrics Tracking)]

## Context

Currently, MMM runs project analysis only at the beginning of the `cook` workflow. This means that changes made during each iteration are not reflected in the context and analysis data available to subsequent iterations. As demonstrated in a recent workflow run with 5 iterations, each iteration makes changes but subsequent iterations don't have updated analysis reflecting those changes. This leads to:

- Stale context data for later iterations
- Potential redundant or conflicting changes
- Missed opportunities for compound improvements
- Inaccurate technical debt assessments after changes

## Objective

Update the MMM cook workflow to run project analysis after every iteration completes, ensuring that each subsequent iteration has access to the most current codebase state and analysis data.

## Requirements

### Functional Requirements
- Run full project analysis after each workflow iteration completes
- Update all context files in `.mmm/context/` with fresh analysis data
- Preserve incremental analysis capabilities for performance
- Ensure analysis completes before the next iteration begins
- Handle analysis failures gracefully without breaking the workflow
- Respect the `--skip-analysis` flag - when specified, skip both initial and inter-iteration analysis

### Non-Functional Requirements
- Minimal performance impact (use incremental analysis where possible)
- Maintain backward compatibility with existing workflows
- Clear logging to indicate when analysis is running
- Configurable option to disable inter-iteration analysis if needed

## Acceptance Criteria

- [ ] Analysis runs automatically after each iteration in the cook workflow
- [ ] Context files in `.mmm/context/` are updated with fresh data after each iteration
- [ ] Subsequent iterations can access updated analysis reflecting previous changes
- [ ] Performance impact is acceptable (< 30 seconds for typical projects)
- [ ] Analysis failures are logged but don't halt the workflow
- [ ] Feature can be disabled via configuration flag if needed
- [ ] Existing workflows continue to function without modification
- [ ] Analysis status is clearly indicated in workflow output
- [ ] When `--skip-analysis` flag is used, no analysis runs (neither initial nor inter-iteration)

## Technical Details

### Implementation Approach

1. Modify the `cook` module to insert analysis step after each iteration
2. Leverage existing `analyze_project` functionality
3. Use incremental analysis to minimize performance impact
4. Update workflow state to track analysis runs

### Architecture Changes

The main changes will be in the `cook.rs` module:
- Add analysis step after `workflow.execute_once()` 
- Ensure analysis completes before checking for more iterations
- Update logging to show analysis progress
- Check for `skip_analysis` flag before running inter-iteration analysis

### Data Flow

```
Start Cook
  ↓
--skip-analysis?
  ↓ No
Initial Analysis
  ↓
┌─→ Execute Iteration
│     ↓
│   Changes Made?
│     ↓ Yes
│   --skip-analysis?
│     ↓ No
│   Run Analysis ← NEW STEP
│     ↓
│   Update Context Files
│     ↓
│   More Iterations?
│     ↓ Yes
└─────┘
  ↓ No
Complete
```

### APIs and Interfaces

No new APIs required. Will reuse existing:
- `analyze_project()` from analyzer module
- Context file writing utilities
- Existing MMM_CONTEXT environment variables

## Dependencies

- **Prerequisites**: 
  - Spec 44: Context-Aware Project Understanding (for analysis infrastructure)
  - Spec 46: Real Metrics Tracking (for metrics updates)
- **Affected Components**: 
  - `cook.rs` - Main workflow orchestration
  - `analyzer/` - May need optimization for repeated runs
  - `.mmm/context/` - More frequent updates
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Mock analysis runs between iterations
  - Verify analysis is called correct number of times
  - Test failure handling
- **Integration Tests**: 
  - Run multi-iteration workflow and verify context updates
  - Check that second iteration sees first iteration's changes
  - Verify performance with real projects
- **Performance Tests**: 
  - Measure analysis time for incremental vs full runs
  - Ensure acceptable performance for 5+ iterations
- **User Acceptance**: 
  - Run example workflows and verify improved results
  - Check that context accurately reflects changes

## Documentation Requirements

- **Code Documentation**: 
  - Document the analysis insertion point in cook workflow
  - Explain incremental vs full analysis decision logic
- **User Documentation**: 
  - Update CLAUDE.md to reflect new analysis behavior
  - Add configuration option documentation
  - Update troubleshooting for analysis-related issues
- **Architecture Updates**: 
  - Update workflow diagrams to show inter-iteration analysis

## Implementation Notes

- Consider caching analysis results to avoid redundant work
- Incremental analysis should track which files changed
- May want to batch small changes before triggering analysis
- Consider parallel analysis if it doesn't interfere with next iteration
- Log analysis duration to help users understand performance impact
- The `--skip-analysis` flag should be consistently respected throughout the workflow
- When analysis is skipped, commands should still function using existing context if available

## Migration and Compatibility

- No breaking changes - enhancement to existing workflow
- Workflows without the flag will behave as before
- Consider making this opt-in initially, then default in future version
- Existing `.mmm/context/` structure remains compatible