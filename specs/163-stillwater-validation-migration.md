---
number: 163
title: Stillwater Validation Migration
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-11-22
---

# Specification 163: Stillwater Validation Migration

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently implements manual error accumulation across numerous validation functions using `Vec<String>` or custom `ValidationResult` structs. This approach has several limitations:

1. **Boilerplate-heavy**: Each validation function manually constructs error vectors and merges them
2. **Inconsistent patterns**: Different modules use different accumulation strategies
3. **Limited composability**: Difficult to combine multiple validators without custom logic
4. **Sequential reporting**: Users often see only the first error, requiring multiple fix cycles
5. **Testing complexity**: Manual accumulation logic needs its own tests

The stillwater library provides `Validation<T, E>` type that accumulates all errors instead of short-circuiting. This is a perfect fit for prodigy's validation-heavy architecture, particularly in:

- `src/core/validation/mod.rs` - 15+ validation functions with manual error accumulation
- `src/config/command_validator.rs` - Sequential validation with early returns
- `src/cli/validation.rs` - Simple validation without accumulation
- `src/worktree/manager_validation.rs` - Custom validation with inline context

## Objective

Migrate prodigy's validation functions from manual error accumulation to stillwater's `Validation<T, E>` type, improving code quality, reducing boilerplate, and providing users with complete error reports on first validation attempt.

## Requirements

### Functional Requirements

- **FR1**: Add stillwater as a dependency with appropriate feature flags
- **FR2**: Migrate all validation functions in `core/validation/mod.rs` to use `Validation<T, E>`
- **FR3**: Create composable validators using `.zip()` and `.all()` combinators
- **FR4**: Preserve existing validation logic behavior (same errors, same conditions)
- **FR5**: Maintain backward compatibility with existing error types
- **FR6**: Enable accumulation of all validation errors before returning
- **FR7**: Support both single-error and multi-error validation contexts

### Non-Functional Requirements

- **NFR1**: Zero performance regression in validation hot paths
- **NFR2**: Maintain or improve error message clarity
- **NFR3**: Reduce validation code size by at least 20%
- **NFR4**: All existing tests pass without modification
- **NFR5**: Documentation for new validation patterns
- **NFR6**: Ensure stillwater abstractions compile to same machine code as manual accumulation

## Acceptance Criteria

- [ ] Stillwater added to Cargo.toml with version `0.1`
- [ ] `validate_paths()` migrated to return `Validation<Vec<PathBuf>, Vec<ValidationError>>`
- [ ] `validate_environment()` migrated to accumulate missing env var errors
- [ ] `validate_command()` migrated to accumulate dangerous pattern errors
- [ ] `validate_resource_limits()` migrated to validate all fields before failing
- [ ] `validate_json_schema()` migrated to accumulate missing field errors
- [ ] `ValidationResult` struct refactored to use `Validation` internally
- [ ] All 325 lines of validation code in `core/validation/mod.rs` reviewed and simplified
- [ ] Proof-of-concept migration document created showing before/after comparison
- [ ] Unit tests updated to validate multi-error accumulation behavior
- [ ] Integration tests confirm same validation logic with better error reporting
- [ ] Benchmark showing validation performance is equivalent or better
- [ ] Documentation added explaining validation patterns and composition
- [ ] CLI commands show all validation errors on first attempt (not just first error)

## Technical Details

### Implementation Approach

**Phase 1: Dependency Setup** (1 day)
1. Add `stillwater = "0.1"` to Cargo.toml dependencies
2. Create `src/validation/stillwater_compat.rs` module for compatibility types
3. Define `ValidationError` type that works with stillwater
4. Create conversion functions between existing errors and new types

