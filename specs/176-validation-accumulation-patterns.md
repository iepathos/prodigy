---
number: 176
title: Validation Accumulation Patterns
category: foundation
priority: high
status: draft
dependencies: [172, 174]
created: 2025-11-24
---

# Specification 176: Validation Accumulation Patterns

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation), Spec 174 (Pure Core Extraction)

## Context

Prodigy currently uses fail-fast validation that stops at the first error:

**Current Problems:**
- **Early exit** - Validation stops at first error, hiding other issues
- **Poor user experience** - Users fix one error, run again, see next error (repeat cycle)
- **Manual accumulation** - When accumulation exists, it's implemented manually with Vec<Error>
- **Incomplete feedback** - Cannot see full scope of validation problems
- **Testing difficulty** - Hard to verify all validation paths

**Example of current fail-fast:**
```rust
fn validate_workflow(config: &WorkflowConfig) -> Result<()> {
    if config.commands.is_empty() {
        return Err(WorkflowError::NoCommands); // Stop here!
    }

    if let Some(env) = &config.env {
        validate_env_vars(env)?; // If this fails, never check secrets
    }

    if let Some(secrets) = &config.secrets {
        validate_secrets(secrets)?; // Never reached if above fails
    }

    Ok(())
}
```

This specification covers Phase 5 of the Stillwater migration: replacing manual error accumulation with Stillwater's Validation applicative functor.

## Objective

Implement comprehensive validation accumulation by:
1. **Using Validation<T, Vec<E>>** for all configuration validation
2. **Accumulating ALL errors** before reporting to users
3. **Applying validation to workflows** (config, env, secrets, merge)
4. **Applying validation to work items** before MapReduce execution
5. **Integrating with DLQ** for validation failure reporting
6. **Providing excellent error messages** with full context

## Requirements

### Functional Requirements

#### FR1: Workflow Configuration Validation
- **MUST** validate workflow commands (non-empty, valid syntax)
- **MUST** validate environment variables (all keys present, valid types)
- **MUST** validate secrets configuration (valid secret refs, types match)
- **MUST** validate merge workflow configuration
- **MUST** accumulate ALL validation errors before failing
- **MUST** report errors with full context (line numbers, field names)

#### FR2: Work Item Validation
- **MUST** validate all work items before MapReduce execution
- **MUST** check required fields present
- **MUST** validate field types match expectations
- **MUST** validate field ranges/constraints
- **MUST** accumulate errors across all items
- **MUST** report item index and field path for each error

#### FR3: Validation Composition
- **MUST** use Validation::combine for applicative composition
- **MUST** use traverse for validating collections
- **MUST** support nested validation (validate workflow → validate commands → validate each command)
- **MUST** preserve error context through composition
- **MUST** flatten nested validation results appropriately

#### FR4: DLQ Integration
- **MUST** add validation failures to DLQ
- **MUST** preserve all accumulated errors in DLQ items
- **MUST** include validation context (what was being validated)
- **MUST** enable retry of validation failures after fixes
- **MUST** maintain consistency between validation and DLQ state

#### FR5: User-Facing Error Reporting
- **MUST** display ALL validation errors to user at once
- **MUST** format errors clearly with context
- **MUST** group related errors
- **MUST** suggest fixes where possible
- **MUST** avoid error message duplication

### Non-Functional Requirements

#### NFR1: Performance
- **MUST** validate in parallel where possible
- **MUST** avoid redundant validation passes
- **MUST** have < 10% overhead vs fail-fast for valid inputs
- **MUST** short-circuit expensive validations when cheap ones fail

#### NFR2: Usability
- **MUST** improve error reporting quality dramatically
- **MUST** reduce validation iteration cycles by showing all errors
- **MUST** provide actionable error messages
- **MUST** make error messages beginner-friendly

#### NFR3: Maintainability
- **MUST** make adding new validation rules easy
- **MUST** keep validation logic pure and testable
- **MUST** centralize validation patterns
- **MUST** follow consistent validation structure

## Acceptance Criteria

