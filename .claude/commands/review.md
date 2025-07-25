# /review

Conduct a comprehensive code review of the current project or specified components. This command performs static analysis, identifies potential issues, ensures code quality standards are met, and provides detailed feedback on implementation patterns.

## Variables

SCOPE: $ARGUMENTS (optional - specify scope like "src/parser", "tests", specific files, or omit for entire codebase)

## Execute

### Phase 1: Project Context and Scope Analysis

1. **Read Project Context**
   - Read .mmm context files (PROJECT.md, ARCHITECTURE.md, CONVENTIONS.md, DECISIONS.md)
   - Understand project goals, architecture patterns, and coding standards
   - Identify recently completed specifications from ROADMAP.md

2. **Determine Review Scope**
   - If SCOPE specified: Focus on specific files/directories
   - If no SCOPE: Review recent changes since last major commit
   - Prioritize areas with recent modifications or new implementations
   - Check git status for uncommitted changes

### Phase 2: Static Analysis and Code Quality

1. **Compilation and Basic Checks**
   - Run `cargo check` to ensure code compiles cleanly
   - Execute `cargo clippy --all-targets --all-features` for lint analysis
   - Check `cargo fmt --check` for formatting consistency
   - Identify any compiler warnings or errors

2. **Code Structure Review**
   - Analyze module organization and dependency structure
   - Review public API design and interface consistency
   - Check adherence to architectural patterns from ARCHITECTURE.md
   - Validate separation of concerns and modularity

3. **Security and Safety Analysis**
   - Identify unsafe code blocks and validate their necessity
   - Review error handling patterns and panic conditions
   - Check for potential security vulnerabilities
   - Analyze input validation and boundary conditions
   - Review memory safety and resource management

### Phase 3: Implementation Quality Assessment

1. **Code Pattern Analysis**
   - Review Rust idioms and best practices usage
   - Check proper use of ownership, borrowing, and lifetimes
   - Analyze iterator usage vs manual loops
   - Validate error handling patterns (Result vs Option)
   - Review async/await patterns if applicable

2. **Performance Considerations**
   - Identify potential performance bottlenecks
   - Review allocation patterns and unnecessary clones
   - Check for inefficient algorithms or data structures
   - Analyze hot paths and optimization opportunities
   - Review concurrent code for race conditions

3. **Maintainability Review**
   - Assess code readability and complexity
   - Review naming conventions and documentation
   - Check for code duplication and refactoring opportunities
   - Analyze function and module size and cohesion
   - Review test coverage and quality

### Phase 4: Architecture and Design Review

1. **Design Pattern Compliance**
   - Verify adherence to project architecture from ARCHITECTURE.md
   - Check design pattern implementation (Factory, Builder, etc.)
   - Review abstraction levels and interface design
   - Validate dependency injection and inversion patterns

2. **API Design Review**
   - Analyze public interface consistency and usability
   - Review method signatures and return types
   - Check for breaking changes in public APIs
   - Validate documentation completeness for public APIs

3. **Integration Points**
   - Review external dependency usage
   - Check integration patterns with other modules
   - Analyze database/storage interaction patterns
   - Review configuration and environment handling

### Phase 5: Testing and Documentation Review

1. **Test Quality Assessment**
   - Review unit test coverage and quality
   - Check integration test completeness
   - Analyze test naming and organization
   - Verify test data and mock quality
   - Review benchmark tests if present

2. **Documentation Review**
   - Check inline documentation (rustdoc) completeness
   - Review code comments for clarity and necessity
   - Validate README and project documentation
   - Check for outdated or incorrect documentation
   - Review example code in documentation

### Phase 6: Specification Compliance

1. **Requirements Verification**
   - Cross-reference implementation against specifications
   - Verify all acceptance criteria are met
   - Check for feature completeness and correctness
   - Validate business logic implementation

2. **Convention Adherence**
   - Verify compliance with CONVENTIONS.md standards
   - Check consistent code style across the project
   - Review naming conventions and structure
   - Validate commit message format and git practices

### Phase 7: Recommendations and Action Items

1. **Issue Categorization**
   - **Critical**: Security issues, compilation errors, broken functionality
   - **High**: Performance issues, architectural violations, missing tests
   - **Medium**: Code quality improvements, minor design issues
   - **Low**: Style improvements, documentation updates

2. **Improvement Suggestions**
   - Specific code refactoring recommendations
   - Performance optimization opportunities
   - Architecture improvements
   - Testing gaps and suggestions
   - Documentation improvements

3. **Action Plan**
   - Prioritized list of issues to address
   - Suggested implementation order
   - Potential breaking changes to consider
   - Long-term architectural considerations

## Review Criteria

### Code Quality Standards
- **Correctness**: Code works as intended without bugs
- **Readability**: Code is clear and self-documenting
- **Maintainability**: Code is easy to modify and extend
- **Performance**: Code meets performance requirements
- **Security**: Code is free from security vulnerabilities

### Rust-Specific Criteria
- **Memory Safety**: Proper ownership and borrowing patterns
- **Error Handling**: Comprehensive Result/Option usage
- **Concurrency**: Safe concurrent code patterns
- **Idioms**: Proper use of Rust language features
- **Dependencies**: Appropriate crate selection and usage

### Architecture Compliance
- **Modularity**: Proper separation of concerns
- **Abstraction**: Appropriate abstraction levels
- **Dependencies**: Clean dependency management
- **Interfaces**: Well-designed public APIs
- **Patterns**: Consistent design pattern usage

## Output Format

The review provides:

1. **Executive Summary**
   - Overall code quality assessment
   - Critical issues requiring immediate attention
   - General recommendations for improvement

2. **Detailed Findings**
   - File-by-file analysis with specific issues
   - Code snippets with suggested improvements
   - Cross-references to relevant specifications

3. **Metrics and Statistics**
   - Code complexity metrics
   - Test coverage statistics
   - Performance benchmark results
   - Lint and warning counts

4. **Action Items**
   - Prioritized list of improvements
   - Estimated effort for each item
   - Dependencies between improvements
   - Timeline recommendations

## Integration with Development Workflow

### Pre-merge Reviews
- Validate changes before merging to main branch
- Ensure new code meets quality standards
- Check for regressions and breaking changes
- Verify specification compliance

### Regular Health Checks
- Periodic comprehensive reviews
- Architecture drift detection
- Technical debt assessment
- Performance regression monitoring

### Specification Validation
- Post-implementation specification reviews
- Acceptance criteria verification
- Documentation synchronization
- Context file updates validation

## Example Usage

```
/review
/review "src/parser"
/review "src/parser/inventory.rs src/parser/manifest.rs"
/review "tests"
```

## Advanced Features

### Git Integration
- Focus review on recent commits or specific commit ranges
- Compare implementation against previous versions
- Identify code churn and stability metrics
- Track technical debt over time

### Custom Review Profiles
- Different review criteria for different project phases
- Specialized reviews for performance-critical code
- Security-focused reviews for sensitive components
- API stability reviews for public interfaces

### Automated Suggestions
- Generate specific code improvement suggestions
- Provide refactoring recommendations with examples
- Suggest performance optimizations
- Recommend additional test cases

## Quality Gates

### Minimum Standards
- All code must compile without warnings
- Critical clippy lints must be addressed
- All tests must pass consistently
- Public APIs must be documented

### Best Practice Enforcement
- Follow established architecture patterns
- Maintain consistent error handling
- Use appropriate Rust idioms
- Maintain good test coverage

### Continuous Improvement
- Track code quality metrics over time
- Identify recurring issue patterns
- Suggest process improvements
- Monitor architectural health