**Phase 2: Core Validation Migration** (3 days)
1. Start with `validate_paths()` as proof of concept:
   ```rust
   // Before
   fn validate_paths(paths: &[PathBuf]) -> ValidationResult {
       let mut errors = Vec::new();
       for path in paths {
           if !path.exists() {
               errors.push(format!("Path not found: {}", path.display()));
           }
       }
       if !errors.is_empty() {
           ValidationResult::with_errors(errors)
       } else {
           ValidationResult::ok()
       }
   }

   // After
   fn validate_paths(paths: &[PathBuf]) -> Validation<Vec<PathBuf>, Vec<ValidationError>> {
       paths.iter()
           .map(|p| validate_path_exists(p))
           .collect::<Validation<Vec<_>, _>>()
   }

   fn validate_path_exists(path: &Path) -> Validation<PathBuf, ValidationError> {
       if path.exists() {
           Validation::ok(path.to_path_buf())
       } else {
           Validation::fail(ValidationError::PathNotFound(path.to_path_buf()))
       }
   }
   ```

2. Migrate `validate_environment()`:
   ```rust
   fn validate_environment(vars: &[String]) -> Validation<HashMap<String, String>, Vec<ValidationError>> {
       vars.iter()
           .map(|var| validate_env_var(var))
           .collect::<Validation<Vec<_>, _>>()
           .map(|pairs| pairs.into_iter().collect())
   }
   ```

3. Migrate `validate_command()` to accumulate all dangerous patterns:
   ```rust
   fn validate_command(cmd: &str) -> Validation<ValidatedCommand, Vec<ValidationError>> {
       Validation::all((
           check_no_shell_injection(cmd),
           check_no_file_overwrite(cmd),
           check_no_rm_rf(cmd),
           check_no_network_access(cmd),
       ))
       .map(|_| ValidatedCommand(cmd.to_string()))
   }
   ```

4. Migrate `validate_resource_limits()` to validate all fields in parallel:
   ```rust
   fn validate_resource_limits(limits: &ResourceLimits) -> Validation<ValidatedLimits, Vec<ValidationError>> {
       Validation::all((
           validate_memory_limit(limits.memory),
           validate_cpu_limit(limits.cpu),
           validate_timeout(limits.timeout),
           validate_disk_limit(limits.disk),
       ))
       .map(|(mem, cpu, timeout, disk)| ValidatedLimits { mem, cpu, timeout, disk })
   }
   ```

**Phase 3: Combinator Composition** (2 days)
1. Create high-level validators that compose smaller validators:
   ```rust
   fn validate_config(config: &Config) -> Validation<ValidConfig, Vec<ValidationError>> {
       Validation::all((
           validate_paths(&config.paths),
           validate_environment(&config.env),
           validate_commands(&config.commands),
           validate_resources(&config.resources),
       ))
       .map(|(paths, env, cmds, res)| ValidConfig { paths, env, cmds, res })
   }
   ```

2. Enable partial validation with warnings:
   ```rust
   // Use Validation::warn() for non-critical issues
   fn validate_with_warnings(input: &Input) -> Validation<ValidInput, Vec<Warning>> {
       Validation::all((
           validate_required_fields(input),
           check_deprecated_fields(input).map_err(Warning::from),
           check_performance_hints(input).map_err(Warning::from),
       ))
   }
   ```

**Phase 4: Integration** (2 days)
1. Update `ValidationResult` to use `Validation` internally
2. Provide compatibility layer for existing callers
3. Update CLI error reporting to show all accumulated errors
4. Update tests to verify multi-error accumulation

### Architecture Changes

**New Module Structure**:
```
src/
├── validation/
│   ├── mod.rs              # Re-exports and high-level validators
│   ├── stillwater_compat.rs # Compatibility types and conversions
│   ├── path.rs             # Path validation using Validation
│   ├── environment.rs      # Environment validation
│   ├── command.rs          # Command safety validation
│   └── resource.rs         # Resource limit validation
└── core/
    └── validation/
        └── mod.rs          # Legacy wrapper (deprecate over time)
```

**Error Type Hierarchy**:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    PathNotFound(PathBuf),
    PathNotReadable(PathBuf),
    EnvVarMissing(String),
    CommandDangerous { cmd: String, reason: String },
    ResourceLimitExceeded { resource: String, limit: usize, requested: usize },
    JsonSchemaFieldMissing(String),
    IterationCountInvalid { count: usize, min: usize, max: usize },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathNotFound(p) => write!(f, "Path not found: {}", p.display()),
            Self::EnvVarMissing(v) => write!(f, "Environment variable missing: {}", v),
            // ... other variants
        }
    }
}
```

### Data Structures

**Validation Result Wrapper**:
```rust
/// Compatibility wrapper that preserves existing ValidationResult API
pub struct ValidationResult {
    inner: Validation<(), Vec<ValidationError>>,
    warnings: Vec<String>,
}

