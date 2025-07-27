# MMM Example Workflows

This directory contains example workflow configurations for different improvement scenarios. Each workflow demonstrates a different approach to automated code improvement using MMM's Claude commands.

## Available Workflows

### 1. **custom_workflow.toml** - Default Workflow
The standard MMM improvement workflow:
- Code review → Implementation → Linting
- Balanced approach for general improvements
- 10 iterations maximum

### 2. **refactoring-workflow.toml** - Code Structure
Focuses on improving code organization:
- Refactoring → Implementation → Test generation → Test implementation → Linting
- Improves maintainability and structure
- 5 iterations for focused refactoring

### 3. **security-workflow.toml** - Security Hardening
Comprehensive security improvements:
- Security audit → Fixes → Security test generation → Test implementation → Linting
- Identifies and fixes vulnerabilities
- 8 iterations for thorough security review

### 4. **performance-workflow.toml** - Speed Optimization
Targets performance bottlenecks:
- Performance analysis → Optimization → Verification → Linting
- Includes performance verification step
- 6 iterations for measurable improvements

### 5. **test-driven-workflow.toml** - Test Coverage
Test-first development approach:
- Test generation → Implementation → Coverage check → Testability review → Improvements → Linting
- Ensures comprehensive test coverage
- 7 iterations for thorough testing

### 6. **documentation-workflow.toml** - Documentation
Improves project documentation:
- Doc generation → Implementation → Example generation → Example implementation → Review → Final improvements
- Creates comprehensive documentation
- 4 iterations focused on docs

### 7. **full-stack-workflow.toml** - Comprehensive
Combines multiple improvement strategies:
- Full cycle including review, refactoring, testing, performance, and security
- Most thorough improvement process
- 3 iterations (each very comprehensive)

### 8. **quick-fix-workflow.toml** - Rapid Fixes
Minimal workflow for quick improvements:
- Critical review → Implementation → Linting
- Fast turnaround for CI/CD
- 3 iterations for speed

## Usage

### Using a Workflow

```bash
# Use a specific workflow
mmm improve --config examples/security-workflow.toml

# Combine with other options
mmm improve --config examples/performance-workflow.toml --target 9.0 --verbose

# Use with focus directive
mmm improve --config examples/refactoring-workflow.toml --focus "error-handling"
```

### Creating Custom Workflows

1. Copy an existing workflow as a template
2. Modify the `commands` array with desired Claude commands
3. Adjust `max_iterations` based on your needs
4. Save as `.toml` file

Example custom workflow:

```toml
# my-custom-workflow.toml
commands = [
    "mmm-cleanup-tech-debt",
    "mmm-implement-spec",
    "mmm-refactor naming",
    "mmm-implement-spec",
    "mmm-lint"
]
max_iterations = 5
```

## Available Claude Commands

- `mmm-code-review` - General code analysis
- `mmm-implement-spec` - Implement generated specs
- `mmm-lint` - Format, lint, and test
- `mmm-refactor` - Improve code structure
- `mmm-test-generate` - Create tests
- `mmm-performance` - Optimize performance
- `mmm-security-audit` - Security analysis
- `mmm-docs-generate` - Generate documentation
- `mmm-coverage` - Check test coverage
- `mmm-cleanup-tech-debt` - Remove technical debt
- `mmm-debug` - Debug issues
- `mmm-commit-changes` - Commit with message
- `mmm-merge-worktree` - Merge parallel work

## Best Practices

1. **Start Simple**: Use `quick-fix-workflow.toml` for initial testing
2. **Focus Areas**: Combine workflows with `--focus` flag for targeted improvements
3. **Iteration Count**: Lower iterations for focused workflows, higher for comprehensive ones
4. **Command Order**: Place analysis commands before implementation
5. **Always Lint**: End workflows with `mmm-lint` for quality assurance

## Workflow Selection Guide

- **New Project**: Start with `full-stack-workflow.toml`
- **Legacy Code**: Use `refactoring-workflow.toml`
- **Production Issues**: Apply `security-workflow.toml`
- **Slow Performance**: Try `performance-workflow.toml`
- **Low Test Coverage**: Use `test-driven-workflow.toml`
- **Poor Documentation**: Apply `documentation-workflow.toml`
- **CI/CD Pipeline**: Use `quick-fix-workflow.toml`
- **General Improvements**: Default `custom_workflow.toml`

## Parallel Execution

Run multiple workflows simultaneously using worktrees:

```bash
# Start security improvements in parallel
mmm improve --config examples/security-workflow.toml --worktree

# Start performance improvements in another session
mmm improve --config examples/performance-workflow.toml --worktree

# Later, merge the improvements
mmm worktree merge --all
```