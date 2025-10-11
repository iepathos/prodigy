---
number: 128
title: Immutable Environment Context Pattern
category: foundation
priority: high
status: draft
dependencies: [101, 127]
created: 2025-10-11
---

# Specification 128: Immutable Environment Context Pattern

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 101 (Error Handling), Spec 127 (Worktree Isolation)

## Context

The current `EnvironmentManager` contains hidden mutable state (`current_dir` field) that caused a critical bug in MapReduce workflows (Spec 127). The `set_working_dir()` method mutates internal state, making it difficult to reason about which directory will be used when steps don't specify an explicit `working_dir`.

**Bug Example**:
```rust
// EnvironmentManager created with main repo directory
let mut env_manager = EnvironmentManager::new(std::env::current_dir()?)?;

// Later, in MapReduce setup phase...
env_manager.set_working_dir(worktree_path);  // Hidden mutation!

// Problem: This mutation is not visible in function signatures
// Steps executed after this may or may not use worktree_path depending on:
// 1. Whether step.working_dir is Some or None
// 2. Whether EnvironmentManager.current_dir was updated
// 3. Call order and timing
```

This violates functional programming principles:
- **Hidden side effects**: State changes not visible in function signatures
- **Action at a distance**: Mutations affect future behavior non-locally
- **Difficult testing**: Must mock entire state machine
- **Race conditions**: Mutable state in async contexts is error-prone

## Objective

Replace mutable `EnvironmentManager` with an immutable `EnvironmentContext` pattern where:
1. All environment configuration is immutable
2. Working directory is determined by pure functions
3. State transformations return new values instead of mutating
4. All inputs and outputs are explicit in function signatures

## Requirements

### Functional Requirements

1. **Immutable EnvironmentContext**
   - Create `EnvironmentContext` struct with immutable fields
   - No `&mut self` methods on EnvironmentContext
   - All state is passed as function parameters

2. **Pure Working Directory Resolution**
   - Extract `resolve_working_directory()` as pure function
   - Takes all inputs as parameters (step, env, context)
   - Returns PathBuf without side effects
   - Testable without mocking

3. **Explicit State Flow**
   - Working directory explicitly passed to command executors
   - No hidden state in manager objects
   - Clear data flow through function parameters

4. **Builder Pattern for Context Creation**
   - `EnvironmentContextBuilder` for constructing contexts
   - Fluent API for adding environment variables
   - Returns immutable `EnvironmentContext`

5. **Backward Compatibility**
   - Existing workflows continue to work
   - Gradual migration path from EnvironmentManager
   - Both patterns can coexist temporarily

### Non-Functional Requirements

- **Performance**: No performance regression vs current implementation
- **Memory**: Efficient cloning/copying of environment data
- **Testability**: 100% unit test coverage for pure functions
- **Maintainability**: Clear separation of pure/impure code

## Acceptance Criteria

- [ ] `EnvironmentContext` struct implemented with only immutable fields
- [ ] `resolve_working_directory()` pure function extracts working dir logic
- [ ] `build_command_env()` pure function creates environment variables
- [ ] `EnvironmentContextBuilder` provides fluent API for context creation
- [ ] All MapReduce phases explicitly pass working directory
- [ ] Unit tests for all pure functions (no mocks needed)
- [ ] Integration tests verify MapReduce workflows execute in worktrees
- [ ] Git status shows main repo remains clean after MapReduce workflows
- [ ] Documentation explains functional approach and benefits
- [ ] Migration guide from EnvironmentManager to EnvironmentContext

## Technical Details

### Implementation Approach

#### Phase 1: Create Immutable EnvironmentContext