impl ValidationResult {
    pub fn from_validation<T>(v: Validation<T, Vec<ValidationError>>) -> Self {
        match v.into_result() {
            Ok(_) => Self::ok(),
            Err(errors) => Self::with_errors(errors.iter().map(|e| e.to_string()).collect()),
        }
    }

    pub fn merge(&mut self, other: ValidationResult) {
        self.inner = self.inner.clone().zip(other.inner)
            .map(|_| ());
    }
}
```

### APIs and Interfaces

**Public Validation API**:
```rust
// Core validation functions return Validation for composability
pub fn validate_paths(paths: &[PathBuf]) -> Validation<Vec<PathBuf>, Vec<ValidationError>>;
pub fn validate_environment(vars: &[String]) -> Validation<HashMap<String, String>, Vec<ValidationError>>;
pub fn validate_command(cmd: &str) -> Validation<ValidatedCommand, Vec<ValidationError>>;

// High-level validators compose smaller ones
pub fn validate_config(config: &Config) -> Validation<ValidConfig, Vec<ValidationError>>;
pub fn validate_workflow(workflow: &Workflow) -> Validation<ValidWorkflow, Vec<ValidationError>>;

// Conversion to Result for existing code
impl<T, E> From<Validation<T, E>> for Result<T, E> {
    fn from(v: Validation<T, E>) -> Self {
        v.into_result()
    }
}
```

## Dependencies

- **Prerequisites**: None (standalone improvement)
- **Affected Components**:
  - `src/core/validation/mod.rs` (325 lines) - complete rewrite
  - `src/config/command_validator.rs` (1113 lines) - partial migration
  - `src/cli/validation.rs` (90 lines) - migration
  - `src/worktree/manager_validation.rs` (partial)
  - `src/cook/workflow/validation.rs` (interaction points)
- **External Dependencies**: `stillwater = "0.1"`

## Testing Strategy

### Unit Tests

**Proof of Concept Test**:
```rust
#[test]
fn test_validation_accumulation() {
    let paths = vec![
        PathBuf::from("/nonexistent1"),
        PathBuf::from("/nonexistent2"),
        PathBuf::from("/nonexistent3"),
    ];

    let result = validate_paths(&paths);

    // Should accumulate ALL errors, not just first
    match result.into_result() {
        Err(errors) => {
            assert_eq!(errors.len(), 3);
            assert!(errors.iter().any(|e| matches!(e, ValidationError::PathNotFound(p) if p.ends_with("nonexistent1"))));
            assert!(errors.iter().any(|e| matches!(e, ValidationError::PathNotFound(p) if p.ends_with("nonexistent2"))));
            assert!(errors.iter().any(|e| matches!(e, ValidationError::PathNotFound(p) if p.ends_with("nonexistent3"))));
        },
        Ok(_) => panic!("Expected validation errors"),
    }
}
```

**Composition Test**:
```rust
#[test]
fn test_validation_composition() {
    let config = Config {
        paths: vec![PathBuf::from("/nonexistent")],
        env: vec!["MISSING_VAR".to_string()],
        commands: vec!["rm -rf /".to_string()],
        resources: ResourceLimits { memory: 999999999, .. },
    };

    let result = validate_config(&config);

    // Should accumulate errors from ALL validators
    match result.into_result() {
        Err(errors) => {
            assert!(errors.len() >= 4, "Should have errors from each validator");
            assert!(errors.iter().any(|e| matches!(e, ValidationError::PathNotFound(_))));
            assert!(errors.iter().any(|e| matches!(e, ValidationError::EnvVarMissing(_))));
            assert!(errors.iter().any(|e| matches!(e, ValidationError::CommandDangerous { .. })));
            assert!(errors.iter().any(|e| matches!(e, ValidationError::ResourceLimitExceeded { .. })));
        },
        Ok(_) => panic!("Expected validation errors"),
    }
}
```

### Integration Tests

- Test CLI commands show all validation errors on first attempt
- Test workflow validation reports complete error list
- Test configuration loading shows all config issues
- Test backward compatibility with existing error handling

### Performance Tests

**Benchmark Validation Performance**:
```rust
#[bench]
fn bench_path_validation_before(b: &mut Bencher) {
    let paths: Vec<_> = (0..1000).map(|i| PathBuf::from(format!("/tmp/test{}", i))).collect();
    b.iter(|| validate_paths_old(&paths));
}