- [ ] Workflow validation uses Validation applicative
- [ ] All workflow validation errors accumulated
- [ ] Work item validation uses traverse pattern
- [ ] All work item errors accumulated across items
- [ ] Validation failures added to DLQ with full context
- [ ] User sees ALL validation errors in single report
- [ ] Unit tests verify accumulation (3+ errors shown)
- [ ] Property tests verify validation laws (applicative)
- [ ] Integration tests verify DLQ integration
- [ ] Error messages improved (user testing)
- [ ] Performance benchmarks show < 10% overhead
- [ ] Documentation includes validation patterns

## Technical Details

### Implementation Approach

#### 1. Workflow Configuration Validation

```rust
// src/core/workflow/validation.rs

use stillwater::Validation;

/// Validate complete workflow configuration
pub fn validate_workflow(
    config: &WorkflowConfig,
) -> Validation<(), Vec<WorkflowError>> {
    // Applicative composition - accumulates ALL errors
    validate_has_commands(config)
        .combine(validate_env_vars(&config.env))
        .combine(validate_secrets(&config.secrets))
        .combine(validate_merge_workflow(&config.merge))
        .combine(validate_command_syntax(&config.commands))
}

/// Validate workflow has commands
fn validate_has_commands(
    config: &WorkflowConfig,
) -> Validation<(), WorkflowError> {
    if config.commands.is_empty() {
        Validation::failure(WorkflowError::NoCommands)
    } else {
        Validation::success(())
    }
}

/// Validate environment variables
fn validate_env_vars(
    env: &Option<HashMap<String, EnvVar>>,
) -> Validation<(), Vec<WorkflowError>> {
    match env {
        None => Validation::success(()),
        Some(vars) => {
            // Validate each variable, accumulate errors
            let validations: Vec<_> = vars
                .iter()
                .map(|(key, var)| validate_env_var(key, var))
                .collect();

            stillwater::traverse(validations, |v| v)
                .map(|_| ())
        }
    }
}

/// Validate single environment variable
fn validate_env_var(
    key: &str,
    var: &EnvVar,
) -> Validation<(), WorkflowError> {
    // Check key format
    let key_valid = if is_valid_env_key(key) {
        Validation::success(())
    } else {
        Validation::failure(WorkflowError::InvalidEnvKey {
            key: key.to_string(),
        })
    };

    // Check value type
    let value_valid = validate_env_value(key, &var.value);

    // Combine both validations
    key_valid.combine(value_valid)
}

/// Validate environment variable value
fn validate_env_value(
    key: &str,
    value: &Value,
) -> Validation<(), WorkflowError> {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => {
            Validation::success(())
        }
        Value::Object(_) if key.ends_with("_JSON") => {
            Validation::success(())
        }
        _ => Validation::failure(WorkflowError::InvalidEnvValue {
            key: key.to_string(),
            got_type: value.type_name(),
        }),
    }
}

/// Validate secrets configuration
fn validate_secrets(
    secrets: &Option<HashMap<String, SecretConfig>>,
) -> Validation<(), Vec<WorkflowError>> {
    match secrets {
        None => Validation::success(()),
        Some(secrets_map) => {
            let validations: Vec<_> = secrets_map
                .iter()
                .map(|(key, secret)| validate_secret(key, secret))
                .collect();

            stillwater::traverse(validations, |v| v)
                .map(|_| ())
        }
    }
}

/// Validate single secret
fn validate_secret(
    key: &str,
    secret: &SecretConfig,
) -> Validation<(), WorkflowError> {
    // Check secret source exists
    let source_valid = match &secret.source {
        SecretSource::Env(env_var) => {
            if env::var(env_var).is_ok() {
                Validation::success(())
            } else {
                Validation::failure(WorkflowError::SecretNotFound {
                    key: key.to_string(),
                    source: env_var.clone(),
                })
            }
        }
        SecretSource::File(path) => {
            if path.exists() {
                Validation::success(())
            } else {
                Validation::failure(WorkflowError::SecretFileNotFound {
                    key: key.to_string(),
                    path: path.clone(),
                })
            }
        }
    };

    // Check secret is marked as secret
    let secret_flag = if secret.secret {
        Validation::success(())
    } else {
        Validation::failure(WorkflowError::SecretNotMarked {
            key: key.to_string(),
        })
    };

    source_valid.combine(secret_flag)
}

/// Validate merge workflow configuration
fn validate_merge_workflow(
    merge: &Option<MergeWorkflow>,
) -> Validation<(), Vec<WorkflowError>> {
    match merge {
        None => Validation::success(()),
        Some(merge_config) => {
            validate_has_commands_merge(merge_config)
                .combine(validate_timeout(merge_config.timeout))
                .map_err(|e| vec![e])
        }
    }
}
```