```rust
// src/cook/environment/context.rs (NEW)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Immutable environment context for command execution
///
/// This struct contains all environment configuration needed for
/// executing commands. It is immutable after construction, preventing
/// hidden state mutations.
#[derive(Debug, Clone)]
pub struct EnvironmentContext {
    /// Base working directory (typically main repo or worktree)
    pub base_working_dir: Arc<PathBuf>,

    /// Environment variables (immutable after creation)
    pub env_vars: Arc<HashMap<String, String>>,

    /// Secret keys for masking in logs
    pub secret_keys: Arc<Vec<String>>,

    /// Active profile name (if any)
    pub profile: Option<Arc<str>>,
}

impl EnvironmentContext {
    /// Create new environment context
    pub fn new(base_working_dir: PathBuf) -> Self {
        Self {
            base_working_dir: Arc::new(base_working_dir),
            env_vars: Arc::new(HashMap::new()),
            secret_keys: Arc::new(Vec::new()),
            profile: None,
        }
    }

    /// Get base working directory
    pub fn working_dir(&self) -> &Path {
        &self.base_working_dir
    }

    /// Get environment variables (immutable reference)
    pub fn env_vars(&self) -> &HashMap<String, String> {
        &self.env_vars
    }

    /// Check if a key is a secret (for masking)
    pub fn is_secret(&self, key: &str) -> bool {
        self.secret_keys.contains(&key.to_string())
    }
}
```

#### Phase 2: Pure Working Directory Resolution

```rust
// src/cook/environment/pure.rs (NEW)

use std::path::{Path, PathBuf};
use crate::cook::workflow::WorkflowStep;
use crate::cook::orchestrator::ExecutionEnvironment;
use super::context::EnvironmentContext;

/// Resolve working directory for a step (PURE FUNCTION)
///
/// Determines which directory to use for command execution based on:
/// 1. Explicit step.working_dir (highest priority)
/// 2. Environment context base directory (from worktree or repo)
/// 3. Execution environment working_dir (fallback)
///
/// # Arguments
/// * `step` - Workflow step (may specify explicit working_dir)
/// * `env` - Execution environment (from orchestrator)
/// * `context` - Environment context (from builder)
///
/// # Returns
/// PathBuf representing the resolved working directory
///
/// # Examples
/// ```
/// let step = WorkflowStep { working_dir: Some(PathBuf::from("/custom")), .. };
/// let working_dir = resolve_working_directory(&step, &env, &context);
/// assert_eq!(working_dir, PathBuf::from("/custom"));
/// ```
pub fn resolve_working_directory(
    step: &WorkflowStep,
    env: &ExecutionEnvironment,
    context: &EnvironmentContext,
) -> PathBuf {
    // 1. Explicit step working_dir takes highest precedence
    if let Some(ref dir) = step.working_dir {
        return dir.clone();
    }

    // 2. Use environment context base directory (set by caller)
    //    This allows MapReduce workflows to explicitly set worktree directory
    context.working_dir().to_path_buf()

    // Note: We intentionally do NOT fall back to env.working_dir here
    // because context.base_working_dir should always be correctly set
    // by the caller (either to repo dir or worktree dir)
}

/// Build complete environment variables for command execution (PURE FUNCTION)
///
/// Combines global environment config, step-specific env vars, and
/// workflow variables to produce the final environment for a command.
///
/// # Arguments
/// * `step` - Workflow step with step-specific env vars
/// * `context` - Environment context with base env vars
/// * `workflow_vars` - Variables from workflow context (for interpolation)
///
/// # Returns
/// HashMap of all environment variables for command
///
/// # Examples
/// ```
/// let env_vars = build_command_env(&step, &context, &workflow_vars);
/// assert_eq!(env_vars.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
/// ```
pub fn build_command_env(
    step: &WorkflowStep,
    context: &EnvironmentContext,
    workflow_vars: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut env = context.env_vars().clone();

    // Add step-specific environment variables
    for (key, value) in &step.env {
        let interpolated = interpolate_value(value, workflow_vars);
        env.insert(key.clone(), interpolated);
    }

    // Add Prodigy-specific variables
    env.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

    env
}

