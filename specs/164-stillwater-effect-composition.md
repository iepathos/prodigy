---
number: 164
title: Stillwater Effect Composition
category: foundation
priority: high
status: draft
dependencies: [163]
created: 2025-11-22
---

# Specification 164: Stillwater Effect Composition

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 163 (Stillwater Validation Migration)

## Context

Prodigy's codebase exhibits common mixing of pure business logic with I/O operations, making testing difficult and reasoning about code harder. Key problem areas:

1. **`src/config/loader.rs`**: File reading mixed with parsing and validation
2. **`src/cook/execution/executor.rs`**: Pure validation mixed with async resource checks
3. **`src/cook/input/processor.rs`**: Sequential async operations without clear separation
4. **`src/storage/`**: Database operations mixed with business logic
5. **`src/worktree/manager.rs`**: Git operations mixed with validation logic

This violates the **pure core, imperative shell** principle, resulting in:
- **Hard-to-test code**: Must mock file system, database, and git for basic logic tests
- **Hidden dependencies**: Functions depend on global state or environment
- **Difficult refactoring**: Can't extract pure functions without carrying I/O
- **Poor composability**: Can't reuse logic without duplicating I/O setup

Stillwater's `Effect<T, E, Env>` type provides a solution by:
- Separating pure logic (transformations, calculations, validations) from I/O (file, network, database)
- Making dependencies explicit through environment type parameter
- Enabling declarative composition with `.map()`, `.and_then()`, `.context()`
- Supporting testability through mock environments

## Objective

Refactor prodigy's I/O-heavy code to use stillwater's Effect composition pattern, separating pure business logic from side effects and improving testability, maintainability, and clarity.

## Requirements

### Functional Requirements

- **FR1**: Separate all pure functions from I/O operations in target modules
- **FR2**: Use `Effect<T, E, Env>` for all I/O operations (file, network, database, git)
- **FR3**: Define environment traits for each I/O category (FileEnv, DbEnv, GitEnv)
- **FR4**: Create mock implementations for testing
- **FR5**: Build declarative pipelines using `.map()`, `.and_then()`, `.context()`
- **FR6**: Preserve all existing functionality and behavior
- **FR7**: Enable running effects with real environment at application boundaries

### Non-Functional Requirements

- **NFR1**: Zero performance overhead from Effect abstractions (verify with benchmarks)
- **NFR2**: 100% test coverage for extracted pure functions
- **NFR3**: Reduce coupling between modules by making dependencies explicit
- **NFR4**: Improve code organization with clear separation of concerns
- **NFR5**: Enable parallel testing by using thread-local mock environments
- **NFR6**: Maintain async/await compatibility for existing async code

## Acceptance Criteria

- [ ] Environment traits defined: `FileEnv`, `DbEnv`, `GitEnv`, `ProcessEnv`
- [ ] Mock implementations created for all environment traits
- [ ] `config/loader.rs::load_from_path` refactored to use Effect composition
- [ ] Pure functions extracted: `parse_config`, `validate_config_format`
- [ ] `cook/execution/executor.rs::validate_request` separated into pure and I/O
- [ ] `cook/input/processor.rs` refactored to declarative Effect pipeline
- [ ] `storage/` database operations wrapped in Effect with DbEnv
- [ ] `worktree/manager.rs` git operations wrapped in Effect with GitEnv
- [ ] Test suite updated with mock environment tests
- [ ] Unit tests cover 100% of pure functions (no I/O mocking needed)
- [ ] Integration tests verify Effect pipelines with real environments
- [ ] Benchmarks confirm zero performance regression
- [ ] Documentation explains Effect pattern and usage
- [ ] Migration guide created for additional Effect refactoring

## Technical Details

### Implementation Approach

**Phase 1: Environment Trait Design** (2 days)

Define environment traits for each I/O category:

```rust
/// File system operations
pub trait FileEnv {
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn write(&self, path: &Path, content: &str) -> io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;
}

/// Database operations
pub trait DbEnv {
    fn fetch_workflow(&self, id: WorkflowId) -> Result<Workflow, DbError>;
    fn save_workflow(&self, workflow: &Workflow) -> Result<(), DbError>;
    fn fetch_events(&self, workflow_id: WorkflowId) -> Result<Vec<Event>, DbError>;
}

/// Git operations
pub trait GitEnv {
    fn worktree_add(&self, path: &Path, branch: &str) -> Result<(), GitError>;
    fn worktree_remove(&self, path: &Path) -> Result<(), GitError>;
    fn merge(&self, branch: &str, strategy: MergeStrategy) -> Result<MergeResult, GitError>;
    fn commit(&self, message: &str) -> Result<CommitHash, GitError>;
}

/// Process execution
pub trait ProcessEnv {
    fn spawn(&self, cmd: &Command) -> Result<Child, io::Error>;
    fn run(&self, cmd: &Command) -> Result<Output, io::Error>;
}

/// Combined application environment
pub struct AppEnv {
    pub fs: Box<dyn FileEnv>,
    pub db: Box<dyn DbEnv>,
    pub git: Box<dyn GitEnv>,
    pub process: Box<dyn ProcessEnv>,
}
```

**Phase 2: Mock Implementations** (1 day)

```rust
/// Mock file system for testing
pub struct MockFileEnv {
    files: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl MockFileEnv {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_file(&self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.files.lock().unwrap().insert(path.into(), content.into());
    }
}

impl FileEnv for MockFileEnv {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.files.lock().unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "file not found"))
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        self.files.lock().unwrap().insert(path.to_path_buf(), content.to_string());
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        // Return mock metadata
        unimplemented!("Mock metadata not needed yet")
    }
}

/// Mock database for testing
pub struct MockDbEnv {
    workflows: Arc<Mutex<HashMap<WorkflowId, Workflow>>>,
    events: Arc<Mutex<Vec<Event>>>,
}

impl DbEnv for MockDbEnv {
    fn fetch_workflow(&self, id: WorkflowId) -> Result<Workflow, DbError> {
        self.workflows.lock().unwrap()
            .get(&id)
            .cloned()
            .ok_or(DbError::NotFound(id))
    }

    fn save_workflow(&self, workflow: &Workflow) -> Result<(), DbError> {
        self.workflows.lock().unwrap().insert(workflow.id, workflow.clone());
        Ok(())
    }

    fn fetch_events(&self, workflow_id: WorkflowId) -> Result<Vec<Event>, DbError> {
        Ok(self.events.lock().unwrap()
            .iter()
            .filter(|e| e.workflow_id == workflow_id)
            .cloned()
            .collect())
    }
}
```

**Phase 3: Config Loader Refactoring** (2 days)

**Before** (mixed I/O and logic):
```rust
// src/config/loader.rs:58-83
pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    // I/O
    let content = fs::read_to_string(path)
        .map_err(|e| ConfigError::IoError(e))?;

    // Pure validation
    validate_config_format(&content)?;

    // Pure parsing
    let config = parse_workflow_config(&content)?;

    // State mutation
    CONFIG_CACHE.lock().unwrap().insert(path.to_path_buf(), config.clone());

    Ok(config)
}
```

**After** (separated with Effects):
```rust
// Pure functions (easily testable, no I/O)
fn validate_config_format(content: &str) -> Result<(), ConfigError> {
    // Pure validation logic
    if !content.starts_with("---") {
        return Err(ConfigError::InvalidFormat("Missing YAML frontmatter"));
    }
    Ok(())
}

fn parse_workflow_config(content: &str) -> Result<Config, ConfigError> {
    // Pure parsing logic
    serde_yaml::from_str(content)
        .map_err(ConfigError::YamlError)
}

// Effect composition (I/O at boundaries)
pub fn load_from_path(path: PathBuf) -> Effect<Config, ConfigError, AppEnv> {
    IO::query(move |env| env.fs.read_to_string(&path))
        .map_err(ConfigError::IoError)
        .and_then(|content| {
            // Pure validation and parsing
            validate_config_format(&content)?;
            let config = parse_workflow_config(&content)?;
            Effect::pure(config)
        })
        .context(format!("Loading config from {}", path.display()))
}

// Use at application boundary
fn main() -> Result<()> {
    let env = AppEnv::real();  // Real file system, database, etc.
    let config = load_from_path(PathBuf::from("workflow.yml"))
        .run(&env)?;
    // ...
}
```