#### 2. Work Item Validation

```rust
// src/cook/execution/mapreduce/validation.rs

use stillwater::Validation;

/// Validate all work items before MapReduce execution
pub fn validate_all_work_items(
    items: Vec<Value>,
    schema: &WorkItemSchema,
) -> Validation<Vec<ValidWorkItem>, Vec<WorkItemError>> {
    // Use traverse to validate each item, accumulating errors
    stillwater::traverse(
        items.into_iter().enumerate(),
        |(idx, item)| validate_work_item(idx, &item, schema),
    )
}

/// Validate single work item
fn validate_work_item(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Validation<ValidWorkItem, WorkItemError> {
    // Combine multiple validations for this item
    validate_required_fields(idx, item, schema)
        .combine(validate_field_types(idx, item, schema))
        .combine(validate_field_ranges(idx, item, schema))
        .map(|_| ValidWorkItem {
            index: idx,
            data: item.clone(),
        })
}

/// Validate required fields present
fn validate_required_fields(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Validation<(), WorkItemError> {
    let missing_fields: Vec<_> = schema
        .required_fields
        .iter()
        .filter(|field| !item.get(field).is_some())
        .cloned()
        .collect();

    if missing_fields.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(WorkItemError::MissingFields {
            item_index: idx,
            fields: missing_fields,
        })
    }
}

/// Validate field types match schema
fn validate_field_types(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Validation<(), Vec<WorkItemError>> {
    let type_errors: Vec<_> = schema
        .fields
        .iter()
        .filter_map(|(field_name, field_type)| {
            item.get(field_name).and_then(|value| {
                if matches_type(value, field_type) {
                    None
                } else {
                    Some(WorkItemError::InvalidFieldType {
                        item_index: idx,
                        field: field_name.clone(),
                        expected: field_type.clone(),
                        got: value.type_name(),
                    })
                }
            })
        })
        .collect();

    if type_errors.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(type_errors)
    }
}

/// Validate field ranges/constraints
fn validate_field_ranges(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Validation<(), Vec<WorkItemError>> {
    let range_errors: Vec<_> = schema
        .constraints
        .iter()
        .filter_map(|(field_name, constraint)| {
            item.get(field_name).and_then(|value| {
                if satisfies_constraint(value, constraint) {
                    None
                } else {
                    Some(WorkItemError::ConstraintViolation {
                        item_index: idx,
                        field: field_name.clone(),
                        constraint: constraint.clone(),
                        value: value.clone(),
                    })
                }
            })
        })
        .collect();

    if range_errors.is_empty() {
        Validation::success(())
    } else {
        Validation::failure(range_errors)
    }
}
```

#### 3. Integration with Orchestrator

