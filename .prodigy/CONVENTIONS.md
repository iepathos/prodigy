# Prodigy Coding Conventions

## Rust Style Guidelines

### Functional Programming First
- **Pure functions over stateful methods**: Extract business logic into pure functions that take inputs and return outputs with no side effects
- **Immutability by default**: Use `&` references and return new data rather than mutating in place
- **Function composition**: Build complex behavior by composing simple, testable units
- **Separation of concerns**: Keep I/O at module boundaries, pure logic in the core

### Code Structure
- **Maximum function length**: 20 lines (prefer 5-10)
- **Single responsibility**: Each function does one thing well
- **Clear naming**: Functions and variables should be self-documenting
- **Error handling**: Use `Result<T, E>` and fail fast with descriptive messages

### Module Organization
- **Domain-driven structure**: Group by feature/capability, not by file type
- **Clear public APIs**: Export only what's needed, hide implementation details
- **Dependency direction**: UI → Domain → Infrastructure
- **No circular dependencies**: Maintain acyclic dependency graph

## Rust-Specific Best Practices

### Data Types
- Prefer `Vec<T>` over arrays for dynamic collections
- Use `HashMap<K, V>` for key-value mappings
- Use `Option<T>` instead of nullability patterns
- Use `Result<T, E>` for error-prone operations

### Async/Await
- Use `async fn` for I/O operations
- Use `tokio::test` for async tests
- Avoid blocking operations in async contexts
- Use `futures::stream` for stream processing

### Error Handling
- **Use ProdigyError**: Primary error type for all modules
- **Error codes**: Use ErrorCode constants for structured errors
- **User messages**: Provide clear, actionable user messages
- **Context chaining**: Add context with `.with_context()` and `.with_source()`
- **Conversion helpers**: Use ErrorExt trait for clean conversions
- **Recovery patterns**: Check `.is_recoverable()` for retry logic
- **Migration**: Use helpers module for gradual migration
- Propagate errors with `?` operator
- **Avoid unwrap()**: Use `.expect()` with descriptive message for impossible errors
- **Prefer map_or()**: Replace `.is_none() || .unwrap()` patterns with `.map_or(true, |v| ...)`
- **Add context**: Always provide error context for debugging: `.with_context("Operation failed")`
- **Error recovery**: Use match for recoverable errors, `?` for critical ones

### Testing
- Unit tests in same file with `#[cfg(test)]`
- Integration tests in `tests/` directory
- Use `mockall` for mocking traits
- Test behavior, not implementation details

### Performance
- Use `&str` over `String` when possible
- Clone only when necessary
- Use iterators over loops for data transformation
- Profile before optimizing

## Code Quality Standards

### Formatting
- Use `cargo fmt` with default settings
- Line length: 100 characters
- Use consistent indentation (4 spaces)

### Linting
- Pass `cargo clippy -- -D warnings`
- Address all clippy suggestions
- Use `#[allow(clippy::xxx)]` only when justified with comment

### Documentation
- Document all public functions and types
- Use examples in documentation
- Keep inline comments minimal and meaningful
- Update module-level docs when adding features

### Commit Standards
- Keep commits atomic and focused
- Use conventional commits format: `feat:`, `fix:`, `refactor:`, etc.
- Include context in commit body when needed
- Run tests before committing

## Architecture Patterns

### Command/Query Separation
- Commands modify state, return `Result<(), Error>`
- Queries return data, never modify state
- Use traits to define interfaces

### Dependency Injection
- Pass dependencies through constructors
- Use trait objects for testability
- Avoid global state and singletons

### Event-Driven Design
- Use channels for async communication
- Emit events for cross-cutting concerns
- Keep event handling pure when possible

### Configuration
- Use `serde` for serialization/deserialization
- Validate configuration at startup
- Use builder pattern for complex configurations

## Project-Specific Patterns

### Command Execution
- Use `CommandExecutor` trait for testability
- Separate command parsing from execution
- Use `ExecutionContext` for environment state

### File Operations
- Use `std::path::Path` and `PathBuf`
- Handle file system errors gracefully
- Use `tempfile` for temporary files in tests

### Workflow Management
- Keep workflow steps atomic
- Use proper error propagation
- Support both sync and async execution

### Goal-Seeking Implementation
- Implement `Validator` trait for custom validators
- Use `ValidationResult` for scoring
- Keep validation logic pure and testable
- Support both JSON and text score formats

These conventions ensure consistency, maintainability, and alignment with Rust best practices while supporting the functional programming paradigm preferred in this codebase.