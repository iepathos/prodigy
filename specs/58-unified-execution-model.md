---
number: 58
title: Unified Execution Model
category: architecture
priority: high
status: draft
dependencies: [57]
created: 2025-09-03
updated: 2025-09-04
---

# Specification 58: Unified Execution Model

**Category**: architecture
**Priority**: high
**Status**: draft
**Dependencies**: [57 - Claude Agent Observability]

## Executive Summary

The current workflow execution system has 4 divergent execution paths in `DefaultCookOrchestrator` that handle the same workflows differently, leading to feature inconsistencies (validation works in only 25% of paths), code duplication, and maintenance complexity. This specification defines a unified execution model that consolidates these paths while preserving all functionality.

## Sub-Specifications

This specification is broken into focused sub-specs:

- **[58a - Workflow Normalization](58a-workflow-normalization.md)**: Pure functional normalization layer that preserves all workflow fields
- **[58b - Unified Variable Substitution](58b-unified-variable-substitution.md)**: Critical standardization of variable naming across all execution modes  
- **[58c - Incremental Migration Plan](58c-incremental-migration-plan.md)**: Detailed week-by-week implementation plan with rollback strategies

## Context

The current implementation has evolved organically into four distinct execution paths:

1. **Standard Workflow Path** (`execute_workflow`) - Full feature support with WorkflowExecutor
2. **Structured Path** (`execute_structured_workflow`) - Loses validation during conversion
3. **Args/Map Path** (`execute_workflow_with_args`) - Loses validation, different variable names
4. **MapReduce Path** (`execute_mapreduce_workflow`) - Separate implementation, missing features

### The Validation Bug

The bug that exposed this issue shows validation configuration being lost when converting between `WorkflowCommand` and `Command` types in 3 of 4 paths:

```rust
// Current problem - loses validation
fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
    let command = cmd.to_command();  // Loses WorkflowStep fields!
    WorkflowStep {
        validate: None,  // LOST!
        ...
    }
}
```

## Objective

Create a unified workflow execution model that:
1. Consolidates all execution modes into a single, consistent pipeline
2. Preserves ALL workflow features across ALL modes (validation, handlers, timeouts)
3. Standardizes variable substitution like Ansible/Terraform
4. Maintains full backward compatibility
5. Follows functional programming principles with pure functions

## Solution Overview

### Three-Layer Architecture

```
┌─────────────────────────────────────────┐
│         Orchestration Layer             │
│  Single path in DefaultCookOrchestrator │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│       Normalization Layer (58a)         │
│  NormalizedWorkflow preserves ALL fields│
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│      Variable Substitution (58b)        │
│  Consistent ${item} syntax everywhere   │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│         Execution Layer                 │
│  Existing executors (unchanged)         │
└─────────────────────────────────────────┘
```

### Key Insights

1. **The executors are fine** - ClaudeExecutor, CommandExecutor, MapReduceExecutor all work correctly
2. **The problem is orchestration** - 4 different paths with different behavior
3. **Information loss during conversion** - Fields dropped when converting between types
4. **Variable inconsistency** - Different names for same data (`$ARG` vs `${item}` vs `${FILE}`)

## Implementation Approach

See [58c - Incremental Migration Plan](58c-incremental-migration-plan.md) for detailed implementation phases. Key principles:

1. **Incremental migration** - One path at a time with feature flags
2. **Test before changing** - Comprehensive baseline tests first
3. **Preserve ALL fields** - Never lose validation, handlers, or other config
4. **Reuse existing executors** - They already work correctly
5. **Pure functional approach** - Immutable data, no side effects, Result types

## Success Metrics

- Validation works in 100% of execution paths (currently 25%)
- All existing workflows continue working unchanged
- ~70% code reduction in orchestrator
- Single variable substitution system
- Single location for each feature implementation

## Risk Mitigation

- Feature flags for gradual rollout
- Comprehensive testing at each phase
- Backward compatibility through aliases
- Clear rollback strategy at each checkpoint

## Dependencies

- **Prerequisites**: 
  - Specification 57: Claude Agent Observability (for progress reporting)
- **Sub-specifications**:
  - 58a: Workflow Normalization
  - 58b: Unified Variable Substitution
  - 58c: Incremental Migration Plan

## Timeline

7 weeks total including buffer - see [58c](58c-incremental-migration-plan.md) for detailed schedule.

## Summary

The unified execution model consolidates 4 divergent orchestration paths into one, ensuring all workflow features work consistently while preserving backward compatibility. By following functional programming principles and implementing incrementally, we fix the validation bug and eliminate significant technical debt without disrupting existing workflows.