#[bench]
fn bench_path_validation_after(b: &mut Bencher) {
    let paths: Vec<_> = (0..1000).map(|i| PathBuf::from(format!("/tmp/test{}", i))).collect();
    b.iter(|| validate_paths(&paths));
}
```

### User Acceptance

- Users see all validation errors on first attempt (no more fix-one-error-at-a-time)
- Error messages remain clear and actionable
- No behavioral changes in successful validation paths

## Documentation Requirements

### Code Documentation

- Document all public validation functions with examples
- Add module-level docs explaining validation patterns
- Document composition strategies with `.zip()` and `.all()`
- Provide migration guide for existing validation code

### User Documentation

- Update CLI help text to mention comprehensive error reporting
- Add examples showing multiple error scenarios
- Document validation error format changes (if any)

### Architecture Updates

Add new section to ARCHITECTURE.md:

```markdown
### Validation Architecture

Prodigy uses the stillwater library's `Validation<T, E>` type for error accumulation. This enables:

- **Complete Error Reports**: All validation errors reported on first attempt
- **Composable Validators**: Small validators combine into larger ones
- **Consistent Patterns**: Uniform validation approach across codebase

See `src/validation/` for core validation functions and composition patterns.
```

## Implementation Notes

### Migration Strategy

1. **Start Small**: Begin with `validate_paths()` as proof of concept
2. **Parallel Development**: Keep old validation working while building new
3. **Feature Flag**: Use `cfg` feature flag to enable stillwater validation
4. **Gradual Rollout**: Migrate one module at a time, verify tests pass
5. **Deprecation Period**: Mark old validation as deprecated, remove after one release

### Common Patterns

**Single Validator**:
```rust
fn validate_item(item: &Item) -> Validation<ValidItem, ValidationError> {
    if item.is_valid() {
        Validation::ok(ValidItem(item.clone()))
    } else {
        Validation::fail(ValidationError::InvalidItem)
    }
}
```

**List Validation**:
```rust
fn validate_items(items: &[Item]) -> Validation<Vec<ValidItem>, Vec<ValidationError>> {
    items.iter()
        .map(validate_item)
        .collect()
}
```

**Struct Validation**:
```rust
fn validate_config(cfg: &Config) -> Validation<ValidConfig, Vec<ValidationError>> {
    Validation::all((
        validate_field_a(&cfg.a),
        validate_field_b(&cfg.b),
        validate_field_c(&cfg.c),
    ))
    .map(|(a, b, c)| ValidConfig { a, b, c })
}
```

### Gotchas

- `Validation::all()` uses applicative composition (runs all validators even if some fail)
- `.and_then()` short-circuits (use for dependent validations)
- `.zip()` combines exactly two validators (use `.all()` for 3+)
- Error type must be cloneable for some operations
- Converting to `Result` loses multi-error accumulation (use sparingly)

## Migration and Compatibility

### Breaking Changes

None - this is an internal refactoring that preserves existing API.

### Compatibility Considerations

- **ValidationResult**: Keep existing struct, refactor internals
- **Error Types**: Preserve existing error messages and formats
- **Return Types**: Maintain existing function signatures where possible
- **Test Suite**: All existing tests must pass without modification

### Rollback Plan

If stillwater causes issues:
1. Disable via feature flag
2. Revert to manual accumulation
3. Keep manual code in separate module for fallback

### Future Work

After successful migration of core validation:
- Migrate `config/command_validator.rs` validation functions
- Migrate `cli/validation.rs` CLI argument validation
- Migrate `worktree/manager_validation.rs` git validation
- Consider migrating workflow validation in `cook/workflow/validation.rs`
