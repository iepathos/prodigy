# CONVENTIONS.md - Simple Coding Standards

## Rust Conventions

### Naming
- **Modules**: snake_case (e.g., `simple_state`)
- **Types**: PascalCase (e.g., `ProjectAnalyzer`)
- **Functions**: snake_case (e.g., `analyze_project`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_TARGET`)

### Code Organization
- Keep modules small and focused
- Public API at top of file
- Private implementations below
- Tests in separate modules

### Error Handling
- Use `anyhow::Result<T>` throughout
- Add context with `.context()`
- Fail fast with clear messages
- Never use `unwrap()` in production code

## Project Structure

### Directory Layout
```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── improve/             # Core improvement logic
├── analyzer/            # Project analysis
└── simple_state/        # Minimal state management
```

### File Naming
- Descriptive names over brevity
- Group related functionality
- Avoid generic names like `utils.rs`

## Git Conventions

### Commit Messages
Format: `type: description`

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `refactor`: Code restructuring
- `test`: Test additions
- `chore`: Maintenance

Example: `feat: add real Claude CLI integration`

## API Design

### Public APIs
- Minimize public surface
- Use simple types over complex builders
- Return `Result<T, Error>`
- Take `&self` when possible

### CLI Design
- Single command: `mmm improve`
- Minimal flags: `--target`, `--verbose`
- Smart defaults
- Clear error messages

## Performance Guidelines

### General Rules
- Favor simplicity over optimization
- Use appropriate data structures
- Cache expensive operations (project analysis)
- Minimize Claude CLI calls

### File Operations
- Always backup before modifying
- Use atomic writes for state
- Validate changes before applying

## Development Practices

### Testing
- Unit tests for core logic
- Integration tests for Claude CLI interaction
- Test error conditions
- Use realistic test data

### Documentation
- Focus on "why" not "what"
- Document public APIs
- Include usage examples
- Keep README up to date

### Code Review
- Prioritize working over perfect
- Check error handling
- Verify Claude CLI integration
- Test on real projects

## Philosophy

1. **Simple > Complex**: Choose simple solutions
2. **Working > Perfect**: Make it work first  
3. **Clear > Clever**: Obvious code over clever tricks
4. **Users > Features**: User value over feature count
5. **Real > Simulated**: Actual Claude integration over mocking