---
number: 166
title: Stillwater Complex Function Refactoring
category: optimization
priority: medium
status: draft
dependencies: [163, 164, 165]
created: 2025-11-22
---

# Specification 166: Stillwater Complex Function Refactoring

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 163 (Stillwater Validation Migration), Spec 164 (Stillwater Effect Composition), Spec 165 (Stillwater Error Context Enhancement)

## Context

After migrating to stillwater patterns (Validation, Effect, error context), several large and complex modules remain that could benefit from functional refactoring. These modules have grown organically and contain:

1. **Large files**:
   - `src/cook/workflow/validation.rs` (759 lines) - validation orchestration
   - `src/config/command_validator.rs` (1113 lines) - command validation registry
   - `src/core/workflow/mod.rs` (complex workflow logic)
   - `src/cook/execution/executor.rs` (mixed concerns)

2. **Complex functions**:
   - Functions > 20 lines with multiple responsibilities
   - Deep nesting (3+ levels) making logic hard to follow
   - Mixed I/O and business logic (despite some extraction)
   - Validation scattered throughout instead of composed

3. **Poor composability**:
   - Functions tightly coupled to specific contexts
   - Hard to reuse logic across different workflows
   - Difficult to test in isolation
   - Unclear data flow

These issues violate functional programming principles from specs 163-165 and the prodigy development guidelines. Now that stillwater infrastructure is in place, we can systematically refactor complex functions into:
- **Small, focused functions** (< 20 lines, single responsibility)
- **Pure functions** (no I/O, easily testable)
- **Composable validators** (using Validation combinators)
- **Declarative pipelines** (using Effect composition)

## Objective

Systematically refactor complex functions in prodigy using stillwater patterns, reducing function size, improving composability, enhancing testability, and making code more maintainable.

## Requirements

### Functional Requirements

- **FR1**: Identify all functions > 20 lines or cyclomatic complexity > 5
- **FR2**: Extract pure functions from complex functions
- **FR3**: Compose validation logic using Validation combinators
- **FR4**: Build I/O pipelines using Effect composition
- **FR5**: Reduce maximum function length to 20 lines
- **FR6**: Reduce maximum nesting depth to 2 levels
- **FR7**: Preserve all existing functionality (no behavioral changes)

### Non-Functional Requirements

- **NFR1**: Maintain or improve performance (no regression)
- **NFR2**: Increase test coverage to 95%+ for pure functions
- **NFR3**: Reduce file sizes to < 500 lines per module
- **NFR4**: Improve code readability (measured by review feedback)
- **NFR5**: Zero compilation warnings after refactoring
- **NFR6**: All existing tests pass without modification

## Acceptance Criteria

- [ ] Complexity audit completed identifying all target functions
- [ ] `cook/workflow/validation.rs` (759 lines) broken into smaller modules
- [ ] `config/command_validator.rs` (1113 lines) refactored using Validation
- [ ] All functions > 20 lines refactored to < 20 lines
- [ ] All functions with nesting > 2 levels refactored
- [ ] Pure functions extracted into `src/pure/` directory
- [ ] Effect pipelines replace imperative I/O sequences
- [ ] Validation composition replaces manual error accumulation
- [ ] Test coverage increased to 95%+ (from current baseline)
- [ ] Module structure documented in ARCHITECTURE.md
- [ ] Code review confirms readability improvement
- [ ] Performance benchmarks show no regression
- [ ] All existing tests pass
- [ ] Clippy warnings resolved

## Technical Details

### Implementation Approach

**Phase 1: Complexity Audit** (1 day)

Identify refactoring targets:

```bash
# Find complex functions using cargo-geiger, tokei, or manual review
rg "fn \w+.*\{" --count-matches src/
rg "if.*\{" --count-matches src/  # Approximation of complexity

# Manual review criteria:
# - Function length > 20 lines
# - Cyclomatic complexity > 5
# - Nesting depth > 2
# - Mixed I/O and pure logic
# - Manual error accumulation
```