/// Interpolate variables in a value (PURE FUNCTION)
fn interpolate_value(
    value: &str,
    variables: &HashMap<String, String>,
) -> String {
    let mut result = value.to_string();

    // Simple ${var} and $var interpolation
    for (key, val) in variables {
        result = result.replace(&format!("${{{}}}", key), val);
        result = result.replace(&format!("${}", key), val);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_resolve_working_directory_explicit_step() {
        let step = WorkflowStep {
            working_dir: Some(PathBuf::from("/explicit")),
            ..Default::default()
        };
        let env = ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/env")),
            project_dir: Arc::new(PathBuf::from("/project")),
            worktree_name: None,
            session_id: Arc::from("test"),
        };
        let context = EnvironmentContext::new(PathBuf::from("/context"));

        let result = resolve_working_directory(&step, &env, &context);
        assert_eq!(result, PathBuf::from("/explicit"));
    }

    #[test]
    fn test_resolve_working_directory_from_context() {
        let step = WorkflowStep {
            working_dir: None,
            ..Default::default()
        };
        let env = ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/env")),
            project_dir: Arc::new(PathBuf::from("/project")),
            worktree_name: None,
            session_id: Arc::from("test"),
        };
        let context = EnvironmentContext::new(PathBuf::from("/worktree"));

        let result = resolve_working_directory(&step, &env, &context);
        assert_eq!(result, PathBuf::from("/worktree"));
    }

    #[test]
    fn test_build_command_env_step_vars() {
        let step = WorkflowStep {
            env: vec![
                ("CUSTOM".to_string(), "value".to_string()),
            ].into_iter().collect(),
            ..Default::default()
        };
        let context = EnvironmentContext::new(PathBuf::from("/test"));
        let workflow_vars = HashMap::new();

        let result = build_command_env(&step, &context, &workflow_vars);

        assert_eq!(result.get("CUSTOM"), Some(&"value".to_string()));
        assert_eq!(result.get("PRODIGY_AUTOMATION"), Some(&"true".to_string()));
    }

    #[test]
    fn test_build_command_env_interpolation() {
        let step = WorkflowStep {
            env: vec![
                ("MESSAGE".to_string(), "Hello ${NAME}".to_string()),
            ].into_iter().collect(),
            ..Default::default()
        };
        let context = EnvironmentContext::new(PathBuf::from("/test"));
        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("NAME".to_string(), "World".to_string());

        let result = build_command_env(&step, &context, &workflow_vars);

        assert_eq!(result.get("MESSAGE"), Some(&"Hello World".to_string()));
    }
}
```

#### Phase 3: Builder Pattern for Context Creation

```rust
// src/cook/environment/builder.rs (NEW)

use super::context::EnvironmentContext;
use super::config::{EnvironmentConfig, EnvProfile};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

/// Builder for creating immutable EnvironmentContext
pub struct EnvironmentContextBuilder {
    base_working_dir: PathBuf,
    env_vars: HashMap<String, String>,
    secret_keys: Vec<String>,
    profile: Option<String>,
}

impl EnvironmentContextBuilder {
    /// Create new builder with base working directory
    pub fn new(base_working_dir: PathBuf) -> Self {
        Self {
            base_working_dir,
            env_vars: std::env::vars().collect(), // Inherit current env
            secret_keys: Vec::new(),
            profile: None,
        }
    }

