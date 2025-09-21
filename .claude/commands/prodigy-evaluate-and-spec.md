# /prodigy-evaluate-and-spec

Perform a comprehensive evaluation of Prodigy's current functionality, identify technical debt, implementation gaps, and areas for improvement, then automatically generate detailed specifications for each identified issue.

## Variables

None required - this command performs a full system evaluation

## Execute

### Phase 1: Comprehensive Codebase Analysis

1. **Structural Analysis**
   - Analyze directory structure and module organization
   - Map component dependencies and relationships
   - Identify architectural patterns and anti-patterns
   - Assess code organization and separation of concerns
   - Review module boundaries and interfaces

2. **Deprecated Code Detection**
   - Search for deprecated commands, aliases, and parameters
   - Find TODO, FIXME, XXX, HACK comments
   - Identify commented-out code blocks
   - Look for legacy compatibility layers
   - Find unused feature flags and conditionals

3. **Dependency Analysis**
   - Audit all dependencies in Cargo.toml
   - Identify unused dependencies with cargo-udeps
   - Find duplicate functionality across dependencies
   - Analyze dependency tree depth and complexity
   - Check for security vulnerabilities in dependencies

4. **Code Quality Metrics**
   - Count unwrap() and panic!() calls
   - Analyze error handling patterns
   - Measure cyclomatic complexity
   - Check function and module sizes
   - Identify code duplication

### Phase 2: Implementation Gap Analysis

1. **Feature Completeness Review**
   - Compare implemented features vs documentation claims
   - Identify partially implemented features
   - Find stub functions and placeholder code
   - Review test coverage for critical paths
   - Check for missing error cases

2. **Architecture Consistency**
   - Identify duplicate implementations of similar functionality
   - Find inconsistent patterns across modules
   - Detect abstraction leaks
   - Review interface consistency
   - Check for proper separation of concerns

3. **Performance Analysis**
   - Identify potential performance bottlenecks
   - Find unnecessary allocations and clones
   - Check for inefficient algorithms
   - Review async/await usage patterns
   - Analyze resource management

4. **Storage and State Management**
   - Review storage implementations and redundancy
   - Analyze session management complexity
   - Check for state consistency issues
   - Review persistence patterns
   - Identify potential data races

### Phase 3: Issue Categorization and Prioritization

1. **Issue Categories**
   ```
   CRITICAL: Data loss risk, crashes, security issues
   HIGH: Major functionality gaps, severe tech debt
   MEDIUM: Performance issues, code quality problems
   LOW: Minor improvements, nice-to-have features
   ```

2. **Evaluation Criteria**
   - User impact severity
   - Implementation complexity
   - Risk of regression
   - Maintenance burden
   - Security implications
   - Performance impact

3. **Priority Matrix**
   ```
   High Impact + Low Effort = Do First
   High Impact + High Effort = Do Next
   Low Impact + Low Effort = Quick Wins
   Low Impact + High Effort = Defer/Skip
   ```

### Phase 4: Issue Documentation

For each identified issue, document:

1. **Issue Summary**
   - Clear description of the problem
   - Current behavior vs expected behavior
   - Impact on users and developers
   - Root cause analysis

2. **Evidence Collection**
   - Code locations and line numbers
   - Specific examples from codebase
   - Metrics and measurements
   - Test failures or gaps

3. **Proposed Solution**
   - High-level approach
   - Alternative solutions considered
   - Implementation complexity estimate
   - Risk assessment

### Phase 5: Specification Generation

1. **Determine Spec Requirements**
   - Analyze existing specs to find highest number
   - Group related issues if they should be addressed together
   - Separate unrelated issues into individual specs
   - Establish implementation dependencies

2. **Generate Specifications**
   For each issue identified:
   ```markdown
   ---
   number: {next_available_number}
   title: {descriptive_title}
   category: {foundation|optimization|testing|compatibility}
   priority: {critical|high|medium|low}
   status: draft
   dependencies: [{related_spec_numbers}]
   created: {current_date}
   ---

   # Specification {number}: {title}

   ## Context
   {background_and_problem_description}

   ## Objective
   {clear_goal_statement}

   ## Requirements
   ### Functional Requirements
   - {specific_requirements}

   ### Non-Functional Requirements
   - {performance_security_usability}

   ## Acceptance Criteria
   - [ ] {measurable_criteria}

   ## Technical Details
   {implementation_approach}

   ## Dependencies
   {prerequisites_and_affected_components}

   ## Testing Strategy
   {test_approach}

   ## Documentation Requirements
   {required_documentation_updates}
   ```