**Testing**:
```rust
#[test]
fn test_load_from_path() {
    let env = AppEnv::mock();
    env.fs.add_file("workflow.yml", "---\nname: test\n");

    let config = load_from_path(PathBuf::from("workflow.yml"))
        .run(&env)
        .expect("should load config");

    assert_eq!(config.name, "test");
}

#[test]
fn test_parse_workflow_config() {
    // Pure function - no mocking needed!
    let yaml = "---\nname: test\nsteps: []\n";
    let config = parse_workflow_config(yaml).expect("valid yaml");
    assert_eq!(config.name, "test");
}
```

**Phase 4: Executor Validation Refactoring** (2 days)

**Before** (src/cook/execution/executor.rs:37-66):
```rust
pub async fn validate_request(&self, req: &ExecutionRequest) -> Result<(), ExecutorError> {
    // Pure validation
    if req.command.is_empty() {
        return Err(ExecutorError::EmptyCommand);
    }

    if !self.command_registry.is_registered(&req.command) {
        return Err(ExecutorError::UnknownCommand(req.command.clone()));
    }

    // I/O operation
    self.resource_monitor.validate_limits(&req.resources).await?;

    Ok(())
}
```

**After**:
```rust
// Pure validation (extracted)
fn validate_command_registered(
    cmd: &str,
    registry: &CommandRegistry,
) -> Result<(), ExecutorError> {
    if cmd.is_empty() {
        return Err(ExecutorError::EmptyCommand);
    }

    if !registry.is_registered(cmd) {
        return Err(ExecutorError::UnknownCommand(cmd.to_string()));
    }

    Ok(())
}

// Effect composition
pub fn validate_request(
    req: ExecutionRequest,
    registry: CommandRegistry,
) -> Effect<ValidatedRequest, ExecutorError, AppEnv> {
    // Pure validation first
    Effect::from_result(validate_command_registered(&req.command, &registry))
        // Then I/O validation
        .and_then(|_| {
            IO::query(move |env| env.resource_monitor.validate_limits(&req.resources))
        })
        .map(|_| ValidatedRequest(req))
        .context("Validating execution request")
}
```

**Phase 5: Input Processor Pipeline** (3 days)

**Before** (src/cook/input/processor.rs:53-71):
```rust
pub async fn process_inputs(&self, sources: Vec<InputSource>) -> Result<Vec<ProcessedInput>, Error> {
    let mut results = Vec::new();

    // Loop with async I/O
    for source in sources {
        let data = self.fetch_source(&source).await?;
        let transformed = self.apply_transformations(data)?;
        let validated = self.apply_validation(transformed)?;
        results.push(validated);
    }

    Ok(results)
}
```

