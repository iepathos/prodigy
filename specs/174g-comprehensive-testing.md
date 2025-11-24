---
number: 174g
title: Comprehensive Testing
category: foundation
priority: high
status: draft
dependencies: [174a, 174b, 174c, 174d, 174e, 174f]
parent: 174
created: 2025-11-24
---

# Specification 174g: Comprehensive Testing

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 174a-f (all previous phases)
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the seventh phase of Spec 174. Now that all pure modules and refactorings are complete, we add comprehensive testing including property tests, integration tests, and coverage verification.

## Objective

Achieve complete test coverage with:
- 100% coverage on all pure functions
- Property tests for determinism and laws
- Integration tests for effect composition
- Zero mocking for pure function tests

## Requirements

### Functional Requirements

#### FR1: Pure Function Coverage
- **MUST** achieve 100% unit test coverage on all pure modules:
  - `src/core/orchestration/` (174a)
  - `src/cook/workflow/pure/` (174b)
  - `src/core/session/` (174c)
- **MUST** require zero mocking in pure function tests
- **MUST** have tests run in < 1ms per test

#### FR2: Property Tests
- **MUST** add property tests for determinism (same input → same output)
- **MUST** add property tests for idempotence where applicable
- **MUST** add property tests for semigroup laws where applicable
- **MUST** use `proptest` crate

#### FR3: Integration Tests
- **MUST** add integration tests for effect composition
- **MUST** add integration tests for orchestrator flow
- **MUST** add integration tests for workflow execution
- **MUST** use mock environments where appropriate

#### FR4: Test Performance
- **MUST** have pure function tests complete in < 100ms total
- **MUST** have integration tests complete in < 5s total
- **MUST** have property tests complete in < 10s total

### Non-Functional Requirements

#### NFR1: Coverage
- **MUST** achieve 100% coverage on pure modules
- **MUST** achieve > 90% coverage on effect modules
- **MUST** use `cargo tarpaulin` to measure

#### NFR2: Maintainability
- **MUST** have clear test names describing scenarios
- **MUST** have one assertion per test where possible
- **MUST** follow existing test patterns

## Acceptance Criteria

- [ ] 100% unit test coverage on pure modules
- [ ] Zero mocking required for pure function tests
- [ ] Property tests added for all key functions
- [ ] Integration tests for effect composition
- [ ] All tests pass
- [ ] Pure tests < 100ms total
- [ ] Integration tests < 5s total
- [ ] Property tests < 10s total
- [ ] Coverage report shows 100% on pure modules
- [ ] `cargo test` passes with no warnings

## Technical Details

### Property Tests

```rust
// tests/property_tests.rs

use proptest::prelude::*;

mod execution_planning {
    use super::*;
    use prodigy::core::orchestration::execution_planning::*;

    proptest! {
        #[test]
        fn prop_planning_is_deterministic(
            max_parallel in 1usize..100,
            dry_run: bool,
        ) {
            let config = create_config(max_parallel, dry_run);

            let plan1 = plan_execution(&config);
            let plan2 = plan_execution(&config);

            prop_assert_eq!(plan1, plan2);
        }
    }
}

mod variable_expansion {
    use super::*;
    use prodigy::cook::workflow::pure::variable_expansion::*;

    proptest! {
        #[test]
        fn prop_expansion_is_deterministic(
            template in ".*",
            vars in prop::collection::hash_map(".*", ".*", 0..10),
        ) {
            let result1 = expand_variables(&template, &vars);
            let result2 = expand_variables(&template, &vars);

            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_expansion_is_idempotent(
            template in ".*",
            vars in prop::collection::hash_map(".*", ".*", 0..10),
        ) {
            let result1 = expand_variables(&template, &vars);
            let result2 = expand_variables(&result1, &vars);

            // Idempotent after first expansion
            prop_assert_eq!(result1, result2);
        }
    }
}

mod session_updates {
    use super::*;
    use prodigy::core::session::updates::*;

    proptest! {
        #[test]
        fn prop_updates_preserve_immutability(
            completed in 0usize..100,
            failed in 0usize..100,
        ) {
            let session = UnifiedSession::default();
            let original_id = session.id.clone();

            let update = SessionUpdate::Progress(ProgressUpdate {
                completed_steps: completed,
                failed_steps: failed,
                current_step: None,
            });

            let result = apply_session_update(session.clone(), update);

            // Original unchanged
            prop_assert_eq!(session.progress.completed_steps, 0);

            // Result has update
            if let Ok(updated) = result {
                prop_assert_eq!(updated.id, original_id);
                prop_assert_eq!(updated.progress.completed_steps, completed);
            }
        }
    }
}
```

### Integration Tests

```rust
// tests/integration_tests.rs

#[tokio::test]
async fn test_orchestrator_with_pure_planning() {
    let config = load_test_config("standard_workflow.yml");

    // Pure planning
    let plan = plan_execution(&config);
    assert_eq!(plan.mode, ExecutionMode::Standard);

    // Effect execution
    let orchestrator = create_test_orchestrator();
    let result = orchestrator.run(config).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_workflow_execution_with_effects() {
    let mock_env = MockWorkflowEnv {
        claude_runner: MockClaudeRunner::with_output("/test", "success"),
        output_patterns: vec![],
    };

    let executor = WorkflowExecutor::new(mock_env);
    let workflow = create_test_workflow();

    let result = executor.execute_workflow(&workflow).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().outputs.len(), 3);
}

#[tokio::test]
async fn test_session_update_effect_composition() {
    let mock_storage = MockSessionStorage::default();
    let env = SessionEnv { storage: Box::new(mock_storage) };

    let session_id = SessionId::new();
    let updates = vec![
        SessionUpdate::Status(SessionStatus::Running),
        SessionUpdate::Progress(ProgressUpdate {
            completed_steps: 1,
            failed_steps: 0,
            current_step: Some("step-1".into()),
        }),
    ];

    let effect = batch_update_session_effect(session_id, updates);
    let result = effect.run_async(&env).await;

    assert!(result.is_ok());
    let session = result.unwrap();
    assert_eq!(session.status, SessionStatus::Running);
    assert_eq!(session.progress.completed_steps, 1);
}
```

### Coverage Measurement

```bash
# Run tests with coverage
cargo tarpaulin --workspace --out Html --output-dir coverage

# Verify pure modules have 100% coverage
cargo tarpaulin --packages prodigy \
  --include-tests \
  --out Stdout \
  | grep "src/core/orchestration"

# Check for coverage gaps
cargo tarpaulin --ignore-tests | grep -v "100.00%"
```

## Testing Strategy

### Unit Tests
- Test every pure function
- Test all edge cases
- Test error conditions
- No mocking required

### Property Tests
- Determinism for all pure functions
- Idempotence where applicable
- Semigroup laws for aggregations

### Integration Tests
- Effect composition patterns
- Orchestrator flows
- Workflow execution
- Error propagation

## Implementation Notes

### Migration Path
1. Run coverage analysis to identify gaps
2. Add missing unit tests
3. Add property tests for key functions
4. Add integration tests for flows
5. Verify coverage targets met
6. Commit

### Critical Success Factors
1. **100% pure coverage** - No gaps in pure modules
2. **Fast tests** - Pure tests < 100ms
3. **Property tests** - Cover key properties
4. **Integration tests** - Cover happy and error paths

## Dependencies

### Prerequisites
- **174a-f** - All previous phases (needed for testing)

### Blocks
- **174h** - Documentation (final phase)

## Success Metrics

- [ ] All 10 acceptance criteria met
- [ ] 100% coverage on pure modules
- [ ] All performance targets met
- [ ] Zero test failures