```rust
// src/cook/orchestrator/core.rs

use crate::core::workflow::validation::*;

impl CookOrchestrator {
    pub async fn run(&self, config: CookConfig) -> Result<ExecutionResult> {
        // Validate workflow configuration upfront
        match validate_workflow(&config.workflow) {
            Validation::Success(_) => {
                // Proceed with execution
                self.execute_workflow(config).await
            }
            Validation::Failure(errors) => {
                // Report ALL errors at once
                eprintln!("Workflow validation failed with {} error(s):\n", errors.len());

                for (i, error) in errors.iter().enumerate() {
                    eprintln!("  {}. {}", i + 1, format_error(error));
                }

                Err(CookError::ValidationFailed {
                    count: errors.len(),
                    errors,
                })
            }
        }
    }
}

// src/cook/execution/mapreduce/phases/map.rs

async fn process_work_items(
    &self,
    items: Vec<Value>,
    schema: &WorkItemSchema,
) -> Result<Vec<AgentResult>, PhaseError> {
    // Validate ALL items first, accumulate errors
    let validated = match validate_all_work_items(items, schema) {
        Validation::Success(valid_items) => valid_items,
        Validation::Failure(errors) => {
            // Log ALL validation errors
            error!("Work item validation failed with {} error(s)", errors.len());

            for error in &errors {
                error!("  - {}", error);
            }

            // Add ALL errors to DLQ
            for error in &errors {
                self.dlq.add_validation_error(error).await?;
            }

            return Err(PhaseError::InvalidWorkItems {
                count: errors.len(),
                errors,
            });
        }
    };

    // Now safe to process validated items
    self.distribute_work(validated).await
}
```

#### 4. Error Formatting

```rust
// src/cook/execution/errors.rs

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkflowError::NoCommands => {
                write!(f, "Workflow must have at least one command")
            }
            WorkflowError::InvalidEnvKey { key } => {
                write!(
                    f,
                    "Invalid environment variable key '{}': must match [A-Z_][A-Z0-9_]*",
                    key
                )
            }
            WorkflowError::InvalidEnvValue { key, got_type } => {
                write!(
                    f,
                    "Invalid environment variable value for '{}': expected string, number, or bool, got {}",
                    key, got_type
                )
            }
            WorkflowError::SecretNotFound { key, source } => {
                write!(
                    f,
                    "Secret '{}' not found: environment variable '{}' is not set",
                    key, source
                )
            }
            WorkflowError::SecretFileNotFound { key, path } => {
                write!(
                    f,
                    "Secret '{}' not found: file '{}' does not exist",
                    key,
                    path.display()
                )
            }
            WorkflowError::SecretNotMarked { key } => {
                write!(
                    f,
                    "Secret '{}' must have 'secret: true' flag to be treated as secret",
                    key
                )
            }
        }
    }
}

impl fmt::Display for WorkItemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkItemError::MissingFields { item_index, fields } => {
                write!(
                    f,
                    "Work item #{}: missing required field(s): {}",
                    item_index,
                    fields.join(", ")
                )
            }
            WorkItemError::InvalidFieldType {
                item_index,
                field,
                expected,
                got,
            } => {
                write!(
                    f,
                    "Work item #{}, field '{}': expected {}, got {}",
                    item_index, field, expected, got
                )
            }
            WorkItemError::ConstraintViolation {
                item_index,
                field,
                constraint,
                value,
            } => {
                write!(
                    f,
                    "Work item #{}, field '{}': value {} violates constraint {}",
                    item_index, field, value, constraint
                )
            }
        }
    }
}
```

### Architecture Changes

**New Modules:**
```
src/core/workflow/
└── validation.rs              # Workflow validation with accumulation

src/cook/execution/mapreduce/
└── validation.rs              # Work item validation with accumulation

src/cook/execution/
└── errors.rs                  # Enhanced error types and formatting
```

**Modified Modules:**
```
src/cook/orchestrator/core.rs  # Use validation before execution
src/cook/execution/mapreduce/phases/map.rs  # Validate work items
src/storage/dlq.rs             # Add validation error methods
```

### Data Structures