**After**:
```rust
// Pure transformation (no I/O)
fn apply_transformations(data: RawInput) -> Result<TransformedInput, Error> {
    // Pure data transformation logic
    let normalized = normalize_input(&data)?;
    let enriched = enrich_metadata(normalized)?;
    Ok(enriched)
}

// Pure validation (no I/O)
fn apply_validation(input: TransformedInput) -> Validation<ValidatedInput, Vec<Error>> {
    Validation::all((
        validate_schema(&input),
        validate_constraints(&input),
        validate_business_rules(&input),
    ))
    .map(|_| ValidatedInput(input))
}

// Effect composition for I/O
fn fetch_source(source: InputSource) -> Effect<RawInput, Error, AppEnv> {
    match source {
        InputSource::File(path) => {
            IO::query(move |env| env.fs.read_to_string(&path))
                .map(RawInput::from_string)
        }
        InputSource::Http(url) => {
            IO::query(move |env| env.http.get(&url))
                .map(RawInput::from_response)
        }
        InputSource::Database(query) => {
            IO::query(move |env| env.db.execute_query(&query))
                .map(RawInput::from_rows)
        }
    }
    .map_err(Error::from)
}

// Declarative pipeline
pub fn process_inputs(
    sources: Vec<InputSource>,
) -> Effect<Vec<ValidatedInput>, Vec<Error>, AppEnv> {
    sources.into_iter()
        .map(|source| {
            fetch_source(source)                          // I/O
                .map(apply_transformations)               // Pure
                .and_then(Effect::from_result)            // Result -> Effect
                .and_then(|t| Effect::from_validation(apply_validation(t)))  // Pure
        })
        .collect::<Effect<Vec<_>, _, _>>()
        .context("Processing input sources")
}
```

**Phase 6: Storage and Git Operations** (2 days)

Wrap existing storage and git operations in Effects:

```rust
// Storage operations
pub fn save_workflow_state(
    workflow: Workflow,
) -> Effect<(), StorageError, AppEnv> {
    IO::execute(move |env| env.db.save_workflow(&workflow))
        .context(format!("Saving workflow {}", workflow.id))
}

pub fn fetch_workflow_history(
    id: WorkflowId,
) -> Effect<Vec<Event>, StorageError, AppEnv> {
    IO::query(move |env| env.db.fetch_events(id))
        .context(format!("Fetching workflow history for {}", id))
}

// Git operations
pub fn create_worktree(
    path: PathBuf,
    branch: String,
) -> Effect<WorktreeHandle, GitError, AppEnv> {
    IO::execute(move |env| {
        env.git.worktree_add(&path, &branch)?;
        Ok(WorktreeHandle { path, branch })
    })
    .context(format!("Creating worktree at {}", path.display()))
}
```

### Architecture Changes

**New Module Structure**:
```
src/
├── env/                    # Environment traits and implementations
│   ├── mod.rs             # Re-exports
│   ├── traits.rs          # FileEnv, DbEnv, GitEnv, ProcessEnv
│   ├── real.rs            # Real implementations
│   ├── mock.rs            # Mock implementations for testing
│   └── app.rs             # Combined AppEnv
├── effects/               # Effect-based operations
│   ├── mod.rs            # Re-exports
│   ├── config.rs         # Config loading effects
│   ├── execution.rs      # Execution effects
│   ├── storage.rs        # Storage effects
│   └── git.rs            # Git effects
└── pure/                  # Pure functions (no I/O)
    ├── validation.rs      # Pure validation logic
    ├── parsing.rs         # Pure parsing logic
    └── transformation.rs  # Pure data transformations
```

**Dependency Flow**:
```
CLI / API
   ↓
Effects (I/O operations)
   ↓
Pure Functions (business logic)
   ↓
Data Structures
```

### Data Structures

**Environment Type**:
```rust
pub struct AppEnv {
    pub fs: Arc<dyn FileEnv + Send + Sync>,
    pub db: Arc<dyn DbEnv + Send + Sync>,
    pub git: Arc<dyn GitEnv + Send + Sync>,
    pub process: Arc<dyn ProcessEnv + Send + Sync>,
}

impl AppEnv {
    pub fn real() -> Self {
        Self {
            fs: Arc::new(RealFileEnv::new()),
            db: Arc::new(RealDbEnv::new()),
            git: Arc::new(RealGitEnv::new()),
            process: Arc::new(RealProcessEnv::new()),
        }
    }

    pub fn mock() -> Self {
        Self {
            fs: Arc::new(MockFileEnv::new()),
            db: Arc::new(MockDbEnv::new()),
            git: Arc::new(MockGitEnv::new()),
            process: Arc::new(MockProcessEnv::new()),
        }
    }
}
```

### APIs and Interfaces

