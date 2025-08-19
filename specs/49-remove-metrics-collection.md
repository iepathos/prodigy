---
number: 49
title: Remove Metrics Collection from MMM Core
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-08-19
---

# Specification 49: Remove Metrics Collection from MMM Core

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MMM currently contains built-in metrics collection functionality that duplicates the capabilities of specialized external tools. This represents architectural debt that violates the Single Responsibility Principle and bloats the codebase with non-essential functionality.

The current metrics collection system includes:
- Test coverage collection (duplicates `cargo tarpaulin`)
- Lint warning collection (duplicates `cargo clippy`)
- Code complexity analysis (duplicates `cargo-complexity`, `debtmap`)
- Compile time and binary size metrics (duplicates `cargo build --timings`)
- Code duplication detection (duplicates `cargo-duplication`, `debtmap`)

This functionality is triggered by the `collect_metrics: true` flag in workflows and executes redundant analysis commands after workflow completion, despite the workflow itself already running specialized tools like `debtmap` and `cargo tarpaulin`.

MMM should focus on its core responsibility: orchestrating AI-driven development workflows. Analysis and metrics collection should be delegated to specialized tools that are better maintained and more feature-complete.

## Objective

Remove all built-in metrics collection functionality from MMM, refocusing the tool on orchestration and coordination of external analysis tools. This will reduce code complexity, improve maintainability, and eliminate redundant analysis operations.

## Requirements

### Functional Requirements
- Remove all metrics collection modules from the codebase
- Remove the `collect_metrics` configuration option from workflows
- Preserve the ability to run external metrics tools through shell commands
- Maintain backward compatibility for workflows that don't use `collect_metrics`
- Ensure existing workflows can still capture metrics through external tool invocation

### Non-Functional Requirements
- Reduce binary size by removing unnecessary dependencies
- Improve build times by eliminating metrics-related code compilation
- Simplify the codebase architecture
- Improve separation of concerns
- Reduce maintenance burden

## Acceptance Criteria

- [ ] All metrics collection code removed from `src/cook/metrics/` directory
- [ ] All metrics collection code removed from `src/metrics/` directory
- [ ] The `collect_metrics` field removed from workflow configuration structs
- [ ] All metrics-related dependencies removed from `Cargo.toml`
- [ ] No references to `MetricsCoordinator` or `MetricsCollector` remain in the codebase
- [ ] All tests related to metrics collection removed or updated
- [ ] Documentation updated to remove references to built-in metrics collection
- [ ] Example workflows updated to use external tools directly for metrics
- [ ] Binary size reduced by at least 5%
- [ ] All existing workflows continue to function (except those using `collect_metrics`)
- [ ] Migration guide created for users currently using `collect_metrics`

## Technical Details

### Implementation Approach

1. **Phase 1: Analysis**
   - Identify all metrics-related code paths
   - Document current usage of metrics collection
   - Identify dependencies that can be removed

2. **Phase 2: Removal**
   - Remove `src/cook/metrics/` directory and all contents
   - Remove `src/metrics/` directory and all contents
   - Remove `collect_metrics` field from workflow configuration structs
   - Remove metrics-related trait implementations
   - Remove metrics-related dependencies from `Cargo.toml`

3. **Phase 3: Refactoring**
   - Update workflow executor to remove metrics collection calls
   - Update configuration parsing to ignore `collect_metrics` field (with deprecation warning)
   - Refactor any code that depends on metrics types

4. **Phase 4: Documentation**
   - Create migration guide for existing users
   - Update README to clarify MMM's focus on orchestration
   - Provide examples of using external tools for metrics

### Architecture Changes

**Current Architecture:**
```
MMM
├── Workflow Orchestration
├── AI Integration
├── Metrics Collection (TO BE REMOVED)
│   ├── Test Coverage
│   ├── Lint Warnings
│   ├── Code Complexity
│   └── Compile Metrics
└── Version Control Integration
```

**Target Architecture:**
```
MMM
├── Workflow Orchestration
├── AI Integration
└── Version Control Integration
```

### Data Structures