3. **Specification Categories**
   - **Foundation**: Core architecture, error handling, storage
   - **Optimization**: Performance, dependency reduction, code cleanup
   - **Testing**: Test coverage, test infrastructure, CI/CD
   - **Compatibility**: Breaking changes, migrations, upgrades

### Phase 6: Evaluation Report Generation

1. **Create Comprehensive Report**
   ```markdown
   # Prodigy Technical Evaluation Report

   ## Executive Summary
   {high_level_findings}

   ## Metrics Summary
   - Total Issues Found: {count}
   - Critical Issues: {count}
   - Lines of Code: {count}
   - Technical Debt Score: {score}

   ## Critical Issues
   {detailed_critical_issues}

   ## High Priority Improvements
   {high_priority_list}

   ## Technical Debt Analysis
   {debt_categories_and_impact}

   ## Recommendations
   {prioritized_action_items}

   ## Generated Specifications
   {list_of_created_specs}
   ```

### Phase 7: Git Commit Process

1. **Stage All Generated Files**
   - Stage evaluation report
   - Stage all new specification files
   - Verify all files are properly formatted

2. **Create Descriptive Commit**
   ```
   add: technical evaluation and improvement specs {first_num}-{last_num}

   Comprehensive evaluation identified {total} issues:
   - {critical_count} critical issues requiring immediate attention
   - {high_count} high priority improvements
   - {medium_count} medium priority enhancements
   - {low_count} low priority optimizations

   Generated specifications:
   - Spec {num}: {title} (priority: {level})
   [list each spec]

   See PRODIGY_EVALUATION_REPORT.md for full analysis.
   ```

## Evaluation Checklist

### Code Quality Issues to Detect
- [ ] Unwrap/panic usage in production code
- [ ] Missing error handling
- [ ] Inconsistent error types
- [ ] Code duplication
- [ ] Dead code
- [ ] Overly complex functions
- [ ] Poor test coverage
- [ ] Missing documentation

### Architectural Issues to Identify
- [ ] Duplicate implementations
- [ ] Inconsistent patterns
- [ ] Tight coupling
- [ ] Abstraction leaks
- [ ] Circular dependencies
- [ ] God objects/modules
- [ ] Missing interfaces
- [ ] Poor separation of concerns

### Performance Issues to Find
- [ ] Unnecessary allocations
- [ ] Inefficient algorithms
- [ ] Blocking I/O in async code
- [ ] Resource leaks
- [ ] Missing caching
- [ ] Redundant computations
- [ ] Large binary size
- [ ] Slow build times

### Maintenance Issues to Detect
- [ ] Deprecated dependencies
- [ ] Unused dependencies
- [ ] Outdated patterns
- [ ] Technical debt
- [ ] Missing tests
- [ ] Unclear code
- [ ] Magic numbers/strings
- [ ] Hardcoded values

## Example Issues and Specifications

### Example 1: Duplicate Storage Systems
**Issue**: Three parallel storage implementations
**Spec Generated**: "Consolidate Storage Systems"
**Priority**: Critical
**Solution**: Unify to single global storage

### Example 2: Poor Error Handling
**Issue**: 140+ unwrap() calls
**Spec Generated**: "Fix Critical Unwrap Calls"
**Priority**: Critical
**Solution**: Replace with proper Result handling

### Example 3: Unused Dependencies
**Issue**: 40% of dependencies unused
**Spec Generated**: "Remove Unused Dependencies"
**Priority**: High
**Solution**: Audit and remove unnecessary deps

### Example 4: Complex MapReduce State
**Issue**: Overly complex state machine
**Spec Generated**: "Simplify MapReduce State Machine"
**Priority**: Medium
**Solution**: Refactor to simpler design

## Output Files

The command generates:
1. `PRODIGY_EVALUATION_REPORT.md` - Full evaluation report
2. `specs/{number}-{title}.md` - Individual specification for each issue
3. Git commit with all changes

## Success Criteria

The evaluation is successful when:
- All major issues are identified and documented
- Specifications are generated for each actionable issue
- Issues are properly prioritized
- Implementation dependencies are established
- Report provides clear improvement roadmap
- All files are committed to git

## Notes

- Focus on actionable issues with clear solutions
- Avoid speculative or "nice to have" improvements
- Prioritize simplification over adding features
- Consider implementation effort vs benefit
- Group related issues when they share solutions
- Keep specifications focused and achievable
- Don't create specs for issues already being addressed