**Effect Construction**:
```rust
use stillwater::{Effect, IO};

// Pure effect (no I/O)
let effect = Effect::pure(value);

// Query effect (read-only I/O)
let effect = IO::query(|env| env.db.fetch_data(id));

// Execute effect (mutating I/O)
let effect = IO::execute(|env| env.fs.write(&path, content));

// From Result
let effect = Effect::from_result(parse_config(&content));

// From Validation
let effect = Effect::from_validation(validate_input(&input));
```

**Effect Composition**:
```rust
// Sequential composition (and_then)
fetch_user(id)
    .and_then(|user| fetch_permissions(user.id))
    .and_then(|perms| authorize_action(action, perms))

// Parallel composition (all)
Effect::all((
    fetch_user(id),
    fetch_settings(id),
    fetch_history(id),
))
.map(|(user, settings, history)| Dashboard { user, settings, history })

// Error context
fetch_data(id)
    .context(format!("Fetching data for user {}", id))
    .map_err(|e| AppError::DataFetchFailed(e))
```

## Dependencies

- **Prerequisites**: Spec 163 (Stillwater Validation Migration)
- **Affected Components**:
  - `src/config/loader.rs` - complete refactoring
  - `src/cook/execution/executor.rs` - validation separation
  - `src/cook/input/processor.rs` - pipeline refactoring
  - `src/storage/` - Effect wrappers
  - `src/worktree/manager.rs` - git Effect wrappers
- **External Dependencies**: `stillwater = "0.1"` (already added in spec 163)

## Testing Strategy

### Unit Tests

**Pure Function Tests** (no mocking needed):
```rust
#[test]
fn test_parse_workflow_config() {
    let yaml = "---\nname: test\nsteps: []\n";
    let config = parse_workflow_config(yaml).expect("valid yaml");
    assert_eq!(config.name, "test");
}

#[test]
fn test_validate_config_format() {
    assert!(validate_config_format("---\nvalid").is_ok());
    assert!(validate_config_format("invalid").is_err());
}

#[test]
fn test_apply_transformations() {
    let raw = RawInput::new("test data");
    let transformed = apply_transformations(raw).expect("valid transform");
    assert_eq!(transformed.normalized, "TEST DATA");
}
```

**Effect Tests** (with mocks):
```rust
#[test]
fn test_load_from_path_success() {
    let env = AppEnv::mock();
    env.fs.add_file("workflow.yml", "---\nname: test\n");

    let config = load_from_path(PathBuf::from("workflow.yml"))
        .run(&env)
        .expect("should load");

    assert_eq!(config.name, "test");
}

#[test]
fn test_load_from_path_file_not_found() {
    let env = AppEnv::mock();

    let result = load_from_path(PathBuf::from("missing.yml"))
        .run(&env);

    assert!(matches!(result, Err(ConfigError::IoError(_))));
}

#[test]
fn test_effect_composition() {
    let env = AppEnv::mock();
    env.db.add_user(User { id: 1, name: "Alice" });

    let dashboard = Effect::all((
            fetch_user(1),
            fetch_settings(1),
        ))
        .map(|(user, settings)| Dashboard { user, settings })
        .run(&env)
        .expect("should compose");

    assert_eq!(dashboard.user.name, "Alice");
}
```

### Integration Tests

- Test Effects with real file system (in temp directory)
- Test database Effects with test database
- Test git Effects with test repository
- Verify Effect pipelines work end-to-end

### Performance Tests

**Benchmark Effect Overhead**:
```rust
#[bench]
fn bench_direct_file_read(b: &mut Bencher) {
    b.iter(|| {
        fs::read_to_string("test.txt").unwrap()
    });
}

#[bench]
fn bench_effect_file_read(b: &mut Bencher) {
    let env = AppEnv::real();
    b.iter(|| {
        IO::query(|e| e.fs.read_to_string(Path::new("test.txt")))
            .run(&env)
            .unwrap()
    });
}
```

Expected: zero overhead due to monomorphization and inlining.

### User Acceptance

