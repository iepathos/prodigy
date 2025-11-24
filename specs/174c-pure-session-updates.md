---
number: 174c
title: Pure Session Updates
category: foundation
priority: high
status: draft
dependencies: [172, 173]
parent: 174
created: 2025-11-24
---

# Specification 174c: Pure Session Updates

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation), Spec 173 (Parallel Execution Effects)
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the third phase of Spec 174 (Pure Core Extraction). Session management currently uses mutable updates scattered across the codebase. This spec creates immutable session transformation functions.

**Scope**: Create pure session update functions only. No session manager refactoring yet (that's in 174d for effects).

## Objective

Extract session update logic into pure, testable functions:
- Immutable session transformations
- State transition validation
- Progress updates
- Variable merging
- Step tracking

## Requirements

### Functional Requirements

#### FR1: Session Update Types
- **MUST** create `SessionUpdate` enum with variants:
  - `Status(SessionStatus)`
  - `Progress(ProgressUpdate)`
  - `Variables(HashMap<String, Value>)`
  - `AddStep(StepRecord)`

#### FR2: Core Update Function
- **MUST** implement `apply_session_update(session: UnifiedSession, update: SessionUpdate) -> Result<UnifiedSession, SessionError>`
- **MUST** return new session (immutable)
- **MUST** update `updated_at` timestamp
- **MUST** preserve all other fields
- **MUST** validate updates before applying

#### FR3: Status Transitions
- **MUST** implement `apply_status_update(session: UnifiedSession, status: SessionStatus) -> Result<UnifiedSession, SessionError>`
- **MUST** validate state transitions
- **MUST** allow: Created→Running, Running→Paused, Running→Completed, Running→Failed, Paused→Running, Paused→Cancelled
- **MUST** reject invalid transitions
- **MUST** return `SessionError::InvalidTransition` for invalid transitions

#### FR4: Progress Updates
- **MUST** implement `apply_progress_update(session: UnifiedSession, progress: ProgressUpdate) -> Result<UnifiedSession, SessionError>`
- **MUST** increment completed_steps counter
- **MUST** increment failed_steps counter
- **MUST** update current_step field
- **MUST** validate non-negative values

#### FR5: Variable Updates
- **MUST** implement `apply_variable_update(session: UnifiedSession, vars: HashMap<String, Value>) -> Result<UnifiedSession, SessionError>`
- **MUST** merge new variables with existing
- **MUST** overwrite existing values with same key
- **MUST** preserve existing variables not in update

#### FR6: Step Tracking
- **MUST** implement `apply_add_step(session: UnifiedSession, step: StepRecord) -> Result<UnifiedSession, SessionError>`
- **MUST** append step to execution history
- **MUST** preserve chronological order

### Non-Functional Requirements

#### NFR1: Purity
- **MUST** have zero I/O operations
- **MUST** be deterministic
- **MUST** have no side effects
- **MUST** pass clippy with no warnings

#### NFR2: Testability
- **MUST** achieve 100% test coverage
- **MUST** require zero mocking in tests
- **MUST** have fast tests (< 1ms per test)

## Acceptance Criteria

- [ ] Module created at `src/core/session/`
- [ ] `updates.rs` with all update functions
- [ ] `validation.rs` with state transition validation
- [ ] `SessionUpdate` enum defined
- [ ] Unit tests achieve 100% coverage
- [ ] No mocking used in any test
- [ ] All tests pass in < 50ms total
- [ ] `cargo fmt` and `cargo clippy` pass with no warnings
- [ ] Module properly exposed in `src/core/mod.rs`

## Technical Details

### Module Structure

```
src/core/session/
├── mod.rs          # Module exports
├── updates.rs      # Pure session transformations
└── validation.rs   # State transition validation
```

### Session Updates

```rust
// src/core/session/updates.rs

use crate::unified_session::{UnifiedSession, SessionStatus, SessionError};
use serde_json::Value;
use std::collections::HashMap;
use chrono::Utc;
use super::validation::validate_status_transition;

#[derive(Debug, Clone)]
pub enum SessionUpdate {
    Status(SessionStatus),
    Progress(ProgressUpdate),
    Variables(HashMap<String, Value>),
    AddStep(StepRecord),
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub current_step: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StepRecord {
    pub command: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
}

/// Pure: Apply session update
pub fn apply_session_update(
    session: UnifiedSession,
    update: SessionUpdate,
) -> Result<UnifiedSession, SessionError> {
    let updated = UnifiedSession {
        updated_at: Utc::now(),
        ..session
    };

    match update {
        SessionUpdate::Status(status) => apply_status_update(updated, status),
        SessionUpdate::Progress(progress) => apply_progress_update(updated, progress),
        SessionUpdate::Variables(vars) => apply_variable_update(updated, vars),
        SessionUpdate::AddStep(step) => apply_add_step(updated, step),
    }
}

/// Pure: Apply status update with validation
fn apply_status_update(
    session: UnifiedSession,
    status: SessionStatus,
) -> Result<UnifiedSession, SessionError> {
    // Validate state transition
    validate_status_transition(&session.status, &status)?;

    Ok(UnifiedSession {
        status,
        ..session
    })
}

/// Pure: Apply progress update
fn apply_progress_update(
    session: UnifiedSession,
    progress: ProgressUpdate,
) -> Result<UnifiedSession, SessionError> {
    let mut new_progress = session.progress.clone();

    new_progress.completed_steps += progress.completed_steps;
    new_progress.failed_steps += progress.failed_steps;
    new_progress.current_step = progress.current_step;

    Ok(UnifiedSession {
        progress: new_progress,
        ..session
    })
}

/// Pure: Apply variable update (merge)
fn apply_variable_update(
    session: UnifiedSession,
    new_vars: HashMap<String, Value>,
) -> Result<UnifiedSession, SessionError> {
    let mut variables = session.variables.clone();
    variables.extend(new_vars);

    Ok(UnifiedSession {
        variables,
        ..session
    })
}

/// Pure: Add step to execution history
fn apply_add_step(
    session: UnifiedSession,
    step: StepRecord,
) -> Result<UnifiedSession, SessionError> {
    let mut steps = session.steps.clone();
    steps.push(step);

    Ok(UnifiedSession {
        steps,
        ..session
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_status_update_valid() {
        let session = UnifiedSession {
            status: SessionStatus::Running,
            ..Default::default()
        };

        let result = apply_session_update(
            session,
            SessionUpdate::Status(SessionStatus::Completed),
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, SessionStatus::Completed);
    }

    #[test]
    fn test_apply_status_update_invalid() {
        let session = UnifiedSession {
            status: SessionStatus::Completed,
            ..Default::default()
        };

        let result = apply_session_update(
            session,
            SessionUpdate::Status(SessionStatus::Running),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_apply_progress_update() {
        let session = UnifiedSession {
            progress: Progress {
                completed_steps: 5,
                failed_steps: 1,
                current_step: None,
            },
            ..Default::default()
        };

        let result = apply_session_update(
            session,
            SessionUpdate::Progress(ProgressUpdate {
                completed_steps: 3,
                failed_steps: 1,
                current_step: Some("step-4".into()),
            }),
        );

        assert!(result.is_ok());
        let progress = result.unwrap().progress;
        assert_eq!(progress.completed_steps, 8); // 5 + 3
        assert_eq!(progress.failed_steps, 2); // 1 + 1
        assert_eq!(progress.current_step, Some("step-4".into()));
    }

    #[test]
    fn test_apply_variable_update_merges() {
        let mut existing_vars = HashMap::new();
        existing_vars.insert("old".into(), Value::String("value".into()));

        let session = UnifiedSession {
            variables: existing_vars,
            ..Default::default()
        };

        let mut new_vars = HashMap::new();
        new_vars.insert("new".into(), Value::String("value2".into()));

        let result = apply_session_update(
            session,
            SessionUpdate::Variables(new_vars),
        );

        assert!(result.is_ok());
        let variables = result.unwrap().variables;
        assert_eq!(variables.len(), 2);
        assert!(variables.contains_key("old"));
        assert!(variables.contains_key("new"));
    }

    #[test]
    fn test_apply_variable_update_overwrites() {
        let mut existing_vars = HashMap::new();
        existing_vars.insert("key".into(), Value::String("old".into()));

        let session = UnifiedSession {
            variables: existing_vars,
            ..Default::default()
        };

        let mut new_vars = HashMap::new();
        new_vars.insert("key".into(), Value::String("new".into()));

        let result = apply_session_update(
            session,
            SessionUpdate::Variables(new_vars),
        );

        assert!(result.is_ok());
        let variables = result.unwrap().variables;
        assert_eq!(variables.get("key").unwrap(), &Value::String("new".into()));
    }

    #[test]
    fn test_apply_add_step() {
        let session = UnifiedSession {
            steps: vec![],
            ..Default::default()
        };

        let step = StepRecord {
            command: "test".into(),
            started_at: Utc::now(),
            completed_at: None,
            status: "running".into(),
        };

        let result = apply_session_update(
            session,
            SessionUpdate::AddStep(step.clone()),
        );

        assert!(result.is_ok());
        let steps = result.unwrap().steps;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].command, "test");
    }
}
```

### State Transition Validation

```rust
// src/core/session/validation.rs

use crate::unified_session::{SessionStatus, SessionError};

/// Pure: Validate status transition
pub fn validate_status_transition(
    from: &SessionStatus,
    to: &SessionStatus,
) -> Result<(), SessionError> {
    use SessionStatus::*;

    let valid = matches!(
        (from, to),
        (Created, Running)
            | (Running, Paused)
            | (Running, Completed)
            | (Running, Failed)
            | (Paused, Running)
            | (Paused, Cancelled)
    );

    if valid {
        Ok(())
    } else {
        Err(SessionError::InvalidTransition {
            from: from.clone(),
            to: to.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        use SessionStatus::*;

        let valid_transitions = vec![
            (Created, Running),
            (Running, Paused),
            (Running, Completed),
            (Running, Failed),
            (Paused, Running),
            (Paused, Cancelled),
        ];

        for (from, to) in valid_transitions {
            assert!(validate_status_transition(&from, &to).is_ok(),
                "Transition {:?} -> {:?} should be valid", from, to);
        }
    }

    #[test]
    fn test_invalid_transitions() {
        use SessionStatus::*;

        let invalid_transitions = vec![
            (Completed, Running),
            (Failed, Running),
            (Cancelled, Running),
            (Created, Completed),
            (Created, Failed),
        ];

        for (from, to) in invalid_transitions {
            assert!(validate_status_transition(&from, &to).is_err(),
                "Transition {:?} -> {:?} should be invalid", from, to);
        }
    }
}
```

## Testing Strategy

### Unit Tests (No Mocking!)
- Test all update types
- Test valid state transitions
- Test invalid state transitions
- Test variable merging
- Test variable overwriting
- Test progress accumulation
- Test step appending

### Property Tests
```rust
use proptest::prelude::*;

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
```

## Implementation Notes

### Critical Success Factors
1. **Immutability** - Never mutate input session
2. **Validation** - Check all transitions
3. **100% coverage** - Every transition tested
4. **Fast tests** - < 1ms per test

### Integration with Existing Code
- Module should compile independently
- Will be wrapped by effects in spec 174d
- Session manager will use these functions

### Migration Path
1. Create module structure
2. Define SessionUpdate enum
3. Implement core apply function
4. Implement specific update functions
5. Implement validation
6. Write comprehensive unit tests
7. Add property tests
8. Commit and close spec

## Dependencies

### Prerequisites
- Spec 172 (Stillwater Foundation) - for Effect types
- Spec 173 (Parallel Execution Effects) - for patterns

### Blocks
- Spec 174d (Effect Modules) - needs these updates

### Parallel Work
- Can be developed in parallel with 174a
- Can be developed in parallel with 174b

## Documentation Requirements

- Module-level documentation on immutable updates
- Function documentation with state transition diagrams
- Test documentation showing validation
- Update `src/core/mod.rs` to expose new module

## Success Metrics

- [ ] All 9 acceptance criteria met
- [ ] 100% test coverage achieved
- [ ] All tests pass in < 50ms
- [ ] Zero clippy warnings
- [ ] Module successfully imports (compile check)