**To Be Removed:**
- `ProjectMetrics` struct
- `MetricsCoordinator` trait
- `MetricsCollector` struct
- `MetricsCollectorImpl` struct
- All metrics-related enums and types

**To Be Modified:**
- Workflow configuration structs (remove `collect_metrics` field)
- Workflow executor (remove metrics collection calls)

### APIs and Interfaces

**Deprecated:**
- `collect_metrics` workflow configuration option

**Recommended Replacement Pattern:**
```yaml
# Instead of:
collect_metrics: true

# Use:
reduce:
  commands:
    - shell: "cargo tarpaulin --out Json > coverage.json"
    - shell: "debtmap analyze . --output metrics.json"
    - claude: "/analyze-metrics --coverage coverage.json --debt metrics.json"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - Workflow executor
  - Configuration parser
  - MapReduce executor
  - Test suite
- **External Dependencies to Remove**:
  - Any metrics-specific crates that are no longer needed

## Testing Strategy

- **Unit Tests**: Remove all metrics-related unit tests
- **Integration Tests**: Update integration tests to not expect metrics output
- **Compatibility Tests**: Ensure workflows without `collect_metrics` still work
- **Deprecation Tests**: Verify graceful handling of `collect_metrics` in old workflows

## Documentation Requirements

- **Migration Guide**: Document how to replace `collect_metrics` with external tools
- **README Updates**: Clarify MMM's role as an orchestrator
- **Example Updates**: Show best practices for using external analysis tools
- **Architecture Documentation**: Update to reflect simplified architecture

## Implementation Notes

### Migration Path for Users

For users currently using `collect_metrics: true`, provide clear migration examples:

```yaml
# Old approach (deprecated):
workflow:
  name: analyze-and-fix
  collect_metrics: true
  steps:
    - claude: "/fix-issues"

# New approach (recommended):
workflow:
  name: analyze-and-fix
  steps:
    - claude: "/fix-issues"
    - shell: "cargo tarpaulin --out Lcov > coverage.lcov"
    - shell: "debtmap analyze . --lcov coverage.lcov --output debt.json"
    - claude: "/summarize-improvements --coverage coverage.lcov --debt debt.json"
```

### Backward Compatibility

- Add deprecation warning if `collect_metrics` is found in workflow
- Ignore the field but continue workflow execution
- Provide helpful error message suggesting migration approach

### Code Cleanup Opportunities

Removing metrics collection will also allow cleanup of:
- Mock metrics implementations in tests
- Metrics-related error handling
- Metrics storage and history functionality
- Metrics report generation code

## Migration and Compatibility

### Breaking Changes

- Workflows using `collect_metrics: true` will need to be updated
- Any custom code depending on MMM's metrics APIs will need refactoring

### Migration Timeline

1. **Version 0.2.0**: Add deprecation warning for `collect_metrics`
2. **Version 0.3.0**: Remove metrics functionality entirely
3. **Documentation**: Provide migration guide 2 weeks before 0.2.0 release

### Compatibility Considerations

- Ensure the removal doesn't affect core workflow execution
- Maintain ability to run external analysis tools via shell commands
- Preserve all non-metrics-related functionality

## Benefits

### Immediate Benefits
- Simplified codebase
- Reduced binary size
- Faster compilation times
- Clearer architectural boundaries

### Long-term Benefits
- Easier maintenance
- Better separation of concerns
- Focus on core competencies
- Reduced technical debt
- More flexibility for users to choose their own analysis tools

## Risks and Mitigations

### Risk: User Disruption
**Mitigation**: Provide comprehensive migration guide and deprecation period

### Risk: Loss of Integrated Metrics
**Mitigation**: Document best practices for using external tools effectively

### Risk: Workflow Breakage
**Mitigation**: Add compatibility layer to ignore `collect_metrics` with warning

## Success Metrics

- Code reduction: Remove at least 1000 lines of metrics-related code
- Binary size: Reduce by at least 5%
- Build time: Improve by at least 10%
- Zero regression in core workflow functionality
- Successful migration of all example workflows