- All existing functionality works identically
- Tests run faster due to mocking (no real I/O)
- Code is easier to understand with clear I/O boundaries
- Debugging is easier with explicit effect context

## Documentation Requirements

### Code Documentation

- Document all environment traits with examples
- Add module-level docs for `env/` and `effects/`
- Document Effect composition patterns
- Provide cookbook of common Effect recipes

### User Documentation

No user-facing changes (internal refactoring).

### Architecture Updates

Add to ARCHITECTURE.md:

```markdown
### Effect System

Prodigy uses the stillwater `Effect<T, E, Env>` type to separate pure business logic from I/O:

- **Pure Core**: Functions in `src/pure/` contain no I/O (easily testable)
- **Effect Shell**: Functions in `src/effects/` compose I/O operations
- **Environment**: `AppEnv` provides all I/O capabilities (file, db, git, process)

#### Testing

- **Unit Tests**: Test pure functions directly (no mocking)
- **Integration Tests**: Test Effects with `AppEnv::mock()`
- **E2E Tests**: Test Effects with `AppEnv::real()`

See `src/env/mock.rs` for mock implementations and testing patterns.
```

## Implementation Notes

### Effect Patterns

**Query vs Execute**:
```rust
// Query: read-only, can be cached
let data = IO::query(|env| env.db.fetch(id));

// Execute: mutating, not cached
let _ = IO::execute(|env| env.db.save(data));
```

**Error Handling**:
```rust
// Add context at each step
fetch_user(id)
    .context(format!("Fetching user {}", id))
    .and_then(|user| validate_user(&user))
    .context("Validating user")
    .and_then(|user| save_user(user))
    .context("Saving user")
```

**Combining Effects and Validation**:
```rust
// Validation then Effect
Effect::from_validation(validate_input(&input))
    .and_then(|valid_input| save_input(valid_input))

// Effect then Validation
fetch_data(id)
    .and_then(|data| Effect::from_validation(validate_data(&data)))
```

### Async Compatibility

Stillwater supports async with the `async` feature:

```rust
// Async Effect
pub fn fetch_user_async(id: UserId) -> Effect<User, Error, AppEnv> {
    IO::query_async(|env| async move {
        env.db.fetch_user_async(id).await
    })
}

// Compose async Effects
fetch_user_async(id)
    .and_then_async(|user| fetch_permissions_async(user.id))
    .run_async(&env)
    .await?
```

### Migration Strategy

1. **Start with config loading** (clear I/O boundary)
2. **Move to execution validation** (mixed I/O and pure)
3. **Refactor input processing** (complex pipeline)
4. **Wrap storage operations** (database abstraction)
5. **Wrap git operations** (external command abstraction)

### Common Gotchas

- Environment must be `Send + Sync` for async code
- Effect is lazy - only runs when `.run()` is called
- `.and_then()` is sequential - use `.all()` for parallel
- Mock environments need interior mutability (`Arc<Mutex<>>`)
- Remember to `.context()` every Effect for debugging

## Migration and Compatibility

### Breaking Changes

None - internal refactoring preserving existing APIs.

### Compatibility Layer

Provide compatibility wrappers for gradual migration:

```rust
// Old API (preserves existing code)
pub fn load_from_path_legacy(path: &Path) -> Result<Config, ConfigError> {
    let env = AppEnv::real();
    load_from_path(path.to_path_buf()).run(&env)
}

// New API (Effect-based)
pub fn load_from_path(path: PathBuf) -> Effect<Config, ConfigError, AppEnv> {
    // Effect implementation
}
```

### Rollback Plan

Environment traits are simple wrappers around existing operations. If Effect system causes issues:
1. Keep environment traits for testability
2. Remove Effect wrappers, call environments directly
3. Preserve pure function extractions (valuable regardless)

### Future Work

- Migrate remaining I/O operations to Effects
- Add Effect-based retry strategies
- Implement Effect caching for expensive operations
- Create Effect tracing for debugging
- Add Effect metrics for monitoring