**Validation Errors:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowError {
    NoCommands,
    InvalidEnvKey { key: String },
    InvalidEnvValue { key: String, got_type: String },
    SecretNotFound { key: String, source: String },
    SecretFileNotFound { key: String, path: PathBuf },
    SecretNotMarked { key: String },
    InvalidCommandSyntax { command: String, reason: String },
    InvalidTimeout { value: u64, max: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkItemError {
    MissingFields { item_index: usize, fields: Vec<String> },
    InvalidFieldType {
        item_index: usize,
        field: String,
        expected: String,
        got: String,
    },
    ConstraintViolation {
        item_index: usize,
        field: String,
        constraint: String,
        value: Value,
    },
}
```

**Work Item Schema:**
```rust
#[derive(Debug, Clone)]
pub struct WorkItemSchema {
    pub required_fields: Vec<String>,
    pub fields: HashMap<String, FieldType>,
    pub constraints: HashMap<String, Constraint>,
}

#[derive(Debug, Clone)]
pub enum FieldType {
    String,
    Number,
    Bool,
    Array(Box<FieldType>),
    Object,
}

#[derive(Debug, Clone)]
pub enum Constraint {
    Range { min: f64, max: f64 },
    MinLength(usize),
    MaxLength(usize),
    Pattern(Regex),
    OneOf(Vec<Value>),
}
```

### APIs and Interfaces

**Validation API:**
```rust
// Workflow validation
pub fn validate_workflow(config: &WorkflowConfig) -> Validation<(), Vec<WorkflowError>>;
pub fn validate_env_vars(env: &Option<HashMap<String, EnvVar>>) -> Validation<(), Vec<WorkflowError>>;
pub fn validate_secrets(secrets: &Option<HashMap<String, SecretConfig>>) -> Validation<(), Vec<WorkflowError>>;

// Work item validation
pub fn validate_all_work_items(items: Vec<Value>, schema: &WorkItemSchema) -> Validation<Vec<ValidWorkItem>, Vec<WorkItemError>>;
pub fn validate_work_item(idx: usize, item: &Value, schema: &WorkItemSchema) -> Validation<ValidWorkItem, WorkItemError>;

// DLQ integration
pub async fn add_validation_errors(&self, errors: &[WorkItemError]) -> Result<()>;
```

## Dependencies

### Prerequisites
- **Spec 172** completed (Stillwater foundation, Validation available)
- **Spec 174** completed (Pure core extraction)
- Stillwater Validation and traverse patterns available

### Affected Components
- Workflow configuration parsing
- MapReduce work item processing
- DLQ error reporting
- User-facing error messages
- All validation tests

### External Dependencies
- `stillwater = "0.2.0"` (Validation, traverse)

## Testing Strategy

### Unit Tests

**Validation Accumulation:**
```rust
#[test]
fn test_workflow_validation_accumulates_errors() {
    let config = WorkflowConfig {
        commands: vec![], // Error 1: no commands
        env: Some(
            [
                ("123invalid".to_string(), EnvVar::simple("value")), // Error 2: invalid key
                ("VALID".to_string(), EnvVar::simple(json!({}))),    // Error 3: invalid value
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        secrets: Some(
            [("API_KEY".to_string(), SecretConfig {
                source: SecretSource::Env("MISSING_VAR".into()),
                secret: false, // Error 4: not marked secret
            })]
            .iter()
            .cloned()
            .collect(),
        ),
        ..Default::default()
    };

    let result = validate_workflow(&config);

    match result {
        Validation::Failure(errors) => {
            // ALL 4 errors should be reported!
            assert_eq!(errors.len(), 4);

            assert!(errors.iter().any(|e| matches!(e, WorkflowError::NoCommands)));
            assert!(errors.iter().any(|e| matches!(e, WorkflowError::InvalidEnvKey { .. })));
            assert!(errors.iter().any(|e| matches!(e, WorkflowError::InvalidEnvValue { .. })));
            assert!(errors.iter().any(|e| matches!(e, WorkflowError::SecretNotMarked { .. })));
        }
        _ => panic!("Expected validation failure with 4 errors"),
    }
}

#[test]
fn test_work_item_validation_accumulates_errors() {
    let schema = WorkItemSchema {
        required_fields: vec!["id".into(), "name".into()],
        fields: [
            ("id".into(), FieldType::Number),
            ("name".into(), FieldType::String),
        ]
        .iter()
        .cloned()
        .collect(),
        constraints: [
            ("id".into(), Constraint::Range { min: 1.0, max: 1000.0 }),
        ]
        .iter()
        .cloned()
        .collect(),
    };

    let items = vec![
        json!({"id": "not_a_number", "name": "test"}), // Type error
        json!({"id": 5000}),                            // Missing name, range violation
        json!({"name": "valid"}),                       // Missing id
    ];

    let result = validate_all_work_items(items, &schema);

    match result {
        Validation::Failure(errors) => {
            // Should have errors from all 3 items
            assert!(errors.len() >= 3);

            // Check specific errors present
            assert!(errors.iter().any(|e| matches!(e,
                WorkItemError::InvalidFieldType { item_index: 0, .. })));
            assert!(errors.iter().any(|e| matches!(e,
                WorkItemError::ConstraintViolation { item_index: 1, .. })));
            assert!(errors.iter().any(|e| matches!(e,
                WorkItemError::MissingFields { item_index: 2, .. })));
        }
        _ => panic!("Expected validation failure"),
    }
}
```

### Property Tests

**Validation Laws:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_validation_success_is_identity(items in prop::collection::vec(valid_work_item(), 1..20)) {
        let schema = simple_schema();

        let result = validate_all_work_items(items.clone(), &schema);

        // Valid items should always succeed
        assert!(matches!(result, Validation::Success(_)));

        // Validated items should equal input
        if let Validation::Success(validated) = result {
            prop_assert_eq!(
                validated.iter().map(|v| &v.data).collect::<Vec<_>>(),
                items.iter().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn prop_validation_accumulates_all_errors(
        invalid_items in prop::collection::vec(invalid_work_item(), 1..20)
    ) {
        let schema = simple_schema();

        let result = validate_all_work_items(invalid_items.clone(), &schema);

        match result {
            Validation::Failure(errors) => {
                // Should have at least one error per item
                prop_assert!(errors.len() >= invalid_items.len());
            }
            _ => prop_assert!(false, "Expected validation to fail"),
        }
    }
}
```

### Integration Tests

**DLQ Integration:**
```rust
#[tokio::test]
async fn test_validation_errors_added_to_dlq() {
    let workflow = create_test_workflow_with_invalid_items();

    let result = execute_mapreduce_workflow(workflow).await;

    assert!(result.is_err());

    // Check DLQ contains all validation errors
    let dlq_items = get_dlq_items(&workflow.job_id).await;

    assert!(!dlq_items.is_empty());

    // Verify ALL validation errors in DLQ
    let validation_errors: Vec<_> = dlq_items
        .iter()
        .flat_map(|item| &item.failure_history)
        .filter(|f| matches!(f.reason, FailureReason::ValidationError(_)))
        .collect();

    assert_eq!(validation_errors.len(), 3); // All 3 invalid items
}
```

## Documentation Requirements

### Code Documentation
- Document Validation patterns
- Provide examples of accumulation
- Show traverse usage for collections
- Explain applicative composition

### User Documentation

**Update CLAUDE.md:**
- Add "Validation Patterns" section
- Show validation error accumulation benefits
- Provide workflow validation examples
- Document DLQ integration

### Architecture Updates

**Update ARCHITECTURE.md:**
- Add validation architecture section
- Document validation boundaries
- Show error accumulation flow
- Explain DLQ integration

## Implementation Notes

### Critical Success Factors
1. **ALL errors reported** - No early exit
2. **Clear error messages** - User-friendly formatting
3. **DLQ integration** - Validation failures tracked
4. **Testing coverage** - Verify accumulation works

### Migration Path
1. Create validation modules with Validation types
2. Implement workflow validation
3. Implement work item validation
4. Integrate with orchestrator
5. Update DLQ for validation errors
6. Enhance error formatting
7. Update all tests
8. Document patterns

## Migration and Compatibility

### Breaking Changes
- **None** - Enhanced validation only
- Existing workflows work, get better errors

### Backward Compatibility
- All workflows valid before remain valid
- Invalid workflows get better error messages
- DLQ format extended (backward compatible)

### Rollback Strategy
If issues arise:
1. Revert to fail-fast validation
2. Remove Validation types
3. Restore original error handling

**Rollback impact:** Lose comprehensive error reporting, return to fail-fast.
