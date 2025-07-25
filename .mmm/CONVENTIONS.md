# CONVENTIONS.md - Coding Standards

## Rust Conventions

### Naming
- **Modules**: snake_case (e.g., `spec_engine`)
- **Types**: PascalCase (e.g., `WorkflowEngine`)
- **Functions**: snake_case (e.g., `parse_specification`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `MAX_RETRIES`)

### Code Organization
- One module per file
- Public API at top of file
- Private implementations below
- Tests in separate `tests` module

### Error Handling
- Use custom error types
- Implement `From` for error conversions
- Add context with `.context()`
- Never use `unwrap()` in production code

### Documentation
- Document all public APIs
- Use `///` for item docs
- Use `//!` for module docs
- Include examples in doc comments

## Project Structure

### Directory Layout
```
src/
├── module/
│   ├── mod.rs      # Module declaration and exports
│   ├── types.rs    # Type definitions
│   ├── impl.rs     # Implementation
│   └── tests.rs    # Unit tests
```

### File Naming
- Descriptive names over brevity
- Group related functionality
- Avoid generic names like `utils.rs`

## Git Conventions

### Commit Messages
Format: `type(scope): description`

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Test additions
- `chore`: Maintenance

Example: `feat(workflow): add checkpoint recovery`

### Branch Naming
- `feature/description`
- `fix/issue-description`
- `refactor/what-is-changing`

## Testing Conventions

### Test Organization
- Unit tests next to code
- Integration tests in `tests/`
- Use descriptive test names
- Test one concept per test

### Test Naming
```rust
#[test]
fn test_parse_valid_specification() { }

#[test]
fn test_parse_invalid_yaml_returns_error() { }
```

## API Design

### Public APIs
- Minimize public surface
- Use builder pattern for complex types
- Return `Result<T, Error>`
- Take `&self` when possible

### Async Conventions
- Use `async/await` throughout
- Name async functions clearly
- Handle cancellation properly
- Avoid blocking operations

## Documentation Standards

### Code Comments
- Explain "why" not "what"
- Document complex algorithms
- Note assumptions
- Reference specifications

### API Documentation
- Start with brief description
- Include usage examples
- Document error conditions
- List panics if any

## Performance Guidelines

### General Rules
- Measure before optimizing
- Prefer clarity over cleverness
- Use appropriate data structures
- Minimize allocations

### Specific Practices
- Use `&str` over `String` when possible
- Prefer iterators over collecting
- Use `Cow` for conditional ownership
- Profile hot paths

## Security Practices

### Input Validation
- Validate all external input
- Use type system for constraints
- Sanitize file paths
- Limit resource consumption

### Sensitive Data
- Never log secrets
- Clear sensitive data after use
- Use secure random generation
- Follow principle of least privilege