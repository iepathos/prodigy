# Documentation Generation Command

Generates comprehensive documentation for undocumented or poorly documented code.

## Usage

```
/mmm-docs-generate [target]
```

Examples:
- `/mmm-docs-generate` - Generate docs for all undocumented code
- `/mmm-docs-generate api` - Focus on API documentation
- `/mmm-docs-generate examples` - Generate usage examples
- `/mmm-docs-generate architecture` - Create architecture docs

## What This Command Does

1. **Documentation Analysis**
   - Identifies undocumented public APIs
   - Finds missing module documentation
   - Detects outdated documentation
   - Evaluates documentation quality

2. **Content Generation**
   - Creates comprehensive API docs
   - Generates usage examples
   - Documents design decisions
   - Adds inline code comments

3. **Documentation Spec**
   - Commits documentation plan
   - Organized by priority
   - Follows project conventions
   - Ready for implementation

## Documentation Types

- **API Documentation**: Function signatures, parameters, returns
- **Module Documentation**: Purpose, architecture, dependencies
- **Usage Examples**: Common use cases, code snippets
- **Architecture Docs**: System design, data flow
- **Getting Started**: Installation, quick start guides
- **Contributing**: Development setup, guidelines

## Documentation Standards

- Clear and concise language
- Code examples for complex features
- Diagrams for architecture
- Versioning information
- Cross-references
- Search-friendly structure

## Output Format

Generates and commits:

```
docs: generate documentation spec for {target} docs-{timestamp}
```

## Best Practices

1. Document the "why" not just "what"
2. Include examples for every public API
3. Keep docs next to code
4. Use consistent formatting
5. Update docs with code changes