# Spec 20: Focus-Directed Initial Analysis

## Overview
Add support for passing focus directives to the initial code analysis phase of mmm, allowing users to guide what aspects of code quality get prioritized in the first improvement spec like "user experience", "performance", or "security".

## Motivation
Currently, the mmm improvement loop runs with broad analysis and Claude prioritizes issues automatically based on severity. Users cannot direct the initial analysis towards specific goals or areas of concern. This spec adds a simple flag to influence what improvements are prioritized in the initial spec generation only.

## User Interface

### CLI Usage
```bash
# Basic usage (current behavior)
mmm

# With focus directive (affects initial analysis only)
mmm --focus "user experience"
mmm --focus "performance" 
mmm --focus "security"
mmm --focus "code coverage"
mmm --focus "documentation"
mmm --focus "error handling"
```

### Implementation Approach
The focus is passed as a simple flag that only affects the initial code review that generates the first improvement spec. Subsequent iterations run normally without the focus directive.

## Technical Design

### 1. CLI Argument Changes

Update `src/main.rs` to add a simple focus flag:

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
    
    /// Focus directive for initial analysis (e.g., "user experience", "performance")
    #[arg(long)]
    focus: Option<String>,
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

Update `src/improve/session.rs` to pass focus directive only on the first iteration:

```rust
async fn call_claude_code_review(&self, iteration: usize) -> Result<()> {
    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions")
       .arg("/mmm-code-review");
       
    // Pass focus directive only on first iteration
    if iteration == 1 {
        if let Some(focus) = &self.focus {
            cmd.env("MMM_FOCUS", focus);
        }
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

### User Experience Focus
```bash
mmm --focus "user experience"
```
Prioritizes:
- API ergonomics and usability
- Error messages and user feedback
- Documentation clarity
- CLI/UI responsiveness
- Developer experience issues

### Performance Focus
```bash
mmm --focus "performance"
```
Prioritizes:
- Algorithmic complexity issues
- Memory allocation patterns
- Hot path optimizations
- Data structure choices
- Concurrent code efficiency

### Security Focus
```bash
mmm --focus "security"
```
Prioritizes:
- Input validation gaps
- Unsafe code usage
- Dependency vulnerabilities
- Authentication/authorization issues
- Sensitive data handling

### Code Coverage Focus
```bash
mmm --focus "code coverage"
```
Prioritizes:
- Missing unit tests
- Untested code paths
- Low coverage modules
- Integration test gaps
- Test quality improvements

### Documentation Focus
```bash
mmm --focus "documentation"
```
Prioritizes:
- Missing API documentation
- Unclear function purposes
- Outdated documentation
- Missing examples
- Poor error messages

## Focus Interpretation

The focus directive is passed as-is to Claude, which interprets it naturally. Common patterns:
- "user experience" → API design, error handling, documentation
- "performance" → Speed, memory, efficiency
- "security" → Safety, validation, vulnerabilities
- "testing" or "code coverage" → Test quality and coverage
- "maintainability" → Code organization, clarity, documentation

## Progress Tracking

Update the session output to show focus directive on first iteration only:

```
🎯 Starting MMM Improve (Target: 8.0)
📋 Focus: user experience (initial analysis)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔄 Iteration 1/10...
   Running focused code review...
   ✓ Found 5 UX-related issues (2 critical)
   ✓ Found 3 other issues (1 medium, 2 low)
   ✓ Generated spec: iteration-1708123456-improvements.md

🔄 Iteration 2/10...
   Running code review...
   ✓ Found 4 issues (1 critical, 2 medium, 1 low)
   ✓ Generated spec: iteration-1708123457-improvements.md
```

## Error Handling

Focus directives are treated as natural language hints. Since they only affect the initial analysis:
- No validation needed - Claude interprets the focus naturally
- Unclear focus results in standard broad analysis
- Focus is simply passed through as an environment variable

## Implementation Steps

1. **Update CLI to accept --focus flag** (src/main.rs)
2. **Pass focus through ImproveCommand** (src/improve/command.rs)
3. **Add focus to Session struct** (src/improve/session.rs)
4. **Pass focus via MMM_FOCUS env var on first iteration only**
5. **Update /mmm-code-review to check for MMM_FOCUS**
6. **Add focus-aware prioritization when MMM_FOCUS is set**
7. **Test with various focus directives**

## Future Enhancements

- Multiple focus areas: `--focus "performance and security"`
- Focus profiles: `--profile production` (combines common focuses)
- Focus-specific metrics in the analysis output

## Success Criteria

1. Users can provide focus via `--focus` flag
2. Initial code review prioritizes issues based on focus
3. First generated spec reflects the focus area
4. Subsequent iterations proceed normally without focus
5. Focus directive is shown in progress output for first iteration
6. System handles any focus string gracefully

## Example Usage Flow

```bash
$ mmm --focus "user experience"
🎯 Starting MMM Improve (Target: 8.0)
📋 Focus: user experience (initial analysis)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Initial Analysis:
   Project Score: 6.8/10.0

🔄 Iteration 1/10...
   Running focused code review...
   ✓ Found 6 UX-related issues:
     - 2 critical: Poor error messages, confusing API
     - 3 high: Missing documentation, unclear naming
     - 1 medium: Inconsistent behavior
   ✓ Found 4 other issues
   ✓ Generated spec: iteration-1708123456-improvements.md
   
   Implementing improvements...
   ✓ Improved error messages with context
   ✓ Renamed confusing functions
   ✓ Added missing documentation
   
   Running tests and linting...
   ✓ All tests passing
   
📊 Progress Update:
   Project Score: 7.2/10.0 ↑
   
🔄 Iteration 2/10...
   Running code review...
   [continues with normal broad analysis]
```

## Status: PROPOSED

This spec outlines a simple system for adding an optional focus directive to guide the initial code analysis phase of mmm. The focus only affects the first iteration, keeping the self-improvement loop simple while allowing users to influence what gets prioritized initially.