Create complexity report:
```markdown
## Complexity Audit Report

### High Priority Targets (> 50 lines or complexity > 10)
- `cook/workflow/validation.rs::validate_workflow_complete` (120 lines, complexity 15)
- `config/command_validator.rs::validate_command` (85 lines, complexity 12)
- `cook/execution/executor.rs::execute_with_retry` (65 lines, complexity 10)

### Medium Priority Targets (20-50 lines or complexity 5-10)
- `core/workflow/mod.rs::interpolate_variables` (35 lines, complexity 7)
- `cook/input/processor.rs::process_input_source` (28 lines, complexity 6)

### Refactoring Strategy
For each function:
1. Extract pure functions
2. Compose using Validation/Effect
3. Add tests for extracted functions
4. Verify original tests pass
```

**Phase 2: Workflow Validation Refactoring** (5 days)

**Target**: `src/cook/workflow/validation.rs` (759 lines)

**Before** (simplified example):
```rust
pub fn validate_workflow_complete(
    workflow: &Workflow,
    config: &ValidationConfig,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Schema validation (30 lines)
    if workflow.name.is_empty() {
        errors.push(ValidationError::EmptyWorkflowName);
    }
    if workflow.steps.is_empty() {
        errors.push(ValidationError::NoSteps);
    }
    // ... 15 more checks

    // Command validation (40 lines)
    for step in &workflow.steps {
        if step.command.is_empty() {
            errors.push(ValidationError::EmptyCommand(step.id));
        }
        // ... 20 more checks per step
    }

    // Resource validation (30 lines)
    if let Some(ref limits) = workflow.resource_limits {
        if limits.memory > config.max_memory {
            errors.push(ValidationError::MemoryExceeded);
        }
        // ... 15 more checks
    }

    // Environment validation (20 lines)
    // ... more validation

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

**After** (using stillwater):

```rust
// Pure validators (small, focused, testable)
mod validators {
    use super::*;

    pub fn validate_workflow_name(name: &str) -> Validation<ValidName, ValidationError> {
        if name.is_empty() {
            Validation::fail(ValidationError::EmptyWorkflowName)
        } else {
            Validation::ok(ValidName(name.to_string()))
        }
    }

    pub fn validate_has_steps(steps: &[Step]) -> Validation<(), ValidationError> {
        if steps.is_empty() {
            Validation::fail(ValidationError::NoSteps)
        } else {
            Validation::ok(())
        }
    }

    pub fn validate_step(step: &Step) -> Validation<ValidStep, ValidationError> {
        Validation::all((
            validate_step_command(&step.command),
            validate_step_args(&step.args),
            validate_step_timeout(step.timeout),
        ))
        .map(|(cmd, args, timeout)| ValidStep { cmd, args, timeout })
    }

    pub fn validate_resource_limits(
        limits: &ResourceLimits,
        config: &ValidationConfig,
    ) -> Validation<ValidLimits, Vec<ValidationError>> {
        Validation::all((
            validate_memory_limit(limits.memory, config.max_memory),
            validate_cpu_limit(limits.cpu, config.max_cpu),
            validate_disk_limit(limits.disk, config.max_disk),
            validate_timeout(limits.timeout, config.max_timeout),
        ))
        .map(|(mem, cpu, disk, timeout)| ValidLimits { mem, cpu, disk, timeout })
    }
}

// Composed validator (declarative, < 20 lines)
pub fn validate_workflow_complete(
    workflow: &Workflow,
    config: &ValidationConfig,
) -> Validation<ValidWorkflow, Vec<ValidationError>> {
    Validation::all((
        validators::validate_workflow_name(&workflow.name),
        validators::validate_has_steps(&workflow.steps),
        workflow.steps.iter()
            .map(validators::validate_step)
            .collect::<Validation<Vec<_>, _>>(),
        workflow.resource_limits.as_ref()
            .map(|l| validators::validate_resource_limits(l, config))
            .unwrap_or_else(|| Validation::ok(ValidLimits::default())),
    ))
    .map(|(name, _, steps, limits)| ValidWorkflow { name, steps, limits })
}
```

**Benefits**:
- **Composability**: Each validator can be reused and tested independently
- **Clarity**: High-level composition reads like specification
- **Testability**: Pure validators need no mocking
- **Maintainability**: Easy to add/remove/modify validators

**New module structure**:
```
src/cook/workflow/
├── validation/
│   ├── mod.rs              # Public API, composition
│   ├── workflow.rs         # Workflow-level validators
│   ├── step.rs             # Step-level validators
│   ├── resource.rs         # Resource validators
│   ├── environment.rs      # Environment validators
│   └── config.rs           # Config validators
└── validation.rs           # Deprecated, re-exports for compatibility
```

**Phase 3: Command Validator Refactoring** (4 days)

**Target**: `src/config/command_validator.rs` (1113 lines)

**Current issues**:
- Massive registry pattern with manual validation
- Sequential validation with early returns
- Complex argument matching logic (100+ lines)
- Mixed validation and business logic

**Refactoring strategy**:

```rust
// Before: monolithic validator
pub fn validate_command(cmd: &Command) -> Result<ValidCommand, ValidationError> {
    // 85 lines of sequential validation with early returns
    validate_command_exists(cmd)?;
    validate_argument_count(cmd)?;
    validate_argument_types(cmd)?;
    validate_options(cmd)?;
    validate_permissions(cmd)?;
    // ...
    Ok(ValidCommand(cmd.clone()))
}

