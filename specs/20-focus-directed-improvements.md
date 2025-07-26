# Spec 20: Focus-Directed Improvements

## Overview
Add support for passing focus directives to the mmm improvement loop, allowing users to guide the code review and improvement process towards specific goals like "improve code coverage", "optimize performance", or "enhance security".

## Motivation
Currently, the mmm improvement loop runs with broad analysis and Claude prioritizes issues automatically based on severity. Users cannot direct the improvement process towards specific goals or areas of concern. This spec adds a natural language interface for providing focus directives that influence what improvements are prioritized.

## User Interface

### CLI Usage
```bash
# Basic usage (current behavior)
mmm

# With focus directive
mmm focus on improving code coverage
mmm focus on optimizing performance 
mmm focus on enhancing security
mmm focus on reducing technical debt
mmm focus on improving documentation
mmm focus on better error handling
```

### Implementation Approach
The `focus on` prefix is optional but improves readability. The system should accept both forms:
- `mmm focus on improving code coverage`
- `mmm improving code coverage`

All remaining arguments after `mmm` are concatenated into a focus directive string.

## Technical Design

### 1. CLI Argument Changes

Update `src/main.rs` to capture all remaining arguments as the focus directive:

```rust
/// Improve code quality with zero configuration
#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "Memento Mori Manager - Improve code quality automatically", long_about = None)]
struct Cli {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    target: f32,

    /// Show detailed progress
    #[arg(long)]
    show_progress: bool,

    /// Enable verbose output (-v for debug, -vv for trace, -vvv for all)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    
    /// Focus directive for improvements (e.g., "focus on improving code coverage")
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    focus: Vec<String>,
}
```

### 2. Command Structure Update

Update `src/improve/command.rs`:

```rust
use clap::Args;

#[derive(Debug, Args)]
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Show detailed progress
    #[arg(long)]
    pub show_progress: bool,
    
    /// Focus directive for improvements
    pub focus: Option<String>,
}
```

### 3. Session Integration

Update `src/improve/session.rs` to pass focus directive to Claude commands:

```rust
async fn call_claude_code_review(&self) -> Result<()> {
    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions")
       .arg("/mmm-code-review");
       
    // Pass focus directive if provided
    if let Some(focus) = &self.focus {
        cmd.env("MMM_FOCUS", focus);
    }
    
    cmd.status().await?;
    Ok(())
}
```

### 4. Claude Command Updates

Update `.claude/commands/mmm-code-review.md` to incorporate focus directives:

#### Add Variables Section
```markdown
## Variables

SCOPE: $ARGUMENTS (optional - specify scope like "src/parser", "tests", specific files, or omit for entire codebase)
FOCUS: $MMM_FOCUS (optional - focus directive from mmm CLI, e.g., "improving code coverage")
```

#### Update Phase 1: Project Context and Scope Analysis
```markdown
### Phase 1: Project Context and Scope Analysis

1. **Read Project Context**
   - Read .mmm context files (PROJECT.md, ARCHITECTURE.md, CONVENTIONS.md, DECISIONS.md)
   - Understand project goals, architecture patterns, and coding standards
   - Identify recently completed specifications from ROADMAP.md

2. **Parse Focus Directive**
   - If FOCUS environment variable is set, parse the focus directive
   - Map focus keywords to review priorities:
     - "code coverage" → Prioritize missing tests, untested code paths
     - "performance" → Focus on algorithmic complexity, allocations, hot paths
     - "security" → Emphasize input validation, unsafe code, dependencies
     - "documentation" → Check missing docs, unclear APIs, examples
     - "error handling" → Review Result/Option usage, panic conditions
     - "technical debt" → Identify refactoring opportunities, code smells
   - Adjust issue severity based on focus area

3. **Determine Review Scope**
   - If SCOPE specified: Focus on specific files/directories
   - If no SCOPE: Review recent changes since last major commit
   - Apply focus filter to prioritize relevant areas
   - Check git status for uncommitted changes
```

#### Update Phase 7: Recommendations and Action Items
```markdown
### Phase 7: Recommendations and Action Items

1. **Focus-Aware Issue Categorization**
   When FOCUS directive is provided, adjust severity ratings:
   
   Example: If FOCUS="improving code coverage":
   - Missing tests for critical functions → Critical (elevated from High)
   - Untested error paths → High (elevated from Medium)
   - Low test coverage modules → High (elevated from Medium)
   - Other issues → Normal severity
   
   Example: If FOCUS="optimizing performance":
   - O(n²) algorithms in hot paths → Critical
   - Unnecessary allocations → High
   - Inefficient data structures → High
   - Style issues → Low (demoted)

2. **Improvement Suggestions**
   - Prioritize suggestions aligned with focus directive
   - Group related improvements under focus themes
   - Provide specific examples relevant to focus area
   - Include metrics/benchmarks for focus area if applicable

3. **Action Plan**
   - Focus-aligned issues at top of priority list
   - Clear indication of why items are prioritized (due to focus)
   - Estimated impact on focus area for each improvement
   - Secondary improvements listed separately
```

