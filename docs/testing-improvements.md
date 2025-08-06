# Testing Infrastructure Improvements

## Current Mock Implementation Issues

After implementing spec 52 and attempting to add comprehensive unit tests, several issues with the mock infrastructure were identified:

### 1. Trait Duplication and Confusion

**Problem**: Multiple traits with the same name but different interfaces exist in different modules:
- `SessionManager` exists in both `src/session/manager.rs` and `src/cook/session/mod.rs` with completely different method signatures
- This causes confusion when implementing mocks as it's unclear which trait to implement

**Solution**: 
- Use fully qualified trait names when implementing mocks
- Consider consolidating duplicate traits or renaming them to be more specific (e.g., `CookSessionManager` vs `GeneralSessionManager`)

### 2. Trait Method Signature Mismatches

**Problem**: When creating mocks, developers often implement the wrong methods:
- `ClaudeExecutor` requires `execute_claude_command`, not `execute_command`
- `UserInteraction` requires `prompt_yes_no`, not `confirm`
- `AnalysisCoordinator` requires `analyze_project`, not `analyze`

**Solution**: 
- Create a comprehensive mock module with correct implementations
- Use builder patterns to make mock creation easier
- Add documentation showing the correct trait methods

### 3. Complex Type Dependencies

**Problem**: Many traits depend on complex types that are difficult to mock:
- `ProjectMetrics` vs `ImprovementMetrics`
- `AnalysisResult` with many nested structures
- Different coverage and analysis types

**Solution**:
- Create factory methods for common test data structures
- Use `Default` implementations where possible
- Provide pre-configured mock instances for common scenarios

## Recommended Mock Infrastructure

### Core Mock Module Structure

```rust
src/testing/mocks/
├── mod.rs           # Re-exports all mocks
├── claude.rs        # Claude CLI mocks
├── git.rs          # Git operation mocks
├── subprocess.rs   # Subprocess mocks
├── fs.rs           # File system mocks
└── builders/       # Builder patterns for complex mocks
    ├── mod.rs
    ├── analysis.rs  # Analysis result builders
    ├── metrics.rs   # Metrics builders
    └── session.rs   # Session state builders
```

### Mock Implementation Pattern

For each trait that needs mocking:

1. **Create a Mock struct** with configurable behavior:
```rust
pub struct MockTraitName {
    responses: Arc<Mutex<Vec<Response>>>,
    behavior: Arc<Mutex<MockBehavior>>,
    recorded_calls: Arc<Mutex<Vec<CallInfo>>>,
}
```

2. **Implement a Builder** for easy configuration:
```rust
pub struct MockTraitNameBuilder {
    responses: Vec<Response>,
    behavior: MockBehavior,
}

impl MockTraitNameBuilder {
    pub fn with_response(mut self, response: Response) -> Self { ... }
    pub fn with_failure(mut self) -> Self { ... }
    pub fn build(self) -> MockTraitName { ... }
}
```

3. **Provide helper methods** for common scenarios:
```rust
impl MockTraitName {
    pub fn success() -> Self { ... }
    pub fn failure() -> Self { ... }
    pub fn unavailable() -> Self { ... }
}
```

## Best Practices for Mock Usage

### 1. Use Builders for Complex Scenarios

```rust
let mock_claude = MockClaudeExecutor::builder()
    .with_success("First command output")
    .with_failure("Second command fails", 1)
    .with_success("Third command succeeds")
    .build();
```

### 2. Verify Mock Interactions

```rust
// After test execution
assert_eq!(mock.get_call_count(), 3);
let calls = mock.get_recorded_calls();
assert_eq!(calls[0].command, "/mmm-code-review");
```

### 3. Use Type-Safe Mock Responses

Instead of using strings or generic types, create specific response types:

```rust
pub enum MockResponse<T> {
    Success(T),
    Failure { message: String, code: i32 },
    Timeout,
    Unavailable,
}
```

## Integration Testing Considerations

While unit tests with mocks are valuable, integration tests remain important:

1. **Keep integration tests in separate directory**: `tests/integration/`
2. **Use real implementations where possible** in integration tests
3. **Mock only external dependencies** (Claude CLI, network, etc.)
4. **Test actual workflows end-to-end** when feasible

## Future Improvements

1. **Auto-generate mocks** from trait definitions using procedural macros
2. **Create a mock registry** for sharing mock configurations across tests
3. **Add mock validation** to ensure mocks behave like real implementations
4. **Implement property-based testing** for mock behavior verification
5. **Create test fixtures** for common project structures and states

## Conclusion

The current mock infrastructure works but has room for improvement. The main issues are:
- Trait confusion due to duplicates
- Complex type dependencies
- Lack of comprehensive mock implementations

By following the patterns and practices outlined above, we can create a more maintainable and user-friendly testing infrastructure that makes it easier to achieve high test coverage without fighting the type system.