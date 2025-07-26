# Debug Command

Runs comprehensive debugging analysis for the Eidolon AI agent orchestration system, including tests, builds, and intelligent error analysis optimized for Claude Code workflows.

## Usage
This command performs automated debugging and provides structured error analysis for rapid issue resolution.

## Core Analysis Steps
1. **Test Execution**: Comprehensive test suite with async/concurrent testing
2. **Build Verification**: Debug and release builds with feature validation
3. **Static Analysis**: Code formatting, linting with clippy, and quality checks
4. **Error Intelligence**: Advanced error categorization and solution suggestions
5. **Dependency Validation**: Cargo dependency tree and feature flag verification

## Test Execution Strategy
```bash
# Progressive test execution with detailed output
just test                    # Basic unit and integration tests
just test-verbose            # Tests with full output visibility
just test-integration        # Integration tests only
just coverage                # Test coverage with tarpaulin
```

## Build Verification Pipeline
```bash
# Core builds and feature validation
just build                   # Debug build
just build-release           # Optimized release build
just check-all               # All targets and features check
just build-native            # Native CPU optimized build
```

## Code Quality Analysis
```bash
# Automated formatting and linting
just fmt-check               # Format verification
just lint                    # Clippy with warnings as errors
just audit                   # Security vulnerability scan
just outdated                # Check for outdated dependencies
```

## Complete CI Pipeline
```bash
# Full continuous integration workflow
just ci                      # Runs: fmt-check → lint → test → doc-check
just full-check              # Complete: clean → build → test → lint → doc → audit
```

## Advanced Error Analysis & Claude Code Integration

### Intelligent Error Detection
The debug command provides structured error analysis optimized for Claude Code workflows:

1. **Context-Aware Pattern Recognition**:
   - **Async/Await Issues**: Missing `.await`, incorrect async block usage
   - **Lifetime Errors**: Borrow checker violations, ownership problems
   - **Tokio Runtime**: Task spawning errors, channel communication issues
   - **Type Mismatches**: Generic constraints, trait bound violations
   - **Test Failures**: Assertion analysis with suggested fixes

2. **Project-Specific Error Categories**:
   - **Agent Management**: Process spawning failures, MCP communication errors
   - **WebSocket Issues**: Connection drops, message serialization problems
   - **Concurrency**: Arc/Mutex deadlocks, DashMap access patterns
   - **Git Operations**: Worktree conflicts, merge failures
   - **Database**: SQLx query errors, migration issues (when implemented)

3. **Claude Code Optimized Diagnostics**:
   - **File Location**: Precise `file_path:line_number` references for easy navigation
   - **Search Patterns**: Provides `rg` commands for investigating related code
   - **Fix Suggestions**: Actionable code changes with context
   - **Test Recommendations**: Specific test cases to verify fixes

### Automated Recovery Actions
```bash
# Safe automatic fixes applied in sequence
just fix                     # Automated compiler suggestions
just fmt                     # Code formatting
just lint-all                # Full clippy analysis
just update                  # Update dependencies (careful review needed)
```

## Example Debug Session Output