#### Update Phase 8: Specification Generation
```markdown
### Phase 8: Temporary Specification Generation & Git Commit

2. **Spec Content Requirements**
   ```markdown
   # Iteration {N}: Code Quality Improvements
   
   ## Overview
   Temporary specification for code improvements identified in automated review.
   {IF FOCUS: "Focus directive: {FOCUS}"}
   
   ## Focus Area Analysis
   {IF FOCUS: Include section explaining how focus directive influenced prioritization}
   
   ## Issues to Address
   
   ### Priority Issues (Focus: {FOCUS})
   {Issues directly related to focus directive}
   
   ### Secondary Issues
   {Other important issues not related to focus}
   ```
```

## Focus Directive Examples

### Code Coverage Focus
```bash
mmm focus on improving code coverage
```
Prioritizes:
- Missing unit tests
- Untested code paths
- Low coverage modules
- Integration test gaps
- Test quality improvements

### Performance Focus
```bash
mmm focus on optimizing performance
```
Prioritizes:
- Algorithmic complexity issues
- Memory allocation patterns
- Hot path optimizations
- Data structure choices
- Concurrent code efficiency

### Security Focus
```bash
mmm focus on enhancing security
```
Prioritizes:
- Input validation gaps
- Unsafe code usage
- Dependency vulnerabilities
- Authentication/authorization issues
- Sensitive data handling

### Documentation Focus
```bash
mmm focus on improving documentation
```
Prioritizes:
- Missing API documentation
- Unclear function purposes
- Outdated documentation
- Missing examples
- Poor error messages

### Technical Debt Focus
```bash
mmm focus on reducing technical debt
```
Prioritizes:
- Code duplication
- Complex/long functions
- Poor abstractions
- Outdated patterns
- Refactoring opportunities

## Natural Language Processing

The focus directive parser should be flexible and handle variations:
- "improve test coverage" → code coverage focus
- "make it faster" → performance focus
- "fix security issues" → security focus
- "better docs" → documentation focus
- "clean up code" → technical debt focus

Multiple focus areas can be specified:
- "improve performance and security" → dual focus
- "better tests and documentation" → dual focus

## Progress Tracking

Update the session output to show focus directive:

```
🎯 Starting MMM Improve (Target: 8.0)
📋 Focus: Improving code coverage
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔄 Iteration 1/10...
   Running focused code review...
   ✓ Found 5 code coverage issues (2 critical)
   ✓ Found 3 other issues (1 medium, 2 low)
   ✓ Generated spec: iteration-1708123456-improvements.md
```

## Error Handling

### Invalid Focus Directives
If the focus directive cannot be parsed or understood:
- Log a warning but continue with broad analysis
- Include note in the review that focus was unclear
- Suggest valid focus examples in output

### Conflicting Focus Areas
When multiple focus areas conflict (e.g., "optimize performance" vs "improve readability"):
- Apply both filters with equal weight
- Note the potential conflict in the spec
- Let the reviewer make case-by-case decisions

## Implementation Steps

1. **Update CLI to accept focus arguments** (src/main.rs)
2. **Pass focus through ImproveCommand** (src/improve/command.rs)
3. **Add focus to Session struct** (src/improve/session.rs)
4. **Pass focus via environment variable** to Claude commands
5. **Update /mmm-code-review** to parse and apply focus directives
6. **Add focus-aware prioritization** to issue categorization
7. **Update spec generation** to include focus context
8. **Test with various focus directives**

## Future Enhancements

### Focus Profiles
Pre-defined focus profiles for common scenarios:
```bash
mmm --profile production  # Focus on performance, security, error handling
mmm --profile testing     # Focus on code coverage, test quality
mmm --profile refactor    # Focus on technical debt, code organization
```

### Focus Metrics
Track improvement metrics specific to focus area:
- Code coverage: Show coverage % before/after
- Performance: Run benchmarks, show improvements
- Security: Count vulnerabilities fixed
- Documentation: Show doc coverage %

### Focus Persistence
Save focus directive in `.mmm/config.toml`:
```toml
[improve]
focus = "improving code coverage"
target = 8.0
```

## Success Criteria

1. Users can provide natural language focus directives
2. Code review prioritizes issues based on focus
3. Generated specs clearly indicate focus influence
4. Improvement loop makes measurable progress on focus area
5. Focus directive is shown in progress output
6. System handles unclear focus gracefully

## Example Usage Flow

```bash
$ mmm focus on improving code coverage
🎯 Starting MMM Improve (Target: 8.0)
📋 Focus: Improving code coverage
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Initial Analysis:
   Project Score: 6.8/10.0
   Code Coverage: 45%
   Focus Impact: High priority on test-related improvements

🔄 Iteration 1/10...
   Running focused code review...
   ✓ Found 8 code coverage issues:
     - 3 critical: Core modules with 0% coverage
     - 4 high: Important functions lacking tests
     - 1 medium: Edge cases not tested
   ✓ Found 5 other issues (lower priority due to focus)
   ✓ Generated spec: iteration-1708123456-improvements.md
   
   Implementing improvements...
   ✓ Added 15 unit tests
   ✓ Created 3 integration tests
   ✓ Fixed test infrastructure issues
   
   Running tests and linting...
   ✓ All tests passing
   ✓ Coverage increased to 62%
   
📊 Progress Update:
   Project Score: 7.2/10.0 ↑
   Code Coverage: 62% ↑ (Focus metric improving!)
   
🔄 Iteration 2/10...
   [continues with focus on remaining coverage gaps]
```

## Status: PROPOSED

This spec outlines a comprehensive system for adding focus directives to the mmm improvement loop, allowing users to guide the automated improvement process towards specific goals while maintaining the simplicity of the current CLI interface.