// After: composed validators
pub fn validate_command(cmd: &Command) -> Validation<ValidCommand, Vec<ValidationError>> {
    Validation::all((
        validate_command_exists(cmd),
        validate_arguments(cmd),
        validate_options(cmd),
        validate_permissions(cmd),
    ))
    .map(|_| ValidCommand(cmd.clone()))
}

// Extracted pure validators
fn validate_arguments(cmd: &Command) -> Validation<ValidArgs, Vec<ValidationError>> {
    let spec = COMMAND_REGISTRY.get_spec(&cmd.name)?;

    Validation::all((
        validate_argument_count(&cmd.args, &spec),
        validate_argument_types(&cmd.args, &spec),
        validate_required_args(&cmd.args, &spec),
    ))
    .map(|_| ValidArgs(cmd.args.clone()))
}

fn validate_argument_count(
    args: &[Arg],
    spec: &CommandSpec,
) -> Validation<(), ValidationError> {
    let count = args.len();
    if count < spec.min_args {
        Validation::fail(ValidationError::TooFewArgs { expected: spec.min_args, got: count })
    } else if count > spec.max_args {
        Validation::fail(ValidationError::TooManyArgs { expected: spec.max_args, got: count })
    } else {
        Validation::ok(())
    }
}
```

**Extract command registry to separate module**:
```
src/config/
├── command/
│   ├── mod.rs              # Public API
│   ├── registry.rs         # Command registry (data)
│   ├── spec.rs             # Command specifications
│   ├── validation/
│   │   ├── mod.rs          # Validator composition
│   │   ├── arguments.rs    # Argument validators
│   │   ├── options.rs      # Option validators
│   │   └── permissions.rs  # Permission validators
│   └── matchers.rs         # Argument type matching
└── command_validator.rs    # Deprecated, re-exports
```

**Phase 4: Effect Pipeline Refactoring** (3 days)

**Target**: Functions with mixed I/O and logic

**Example**: `cook/execution/executor.rs::execute_with_retry`

**Before**:
```rust
pub async fn execute_with_retry(
    &self,
    cmd: Command,
    retries: usize,
) -> Result<Output, ExecutorError> {
    let mut attempts = 0;
    let mut last_error = None;

    loop {
        attempts += 1;

        // Validate (pure)
        self.validate_command(&cmd)?;

        // Check resources (I/O)
        if !self.resource_monitor.has_capacity().await? {
            if attempts >= retries {
                return Err(ExecutorError::NoCapacity);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        // Execute (I/O)
        match self.spawn_process(&cmd).await {
            Ok(output) => {
                // Validate output (pure)
                self.validate_output(&output)?;
                return Ok(output);
            }
            Err(e) => {
                last_error = Some(e);
                if attempts >= retries {
                    return Err(last_error.unwrap());
                }
                tokio::time::sleep(Duration::from_secs(attempts * 2)).await;
            }
        }
    }
}
```

**After** (using Effect):

```rust
// Pure validators (extracted)
mod pure {
    pub fn validate_command(cmd: &Command) -> Validation<ValidCommand, ValidationError> {
        Validation::all((
            validate_not_empty(cmd),
            validate_args(cmd),
            validate_no_shell_injection(cmd),
        ))
        .map(|_| ValidCommand(cmd.clone()))
    }

    pub fn validate_output(output: &Output) -> Validation<ValidOutput, ValidationError> {
        if output.status.success() {
            Validation::ok(ValidOutput(output.clone()))
        } else {
            Validation::fail(ValidationError::NonZeroExit(output.status.code()))
        }
    }
}

// Effect-based execution
pub fn execute_with_retry(
    cmd: Command,
    retries: usize,
) -> Effect<Output, ExecutorError, AppEnv> {
    // Validate first
    Effect::from_validation(pure::validate_command(&cmd))
        .and_then(|valid_cmd| {
            // Retry logic as effect
            retry_effect(
                move || execute_once(valid_cmd.clone()),
                retries,
                Duration::from_secs(1),
            )
        })
        .and_then(|output| {
            // Validate output
            Effect::from_validation(pure::validate_output(&output))
                .map(|valid| valid.0)
        })
        .context(format!("Executing command: {}", cmd.name))
}

// Extracted retry effect (reusable!)
fn retry_effect<T, E, Env>(
    operation: impl Fn() -> Effect<T, E, Env>,
    max_retries: usize,
    backoff: Duration,
) -> Effect<T, E, Env>
where
    E: Clone,
{
    // Retry logic extracted as reusable combinator
    // (implementation details)
}

// Simple execute-once effect
fn execute_once(cmd: ValidCommand) -> Effect<Output, ExecutorError, AppEnv> {
    IO::execute(move |env| env.process.run(&cmd.to_command()))
        .context("Spawning process")
}
```

**Benefits**:
- Pure validation (easily tested)
- Reusable retry combinator
- Declarative error handling
- Clear separation of concerns

**Phase 5: Function Size Reduction** (3 days)

Systematically reduce all functions > 20 lines:

**Strategy**:
1. Identify cohesive blocks (5-10 lines each)
2. Extract to named function
3. Compose extracted functions
4. Verify tests pass

**Example** - variable interpolation:

**Before** (35 lines):
```rust
pub fn interpolate_variables(
    template: &str,
    vars: &HashMap<String, String>,
) -> Result<String, InterpolationError> {
    let mut result = template.to_string();
    let mut missing = Vec::new();

    // Find all variables (10 lines)
    let var_pattern = Regex::new(r"\$\{([^}]+)\}").unwrap();
    for capture in var_pattern.captures_iter(template) {
        let var_name = &capture[1];

        // Look up variable (15 lines)
        let value = if let Some(v) = vars.get(var_name) {
            v
        } else if let Ok(v) = env::var(var_name) {
            &v
        } else {
            missing.push(var_name.to_string());
            continue;
        };

        // Replace (10 lines)
        result = result.replace(&format!("${{{}}}", var_name), value);
    }

    if !missing.is_empty() {
        Err(InterpolationError::MissingVars(missing))
    } else {
        Ok(result)
    }
}
```

**After** (4 small functions):

```rust
// Pure: extract variable names (< 10 lines)
fn extract_variable_names(template: &str) -> Vec<String> {
    let var_pattern = Regex::new(r"\$\{([^}]+)\}").unwrap();
    var_pattern
        .captures_iter(template)
        .map(|cap| cap[1].to_string())
        .collect()
}

// Pure: resolve single variable (< 10 lines)
fn resolve_variable(
    name: &str,
    vars: &HashMap<String, String>,
) -> Option<String> {
    vars.get(name)
        .cloned()
        .or_else(|| env::var(name).ok())
}

// Pure: replace variable in template (< 5 lines)
fn replace_variable(template: &str, name: &str, value: &str) -> String {
    template.replace(&format!("${{{}}}", name), value)
}

// Composition (< 15 lines)
pub fn interpolate_variables(
    template: &str,
    vars: &HashMap<String, String>,
) -> Validation<String, Vec<InterpolationError>> {
    let var_names = extract_variable_names(template);

    let resolutions: Validation<Vec<(String, String)>, Vec<InterpolationError>> =
        var_names.iter()
            .map(|name| {
                resolve_variable(name, vars)
                    .map(|val| (name.clone(), val))
                    .ok_or(InterpolationError::MissingVar(name.clone()))
            })
            .collect();

    resolutions.map(|resolved| {
        resolved.iter()
            .fold(template.to_string(), |acc, (name, val)| {
                replace_variable(&acc, name, val)
            })
    })
}
```

**Phase 6: Module Organization** (2 days)

Reorganize code into clear module structure:

```
src/
├── pure/                   # Pure functions (no I/O)
│   ├── validation/         # Pure validators
│   ├── parsing/            # Pure parsers
│   ├── transformation/     # Pure transformations
│   └── calculation/        # Pure calculations
├── effects/               # Effect-based I/O
│   ├── config/            # Config loading effects
│   ├── storage/           # Storage effects
│   ├── execution/         # Execution effects
│   └── git/               # Git effects
└── composed/              # High-level compositions
    ├── workflows/         # Workflow orchestration
    ├── validation/        # Composed validators
    └── pipelines/         # Data pipelines
```

### Architecture Changes

**Functional Layers**:
```
CLI / API (imperative shell)
        ↓
Composed Operations (orchestration)
        ↓
Effects (I/O boundary)
        ↓
Pure Functions (core logic)
        ↓
Data Structures
```

**Complexity Constraints**:
- **Function length**: Max 20 lines (prefer 5-10)
- **Nesting depth**: Max 2 levels
- **Cyclomatic complexity**: Max 5
- **Parameters**: Max 4 (use structs for more)

### Data Structures

**Validation Results**:
```rust
// Small validated types
pub struct ValidName(String);
pub struct ValidCommand(Command);
pub struct ValidWorkflow {
    name: ValidName,
    steps: Vec<ValidStep>,
    limits: ValidLimits,
}
```

**Effect Pipelines**:
```rust
// Declarative pipeline types
pub type ConfigLoadPipeline = Effect<Config, ConfigError, AppEnv>;
pub type WorkflowValidationPipeline = Validation<ValidWorkflow, Vec<ValidationError>>;
pub type ExecutionPipeline = Effect<Output, ExecutorError, AppEnv>;
```

### APIs and Interfaces

**Pure Function API**:
```rust
// Small, focused, testable
pub fn validate_name(name: &str) -> Validation<ValidName, ValidationError>;
pub fn parse_config(yaml: &str) -> Result<Config, ParseError>;
pub fn transform_input(raw: RawInput) -> TransformedInput;
```

**Composed Validator API**:
```rust
// High-level composition
pub fn validate_workflow(wf: &Workflow) -> Validation<ValidWorkflow, Vec<ValidationError>>;
pub fn validate_config(cfg: &Config) -> Validation<ValidConfig, Vec<ValidationError>>;
```

**Effect Pipeline API**:
```rust
// Declarative I/O
pub fn load_config(path: PathBuf) -> Effect<Config, ConfigError, AppEnv>;
pub fn execute_workflow(wf: ValidWorkflow) -> Effect<Output, ExecutorError, AppEnv>;
```

## Dependencies

- **Prerequisites**:
  - Spec 163 (Stillwater Validation Migration)
  - Spec 164 (Stillwater Effect Composition)
  - Spec 165 (Stillwater Error Context Enhancement)
- **Affected Components**:
  - `src/cook/workflow/validation.rs` (759 lines) - major refactoring
  - `src/config/command_validator.rs` (1113 lines) - major refactoring
  - `src/core/workflow/mod.rs` - function extraction
  - `src/cook/execution/executor.rs` - effect pipelines
  - All modules with functions > 20 lines

## Testing Strategy

### Unit Tests

**Pure Function Tests** (no mocking):
```rust
#[test]
fn test_validate_workflow_name() {
    assert!(validate_name("valid").is_ok());
    assert!(validate_name("").is_err());
}

#[test]
fn test_extract_variable_names() {
    let names = extract_variable_names("${FOO} and ${BAR}");
    assert_eq!(names, vec!["FOO", "BAR"]);
}

#[test]
fn test_resolve_variable() {
    let mut vars = HashMap::new();
    vars.insert("FOO".to_string(), "value".to_string());

    assert_eq!(resolve_variable("FOO", &vars), Some("value".to_string()));
    assert_eq!(resolve_variable("MISSING", &vars), None);
}
```

**Composition Tests**:
```rust
#[test]
fn test_validate_workflow_composition() {
    let workflow = Workflow {
        name: "test".to_string(),
        steps: vec![Step { command: "build".to_string(), /* ... */ }],
        resource_limits: Some(ResourceLimits::default()),
    };

    let result = validate_workflow(&workflow);
    assert!(result.is_ok());
}
```

### Integration Tests

- Test composed validators with complex inputs
- Test effect pipelines end-to-end
- Verify refactored code behaves identically to original

### Performance Tests

**Benchmark Before/After**:
```rust
#[bench]
fn bench_validate_workflow_before(b: &mut Bencher) {
    let workflow = create_test_workflow();
    b.iter(|| validate_workflow_old(&workflow));
}

#[bench]
fn bench_validate_workflow_after(b: &mut Bencher) {
    let workflow = create_test_workflow();
    b.iter(|| validate_workflow(&workflow));
}
```

Expected: similar or better performance (zero-cost abstractions).

### User Acceptance

- All existing functionality works identically
- Code is more readable and maintainable
- Tests are faster and more comprehensive
- New features easier to add

## Documentation Requirements

### Code Documentation

- Document module structure and organization
- Explain pure/effect separation
- Provide examples of composition patterns
- Document function size and complexity guidelines

### User Documentation

No user-facing changes (internal refactoring).

### Architecture Updates

Add to ARCHITECTURE.md:

```markdown
### Code Organization

Prodigy follows strict functional programming principles:

**Function Constraints**:
- Max 20 lines per function (prefer 5-10)
- Max 2 levels of nesting
- Max cyclomatic complexity of 5
- Single responsibility per function

**Module Structure**:
```
src/
├── pure/       # Pure functions (no I/O, easily testable)
├── effects/    # I/O operations (using Effect)
└── composed/   # High-level orchestration
```

**Refactoring Workflow**:
1. Identify complex function (> 20 lines or complexity > 5)
2. Extract pure logic to `src/pure/`
3. Wrap I/O in Effect
4. Compose at higher level
5. Add comprehensive tests

See specs 163-166 for detailed refactoring patterns.
```

## Implementation Notes

### Refactoring Workflow

For each complex function:

1. **Analyze** (5 min):
   - Identify cohesive blocks
   - Separate pure logic from I/O
   - Note dependencies

2. **Extract** (15 min):
   - Extract pure functions (< 10 lines each)
   - Name descriptively
   - Add type signatures

3. **Compose** (10 min):
   - Compose using Validation/Effect
   - Keep composition function < 20 lines
   - Add context to effects

4. **Test** (20 min):
   - Test each pure function
   - Test composition
   - Verify original tests pass

5. **Document** (5 min):
   - Add doc comments
   - Update module docs if needed

Total: ~1 hour per complex function

### Common Patterns

**Extract validation logic**:
```rust
// Before
if condition1 && condition2 && condition3 {
    // ...
}

// After
fn validate_conditions(x: &X) -> Validation<(), Error> {
    Validation::all((
        validate_condition1(x),
        validate_condition2(x),
        validate_condition3(x),
    ))
}
```

**Extract I/O operations**:
```rust
// Before
let data = fs::read(&path)?;
let parsed = parse(data)?;
let validated = validate(parsed)?;

// After
fn load_and_validate(path: PathBuf) -> Effect<ValidData, Error, AppEnv> {
    IO::query(|env| env.fs.read(&path))
        .map(|data| parse(data))
        .and_then(Effect::from_result)
        .and_then(|parsed| Effect::from_validation(validate(parsed)))
}
```

**Extract complex logic**:
```rust
// Before: 40 lines of nested logic
fn complex_function(input: Input) -> Output {
    // ... 40 lines ...
}

// After: 4 functions of 10 lines each
fn step1(input: Input) -> Result1 { /* 8 lines */ }
fn step2(r1: Result1) -> Result2 { /* 9 lines */ }
fn step3(r2: Result2) -> Result3 { /* 7 lines */ }
fn complex_function(input: Input) -> Output {
    step1(input)
        .and_then(step2)
        .and_then(step3)
}
```

### Gotchas

- Don't extract prematurely - wait until function is actually complex
- Don't over-abstract - 3 similar lines don't need a function
- Do extract when logic is reused 2+ times
- Do extract when testing requires complex setup
- Keep extraction within same file initially (move to module later)

## Migration and Compatibility

### Breaking Changes

None - internal refactoring only.

### Compatibility Strategy

- Refactor implementation, keep public API
- Add new functions alongside old (mark old as deprecated)
- Remove deprecated after one release
- Provide re-exports during transition

### Rollback Plan

All refactoring is incremental:
- Each function refactored independently
- Original tests verify behavior
- Easy to revert individual changes if issues arise

### Future Work

- Automated complexity detection in CI
- Complexity budget per module
- Refactoring suggestions in code review
- Complexity metrics dashboard
- Additional functional patterns (trampolining, recursion schemes)