```
🔍 Eidolon Debug Analysis - Claude Code Optimized

=== BUILD VERIFICATION ===
✅ just build: Debug build completed successfully
✅ just build-release: Release build optimized
❌ just check-all: Feature 'experimental' compilation error

=== TEST EXECUTION ===  
✅ just test: 156/156 tests passed
❌ just test-pattern coordination_test: Async test timeout
   📍 tests/coordination_handlers_test.rs:89 - task claim timeout
✅ just coverage: 78.4% coverage (target: 80%+)

=== STATIC ANALYSIS ===
❌ just lint: 3 warnings found
   📍 src/agents/claude_wrapper.rs:234 - unnecessary clone()
   📍 src/mcp/handlers.rs:567 - complex type needs type alias
   📍 src/orchestrator/scheduler.rs:123 - unused Result

🧠 INTELLIGENT ERROR ANALYSIS

1. **Async Test Timeout** - tests/coordination_handlers_test.rs:89
   🔍 Pattern: Tokio test runtime timeout on task claim
   💡 Fix: Add timeout annotation or increase duration
   📝 Code: `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`
   🔎 Search: `rg "tokio::test" tests/`

2. **Unnecessary Clone** - src/agents/claude_wrapper.rs:234  
   🔍 Pattern: Cloning Arc is redundant
   💡 Fix: Remove .clone() call on Arc
   🔎 Search: `rg "\.clone\(\)" src/agents/`

3. **Complex Type** - src/mcp/handlers.rs:567
   🔍 Pattern: Type alias would improve readability
   💡 Fix: Create type alias for complex generic
   📝 Code: `type HandlerResult = Result<serde_json::Value, McpError>;`
   🔎 Search: `rg "Result<.*Value.*Error>" src/mcp/`

🔧 AUTOMATED RECOVERY
✅ just fix: Applied 2 compiler suggestions  
✅ just fmt: Code formatting standardized
✅ just lint-all: Full target analysis completed
✅ just update: Dependencies up to date

🎯 CLAUDE CODE ACTIONS
1. Edit tests/coordination_handlers_test.rs:89 - Add timeout config
2. Edit src/agents/claude_wrapper.rs:234 - Remove unnecessary clone  
3. Edit src/mcp/handlers.rs:567 - Add type alias
4. Run: `just test` to verify fixes
5. Test: `just run` to validate orchestration

📊 PROJECT HEALTH
- Build Status: ⚠️  Feature flag issue (1 error)
- Test Coverage: ⚠️  78.4% (below 80% target)
- Dependencies: ✅ All crates verified
- Async Safety: ✅ No data races detected
```

## Eidolon-Specific Intelligence

### AI Agent Orchestration Expertise
- **Agent Spawning**: Validates Claude Code process creation and MCP connections
- **Concurrency Safety**: Detects DashMap/Arc<RwLock> race conditions and deadlocks
- **Task Scheduling**: Verifies priority queue ordering and dependency resolution
- **Git Operations**: Checks worktree isolation and merge conflict handling
- **WebSocket Stability**: Identifies MCP server connection drops and reconnection logic

### Claude Code Workflow Optimization
- **Precise Navigation**: `file_path:line_number` format for instant IDE navigation
- **Search Integration**: Ready-to-use `rg` commands for code investigation
- **Batch Operations**: Parallelizable fix suggestions for efficient execution
- **Context Preservation**: Maintains error context across multiple debugging iterations

### Justfile Command Alignment
All commands verified against current `justfile` structure:
- ✅ `just build` → Debug build for development
- ✅ `just build-release` → Optimized production build
- ✅ `just ci` → Complete fmt/mmm-lint/test pipeline
- ✅ `just full-check` → Comprehensive validation
- ✅ `just run` → Launch Eidolon orchestrator

## Prerequisites & Dependencies

### Required Tools
```bash
# Core Rust toolchain
rustc --version          # Rust 1.70+ required
cargo --version          # Cargo package manager
just --version           # Just command runner

# Optional but recommended
cargo-watch --version    # Hot reloading
cargo-tarpaulin --version # Code coverage
cargo-audit --version    # Security scanning
```

### Project Dependencies
```bash
# Core async runtime and concurrency
tokio                    # Async runtime
dashmap                  # Concurrent hashmap
axum                     # Web framework

# Agent management
serde/serde_json         # Serialization
uuid                     # Unique identifiers
git2                     # Git operations

# Development tools
tracing                  # Structured logging
anyhow                   # Error handling
clap                     # CLI framework
```

### System Requirements
```bash
# Git for workspace management
git --version            # Git 2.20+ for worktree support

# Claude Code CLI (agents will use this)
claude-code --version    # Required for agent spawning

# Database (when implemented)
sqlite3 --version        # SQLite for persistence
```
