# MMM Development Conventions

## Code Style
- Follow Rust standard conventions (rustfmt)
- Use `cargo clippy` for linting
- Prefer explicit error handling with Result<T, Error>
- Document public APIs with rustdoc comments

## Naming Conventions
- Module names: lowercase with underscores (e.g., `project_manager`)
- Struct/Enum names: PascalCase (e.g., `ProjectConfig`)
- Function names: snake_case (e.g., `load_project`)
- Constants: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_TIMEOUT`)

## Error Handling
- Use custom error types with thiserror
- Provide context with error messages
- Use anyhow for application-level errors
- Implement graceful degradation where possible

## Testing
- Unit tests in same file as implementation
- Integration tests in `tests/` directory
- Use `#[cfg(test)]` for test modules
- Aim for >80% code coverage

## Documentation
- Every public item must have documentation
- Use examples in doc comments
- Keep README.md up to date
- Document architectural decisions in ADRs

## Git Conventions
- Commit messages: "type: description" format
  - feat: new feature
  - fix: bug fix
  - docs: documentation changes
  - refactor: code refactoring
  - test: test additions/changes
  - chore: maintenance tasks
- Branch names: feature/description or fix/description
- PR titles should match commit message format

## Specification Format
- Markdown files with YAML frontmatter
- Clear acceptance criteria
- Technical details section
- Implementation notes where relevant

## Database Conventions
- Use migrations for schema changes
- Index frequently queried columns
- Use transactions for multi-step operations
- Keep queries simple and readable