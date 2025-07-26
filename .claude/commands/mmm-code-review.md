# /mmm-code-review

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

## Structured Output for Automation

**CRITICAL**: When invoked in automation mode, always end response with this exact JSON structure:

```json
{
  "mmm_structured_output": {
    "review_id": "review-{{ timestamp }}-{{ uuid }}",
    "timestamp": "{{ current_timestamp }}",
    "overall_score": {{ calculated_score }},
    "scope": "{{ analyzed_scope }}",
    "actions": [
      {
        "id": "action_{{ sequence }}",
        "type": "fix_error|improve_code|improve_performance|fix_style|add_tests|refactor",
        "severity": "critical|high|medium|low", 
        "file": "{{ file_path }}",
        "line": {{ line_number }},
        "line_range": [{{ start_line }}, {{ end_line }}],
        "title": "{{ brief_description }}",
        "description": "{{ detailed_explanation }}",
        "suggestion": "{{ specific_fix_recommendation }}",
        "automated": {{ true_if_automatable }},
        "estimated_effort": "{{ time_estimate }}",
        "category": "{{ issue_category }}",
        "impact": "{{ business_impact }}"
      }
    ],
    "summary": {
      "total_issues": {{ total_count }},
      "critical": {{ critical_count }},
      "high": {{ high_count }},
      "medium": {{ medium_count }},
      "low": {{ low_count }},
      "automated_fixes": {{ automatable_count }},
      "manual_fixes": {{ manual_count }},
      "compilation_errors": {{ error_count }},
      "test_failures": {{ test_failure_count }},
      "clippy_warnings": {{ warning_count }}
    },
    "metrics": {
      "code_complexity": {{ complexity_score }},
      "test_coverage": {{ coverage_percentage }},
      "technical_debt_ratio": {{ debt_ratio }},
      "maintainability_index": {{ maintainability_score }}
    },
    "recommendations": {
      "next_iteration_focus": "{{ focus_area }}",
      "architecture_improvements": ["{{ suggestion_1 }}", "{{ suggestion_2 }}"],
      "priority_actions": ["{{ action_id_1 }}", "{{ action_id_2 }}"]
    }
  }
}
```

**Automation Detection**: The command detects automation mode when:
- Invoked with `--format=json` parameter
- Environment variable `MMM_AUTOMATION=true` is set
- Called from within an MMM workflow context

## Output Format

The review provides structured, machine-parseable output for automation:

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

4. **Actionable Feedback (JSON Structure)**
   ```json
   {
     "review_id": "uuid",
     "timestamp": "2025-01-26T10:30:00Z",
     "overall_score": 8.2,
     "actions": [
       {
         "id": "action_001",
         "type": "fix_error",
         "severity": "critical",
         "file": "src/parser.rs",
         "line": 45,
         "title": "Fix compilation error",
         "description": "Missing semicolon causing compilation failure",
         "suggestion": "Add semicolon at end of line 45",
         "automated": true,
         "estimated_effort": "1min"
       },
       {
         "id": "action_002", 
         "type": "improve_performance",
         "severity": "medium",
         "file": "src/query.rs",
         "line_range": [23, 35],
         "title": "Replace manual loop with iterator",
         "description": "Manual loop can be replaced with more idiomatic iterator chain",
         "suggestion": "Use .filter().map().collect() pattern",
         "automated": true,
         "estimated_effort": "5min"
       },
       {
         "id": "action_003",
         "type": "add_tests",
         "severity": "high", 
         "file": "src/database.rs",
         "title": "Add unit tests for error handling",
         "description": "Function lacks test coverage for error conditions",
         "automated": false,
         "estimated_effort": "15min"
       }
     ],
     "summary": {
       "total_issues": 15,
       "critical": 1,
       "high": 4,
       "medium": 8,
       "low": 2,
       "automated_fixes": 12,
       "manual_fixes": 3
     }
   }
   ```

5. **Integration Commands**
   ```bash
   # To automatically apply fixes from this review:
   /mmm-improve --review-id uuid --severity critical,high
   
   # To iterate until all issues resolved:
   /mmm-iterate --target-score 9.0 --max-iterations 3
   ```

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
/mmm-code-review
/mmm-code-review "src/parser"
/mmm-code-review "src/parser/inventory.rs src/parser/manifest.rs"
/mmm-code-review "tests"
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