# MMM Example Workflows

This directory contains example workflow configurations for different improvement scenarios. All workflows use clean YAML format for maximum simplicity and readability.

## Available Workflows

### 1. **custom_workflow.yml** - Default Workflow
The standard MMM improvement workflow:
- Code review → Implementation → Linting
- Balanced approach for general improvements

### 2. **security-workflow.yml** - Security Focus
Security-focused improvements with targeted analysis:
- Security audit with security focus → Implementation → Security test generation → Implementation → Linting
- Thorough security review

### 3. **performance-workflow.yml** - Performance Focus
Performance optimization workflow:
- Performance-focused code review → Implementation → Performance test generation → Implementation → Linting
- Focused on measurable improvements

### 4. **quick-fix-workflow.yml** - Rapid Fixes
Minimal workflow for quick improvements:
- Critical review → Implementation → Linting  
- Optimized for speed

### 5. **mixed-focus-workflow.yml** - Multi-Focus
Demonstrates different focus areas in one workflow:
- Architecture review → Implementation → Performance review → Implementation → Integration tests → Implementation → Linting
- Varied focus per command

### 6. **test-driven-workflow.yml** - Test Coverage
Test-first development approach:
- Coverage analysis → Implementation → Unit test generation → Implementation → Testability review → Implementation → Linting
- Comprehensive testing approach

### 7. **documentation-workflow.yml** - Documentation
Improves project documentation:
- Documentation analysis → Implementation → Readability review → Implementation → API doc generation → Implementation → Linting
- Focused on documentation quality

### 8. **refactoring-workflow.yml** - Code Quality
Architecture and maintainability improvements:
- Architecture review → Implementation → Complexity review → Implementation → Maintainability review → Implementation → Linting
- Deep refactoring approach

### 9. **demo-focus.yml** - Focus Examples
Shows both ways to specify focus arguments:
- YAML object syntax vs string format
- Quick demo format

### 10. **product-enhancement-workflow.yml** - Product Focus
Product management perspective on improvements:
- Product enhancement analysis → Implementation → Linting
- Focus on user value and features over code quality
- Comprehensive product improvements

## Usage

### Using a Workflow

```bash
# Use a specific workflow
mmm improve --config examples/security-workflow.yml

# Combine with other options
mmm improve --config examples/performance-workflow.yml --target 9.0 --verbose

# Use with focus directive
mmm improve --config examples/refactoring-workflow.yml --focus "error-handling"
```

## YAML Format Guide

### Simple Commands
```yaml
commands:
  - mmm-code-review
  - mmm-implement-spec
  - mmm-lint
```

### Focus Arguments (Ansible-style)
```yaml
commands:
  - name: mmm-code-review
    focus: security
  - mmm-implement-spec
  - name: mmm-test-generate
    focus: security
  - mmm-implement-spec
  - mmm-lint
```

### String Format (Alternative)
```yaml
commands:
  - mmm-code-review --focus security
  - mmm-implement-spec
  - mmm-lint
```

### Mixed Format
```yaml
commands:
  - mmm-code-review  # Simple string
  - name: mmm-test-generate  # YAML object with focus
    focus: integration
  - mmm-implement-spec  # Simple string
  - mmm-lint --verbose  # String with flags
```

## Available Claude Commands

- `mmm-code-review` - General code analysis
- `mmm-implement-spec` - Implement generated specs
- `mmm-lint` - Format, lint, and test
- `mmm-security-audit` - Security analysis
- `mmm-test-generate` - Create tests
- `mmm-coverage-analysis` - Check test coverage
- `mmm-doc-analysis` - Documentation review
- `mmm-doc-generate` - Generate documentation
- `mmm-product-enhance` - Product management perspective

## Focus Areas

Use focus arguments to target specific aspects:

- `security` - Security vulnerabilities and hardening
- `performance` - Speed and resource optimization
- `testing` - Test coverage and quality
- `documentation` - Code documentation and readability
- `architecture` - Design patterns and structure
- `complexity` - Code simplicity and maintainability
- `critical` - High-priority issues only
- `integration` - Integration and end-to-end testing
- `unit-tests` - Unit testing specifically
- `api-docs` - API documentation
- `readability` - Code clarity and style
- `testability` - Making code more testable
- `onboarding` - First-run experience and getting started
- `api` - Developer experience and API design
- `cli-ux` - Command ergonomics and user experience
- `features` - Missing functionality and enhancements

## Best Practices

1. **Start Simple**: Use `quick-fix-workflow.yml` for initial testing
2. **Focus Areas**: Use specific focus arguments for targeted improvements
3. **Iteration Control**: Use the `--max-iterations` CLI flag to control the number of improvement cycles
4. **Command Order**: Place analysis commands before implementation
5. **Always Lint**: End workflows with `mmm-lint` for quality assurance
6. **YAML Syntax**: Use YAML object format when you need focus, string format otherwise

## Workflow Selection Guide

- **New Project**: Start with `custom_workflow.yml`
- **Security Concerns**: Use `security-workflow.yml`
- **Performance Issues**: Try `performance-workflow.yml`
- **Low Test Coverage**: Use `test-driven-workflow.yml`
- **Poor Documentation**: Apply `documentation-workflow.yml`
- **Legacy Code**: Use `refactoring-workflow.yml`
- **CI/CD Pipeline**: Use `quick-fix-workflow.yml`
- **Multiple Focus Areas**: Use `mixed-focus-workflow.yml`
- **User Experience**: Use `product-enhancement-workflow.yml`

## Parallel Execution

Run multiple workflows simultaneously using worktrees:

```bash
# Start security improvements in parallel
mmm improve --config examples/security-workflow.yml --worktree

# Start performance improvements in another session
mmm improve --config examples/performance-workflow.yml --worktree

# Later, merge the improvements
mmm worktree merge --all
```

All examples are designed to be dead simple and immediately usable!