    /// Add environment variable
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_vars.insert(key, value);
        self
    }

    /// Add multiple environment variables
    pub fn with_env_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.env_vars.extend(vars);
        self
    }

    /// Mark a key as secret (for masking)
    pub fn with_secret(mut self, key: String) -> Self {
        self.secret_keys.push(key);
        self
    }

    /// Set active profile
    pub fn with_profile(mut self, profile: String) -> Self {
        self.profile = Some(profile);
        self
    }

    /// Apply global environment configuration
    pub fn with_config(mut self, config: &EnvironmentConfig) -> Result<Self> {
        // Apply profile if specified
        if let Some(profile_name) = &config.active_profile {
            self = self.with_profile(profile_name.clone());

            if let Some(profile) = config.profiles.get(profile_name) {
                self = self.apply_profile(profile)?;
            }
        }

        // Apply global env vars from config
        for (key, value) in &config.global_env {
            // Resolve EnvValue to String (static values only for now)
            if let crate::cook::environment::EnvValue::Static(s) = value {
                self = self.with_env(key.clone(), s.clone());
            }
        }

        // Mark secrets
        for key in config.secrets.keys() {
            self = self.with_secret(key.clone());
        }

        Ok(self)
    }

    /// Apply environment profile
    fn apply_profile(mut self, profile: &EnvProfile) -> Result<Self> {
        for (key, value) in &profile.env {
            self = self.with_env(key.clone(), value.clone());
        }
        Ok(self)
    }

    /// Build immutable EnvironmentContext
    pub fn build(self) -> EnvironmentContext {
        use std::sync::Arc;

        EnvironmentContext {
            base_working_dir: Arc::new(self.base_working_dir),
            env_vars: Arc::new(self.env_vars),
            secret_keys: Arc::new(self.secret_keys),
            profile: self.profile.map(Arc::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_env("KEY".to_string(), "value".to_string())
            .with_secret("SECRET".to_string())
            .build();

        assert_eq!(context.working_dir(), PathBuf::from("/test").as_path());
        assert_eq!(context.env_vars().get("KEY"), Some(&"value".to_string()));
        assert!(context.is_secret("SECRET"));
    }

    #[test]
    fn test_builder_multiple_env_vars() {
        let mut vars = HashMap::new();
        vars.insert("A".to_string(), "1".to_string());
        vars.insert("B".to_string(), "2".to_string());

        let context = EnvironmentContextBuilder::new(PathBuf::from("/test"))
            .with_env_vars(vars)
            .build();

        assert_eq!(context.env_vars().get("A"), Some(&"1".to_string()));
        assert_eq!(context.env_vars().get("B"), Some(&"2".to_string()));
    }
}
```

#### Phase 4: Update MapReduce Executor

```rust
// src/cook/workflow/executor.rs (MODIFIED)

// In execute_mapreduce_workflow method:
async fn execute_mapreduce_workflow(
    &mut self,
    workflow: &NormalizedWorkflow,
    session_id: &str,
) -> Result<()> {
    // Create parent worktree
    let worktree_result = self.worktree_manager.create_worktree(...).await?;

    // EXPLICIT: Create environment context for worktree execution
    let worktree_context = EnvironmentContextBuilder::new(worktree_result.path.clone())
        .with_config(self.global_environment_config.as_ref().unwrap_or(&EnvironmentConfig::default()))?
        .build();

    // Execute setup phase with explicit worktree context
    if !setup_phase.commands.is_empty() {
        let mut setup_executor = SetupPhaseExecutor::new(&setup_phase);

        // Pass worktree context explicitly
        let (captured, gen_file) = setup_executor
            .execute_with_context(
                &setup_phase.commands,
                self,
                &worktree_env,
                &worktree_context,  // <-- Explicit context
                &mut workflow_context,
            )
            .await?;

        // ... rest of setup phase
    }

    // ... map and reduce phases also use worktree_context
}

// In setup_step_environment_context method:
async fn setup_step_environment_context(
    &mut self,
    step: &WorkflowStep,
    env: &ExecutionEnvironment,
    context: &EnvironmentContext,  // <-- New parameter
    ctx: &mut WorkflowContext,
) -> Result<(HashMap<String, String>, PathBuf)> {
    // Pure function determines working directory
    let working_dir = resolve_working_directory(step, env, context);

    // Pure function builds environment variables
    let env_vars = build_command_env(step, context, &ctx.variables);

    Ok((env_vars, working_dir))
}
```

### Architecture Changes

**Before (Mutable State)**:
```
EnvironmentManager (mutable)
  ├─ current_dir: PathBuf (MUTABLE - hidden state)
  ├─ base_env: HashMap (MUTABLE)
  └─ set_working_dir(&mut self, dir) → ()  // Hidden mutation
```

**After (Immutable Context)**:
```
EnvironmentContext (immutable)
  ├─ base_working_dir: Arc<PathBuf> (IMMUTABLE)
  ├─ env_vars: Arc<HashMap> (IMMUTABLE)
  └─ working_dir(&self) → &Path  // Immutable accessor

Pure Functions (no state):
  ├─ resolve_working_directory(step, env, context) → PathBuf
  └─ build_command_env(step, context, vars) → HashMap
```

### Data Structures

```rust
/// Before: Mutable manager with hidden state
pub struct EnvironmentManager {
    current_dir: PathBuf,  // Hidden mutable state!
    // ... other fields
}

/// After: Immutable context passed explicitly
pub struct EnvironmentContext {
    base_working_dir: Arc<PathBuf>,  // Immutable
    env_vars: Arc<HashMap<String, String>>,  // Immutable
    secret_keys: Arc<Vec<String>>,  // Immutable
}
```

## Dependencies

- **Spec 101**: Error handling patterns (anyhow::Result, context)
- **Spec 127**: Worktree isolation for MapReduce workflows
- **Spec 102B**: Clean executor rebuild (if implemented, coordinates with this)

## Testing Strategy

### Unit Tests (Pure Functions)

```rust
#[test]
fn test_resolve_working_directory_priority() {
    // Test 1: Step working_dir takes precedence
    let step = WorkflowStep { working_dir: Some(PathBuf::from("/step")), .. };
    assert_eq!(
        resolve_working_directory(&step, &env, &context),
        PathBuf::from("/step")
    );

    // Test 2: Context working_dir used when step has none
    let step = WorkflowStep { working_dir: None, .. };
    let context = EnvironmentContext::new(PathBuf::from("/worktree"));
    assert_eq!(
        resolve_working_directory(&step, &env, &context),
        PathBuf::from("/worktree")
    );
}

#[test]
fn test_build_command_env_interpolation() {
    let mut workflow_vars = HashMap::new();
    workflow_vars.insert("VAR".to_string(), "value".to_string());

    let step = WorkflowStep {
        env: vec![("KEY".to_string(), "prefix-${VAR}".to_string())].into_iter().collect(),
        ..
    };

    let env = build_command_env(&step, &context, &workflow_vars);
    assert_eq!(env.get("KEY"), Some(&"prefix-value".to_string()));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_mapreduce_setup_phase_in_worktree() {
    // Execute MapReduce workflow
    let result = execute_mapreduce_workflow(&workflow).await?;

    // Verify files created in worktree, not main repo
    let worktree_files = list_files(&worktree_path)?;
    assert!(worktree_files.contains(&".prodigy/book-analysis".to_string()));

    // Verify main repo is clean
    let repo_status = run_command("git status --short", &repo_path)?;
    assert!(!repo_status.contains(".prodigy/book-analysis"));
}
```

### Property-Based Tests

```rust
#[quickcheck]
fn prop_resolve_working_directory_deterministic(
    step: WorkflowStep,
    env: ExecutionEnvironment,
    context: EnvironmentContext,
) -> bool {
    let result1 = resolve_working_directory(&step, &env, &context);
    let result2 = resolve_working_directory(&step, &env, &context);
    result1 == result2  // Always returns same result
}
```

## Documentation Requirements

### Code Documentation

```rust
/// Resolve working directory for a step (PURE FUNCTION)
///
/// This function determines which directory to use for command execution.
/// It is a pure function with no side effects, making it easy to test
/// and reason about.
///
/// # Resolution Priority
/// 1. Explicit `step.working_dir` (highest priority)
/// 2. `context.base_working_dir` (set by caller)
/// 3. Never falls back to `env.working_dir` (callers must set context correctly)
///
/// # Why This Matters
/// Previous implementation used hidden mutable state in EnvironmentManager,
/// which caused bugs where steps executed in wrong directories. This pure
/// function makes the resolution logic explicit and testable.
///
/// # Examples
/// ```rust
/// // MapReduce workflow: context explicitly set to worktree
/// let context = EnvironmentContext::new(worktree_path);
/// let working_dir = resolve_working_directory(&step, &env, &context);
/// assert_eq!(working_dir, worktree_path);
/// ```
```

### Migration Guide

```markdown
## Migrating from EnvironmentManager to EnvironmentContext

### Before (Mutable State)

```rust
// Create mutable manager
let mut env_manager = EnvironmentManager::new(current_dir)?;

// Hidden mutation - not visible in function signature
env_manager.set_working_dir(worktree_path);

// Implicit: working directory might be worktree_path or might not be
execute_step(step, &mut env_manager).await?;
```

### After (Immutable Context)

```rust
// Create immutable context with explicit working directory
let context = EnvironmentContextBuilder::new(worktree_path)
    .with_config(&global_config)?
    .build();

// Explicit: working directory passed as parameter
let working_dir = resolve_working_directory(&step, &env, &context);
execute_command(step, &working_dir, &context).await?;
```

### Benefits

1. **No Hidden State**: All inputs visible in function signatures
2. **Testable**: Pure functions don't need mocks
3. **Predictable**: Same inputs always produce same outputs
4. **Thread-Safe**: Immutable data is inherently thread-safe
```

## Implementation Notes

### Performance Considerations

- Use `Arc` for shared immutable data (cheap cloning)
- Environment variables HashMap is cloned when needed (acceptable overhead)
- PathBuf resolution is fast (no I/O, just logic)

### Error Handling

- Pure functions return `PathBuf` directly (no Result needed)
- Builder can return `Result` for config validation
- Use `anyhow::Context` for error messages

### Gotchas

- **Don't fall back to env.working_dir**: Callers must explicitly set context
- **Clone step.working_dir**: Don't return references (lifetime issues)
- **Test with worktrees**: Ensure MapReduce tests verify worktree isolation

## Migration and Compatibility

### Phase 1: Add New Code (No Breaking Changes)

- Add `EnvironmentContext`, `EnvironmentContextBuilder`, pure functions
- Existing `EnvironmentManager` continues to work
- Both can coexist

### Phase 2: Migrate MapReduce Workflows

- Update `execute_mapreduce_workflow` to use `EnvironmentContext`
- Update `SetupPhaseExecutor` to accept context parameter
- Keep backward compatibility with existing workflows

### Phase 3: Deprecate EnvironmentManager

- Mark `EnvironmentManager::set_working_dir()` as `#[deprecated]`
- Add deprecation warnings
- Provide migration guide

### Phase 4: Remove EnvironmentManager (Future)

- After all code migrated to `EnvironmentContext`
- Remove deprecated methods
- Clean up tests

## Success Metrics

- **Zero bugs** related to working directory resolution
- **100% test coverage** for pure functions
- **Performance**: No measurable regression
- **Code clarity**: Reviewers understand working directory logic immediately
- **Maintainability**: New developers can modify without introducing bugs

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing workflows | High | Backward compatibility, gradual migration |
| Performance regression from cloning | Medium | Use Arc for shared data, benchmark |
| Incomplete migration leaves both patterns | Medium | Clear deprecation path, tracking |
| Complexity from two approaches | Low | Document migration, provide examples |

## Related Specifications

- **Spec 127**: Worktree isolation - this spec fixes the bug discovered there
- **Spec 101**: Error handling - follows error handling patterns
- **Spec 102B**: Clean executor - coordinates with executor refactoring if implemented

## Future Enhancements

### Type-Safe Working Directory

```rust
/// Newtype for working directories
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingDirectory(PathBuf);

impl WorkingDirectory {
    pub fn new(path: PathBuf) -> Result<Self> {
        // Validate path exists and is directory
        if !path.exists() || !path.is_dir() {
            return Err(anyhow!("Invalid working directory: {}", path.display()));
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

// Then resolve_working_directory returns WorkingDirectory instead of PathBuf
```

### Lens-Based Updates

```rust
// Functional optics for updating immutable contexts
impl EnvironmentContext {
    pub fn with_working_dir(self, dir: PathBuf) -> Self {
        Self {
            base_working_dir: Arc::new(dir),
            ..self
        }
    }

    pub fn with_env_var(mut self, key: String, value: String) -> Self {
        // Clone-on-write for Arc<HashMap>
        let mut vars = (*self.env_vars).clone();
        vars.insert(key, value);
        Self {
            env_vars: Arc::new(vars),
            ..self
        }
    }
}
```

## Appendix: Functional Programming Principles Applied

1. **Immutability**: EnvironmentContext has no mutable fields
2. **Pure Functions**: resolve_working_directory has no side effects
3. **Explicit State**: All state passed as function parameters
4. **Referential Transparency**: Same inputs → same outputs always
5. **Type Safety**: Use type system to prevent invalid states
6. **Composition**: Builder pattern composes immutable transformations
