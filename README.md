# mmm

A dead simple CLI tool that enables highly configurable and easily manageable self-sufficient Claude development loops.

## What Are Self-Sufficient Claude Development Loops?

Self-sufficient Claude development loops are fully autonomous improvement cycles where Claude AI acts as both the reviewer and implementor of code changes. These loops run without human intervention, making decisions about what to improve, how to implement changes, and when to stop - creating a continuous improvement process that enhances code quality automatically.

### Why Are They Powerful?

1. **Autonomous Operation**: Once started, the loop runs completely independently, analyzing code, identifying issues, implementing fixes, and validating changes without manual oversight
2. **Consistent Quality**: Every iteration follows the same high standards, applying best practices uniformly across your entire codebase
3. **Parallel Execution**: Multiple loops can run simultaneously on different aspects (security, performance, testing) using git worktrees
4. **Git-Native Workflow**: Every change is tracked through commits, providing complete auditability and easy rollback if needed
5. **Customizable Focus**: Direct the AI's attention to specific concerns like security vulnerabilities, performance bottlenecks, or test coverage

## How MMM Simplifies Running These Loops

`mmm` makes self-sufficient development loops accessible with a single command:

```bash
# Start an autonomous improvement loop
mmm cook
```

Behind this simplicity, `mmm` handles all the complexity:

1. **Automated Claude CLI Integration**: Manages all interactions with Claude CLI, handling authentication, retries, and error recovery
2. **Git Workflow Management**: Automatically creates commits for each step (review, implementation, linting) with meaningful messages
3. **Worktree Isolation**: Optionally runs improvements in isolated git worktrees, enabling parallel improvement sessions
4. **Configurable Workflows**: Define custom sequences of Claude commands via simple YAML files
5. **Smart State Management**: Tracks progress, handles interruptions, and provides clear status updates
6. **Focus Persistence**: Maintains improvement direction across all iterations when given a focus area

## What It Does

`mmm` orchestrates self-sufficient Claude development loops that continuously improve your codebase. Run `mmm cook` and it automatically:

1. **Reviews** code with Claude CLI and creates improvement specs
2. **Implements** the improvements by applying fixes to your code
3. **Lints** and formats the code with automated tools
4. **Repeats** until your target iterations is reached or no more improvements are found

Each iteration is fully autonomous - Claude handles review, implementation, and validation without manual intervention. All changes are committed to git with clear audit trails. Configure workflows to focus on security, performance, testing, or any development aspect you need.

## Installation

```bash
# Clone and build
git clone https://github.com/iepathos/mmm
cd mmm
cargo build --release

# Add to PATH or use directly
./target/release/mmm cook
```

## Usage

### Getting Started
```bash
# First time setup - install MMM commands in your project
mmm init

# Then cook your code to improve it
mmm cook
```

### Basic Usage
```bash
# Cook your code in current directory
mmm cook

# Cook code in a specific directory
mmm cook /path/to/repo
mmm cook ./relative/path
mmm cook ~/projects/myapp

# Cook with a specific focus area
mmm cook --focus security

# Run with more iterations
mmm cook --max-iterations 20

# Use a custom workflow configuration
mmm cook --config examples/security-workflow.yml

# Run in an isolated git worktree for parallel execution
mmm cook --worktree --focus performance

# Fully automated mode (auto-accept merge prompts)
mmm cook --worktree --yes

# Process multiple files with mapping
mmm cook --map "specs/*.md" --config examples/implement.yml

# See detailed progress
mmm cook --verbose
```

### Focus Areas

The `--focus` flag applies to every iteration in your cooking session, ensuring consistent improvement direction:
- **security**: Security vulnerabilities, input validation, authentication
- **performance**: Speed optimizations, memory usage, algorithmic improvements
- **testing**: Test coverage, test quality, edge cases
- **architecture**: Code structure, design patterns, modularity
- **critical**: Only critical issues and bugs
- Custom focus areas based on your project needs

### What Happens (Git-Native Flow)
1. **Code Review**: Claude analyzes code and generates improvement specs
2. **Spec Commit**: Creates `specs/temp/iteration-*-improvements.md` and commits it
3. **Implementation**: Applies fixes from the spec and commits changes
4. **Linting**: Runs appropriate linting tools for your language, commits if changes
5. **Progress Tracking**: Repeats until target iterations reached or no more improvements found

Each step creates git commits for complete auditability.

### Requirements
- [Claude CLI](https://claude.ai/cli) installed and configured (v0.6.0 or later)
- Git repository (for commit tracking and worktree support)
- A project with code files

## Examples

```bash
# Basic cooking run
$ mmm cook
🔍 Starting improvement loop...
📋 Focus: None specified (general improvements)
🔄 Iteration 1/10...
🤖 Running /mmm-code-review...
✅ Code review completed
🔧 Running /mmm-implement-spec iteration-1708123456-improvements...
✅ Implementation completed
🧹 Running /mmm-lint...
✅ Linting completed

✅ Improvement session finished early:
   Iterations: 2/10
   Files improved: 3
   Reason: No more issues found

# Focused improvement with custom workflow
$ mmm cook --config examples/security-workflow.yml
📝 Starting workflow with 5 commands
📋 Focus: security
🔄 Workflow iteration 1/8...
📋 Step 1/5: mmm-security-audit
📋 Step 2/5: mmm-implement-spec
📋 Step 3/5: mmm-test-generate
📋 Step 4/5: mmm-implement-spec
📋 Step 5/5: mmm-lint
✅ Improvement session completed

# Parallel worktree sessions
$ mmm cook --worktree --focus performance
🌳 Created worktree: mmm-performance-1708123456 at ~/.mmm/worktrees/myproject/mmm-performance-1708123456
🔄 Iteration 1/10...
[... improvements run ...]
✅ Improvements completed in worktree: mmm-performance-1708123456

Would you like to merge the completed worktree now? (y/N): y
✅ Successfully merged worktree: mmm-performance-1708123456

# Check the git history to see what happened
$ git log --oneline -10
a1b2c3d style: apply automated formatting and lint fixes
b2c3d4e fix: apply improvements from spec iteration-1708123789-improvements
c3d4e5f review: generate improvement spec for iteration-1708123789-improvements
d4e5f6g style: apply automated formatting and lint fixes  
e5f6g7h fix: apply improvements from spec iteration-1708123456-improvements
f6g7h8i review: generate improvement spec for iteration-1708123456-improvements
```

## How It Works

### Git-Native Architecture
```
mmm cook
    ↓
Load configuration (workflow.yml or defaults)
    ↓
┌─────────────────── COOKING LOOP ──────────────────────┐
│  Call claude /mmm-code-review                         │
│      ↓                                                │
│  Generate specs/temp/iteration-*-improvements.md      │
│      ↓                                                │
│  Commit: "review: generate improvement spec..."       │
│      ↓                                                │
│  Extract spec ID from git log                         │
│      ↓                                                │
│  Call claude /mmm-implement-spec {spec-id}            │
│      ↓                                                │
│  Apply fixes and commit: "fix: apply improvements..." │
│      ↓                                                │
│  Call claude /mmm-lint                                │
│      ↓                                                │
│  Format/lint and commit: "style: apply automated..."  │
│      ↓                                                │
│  Check iterations → Continue or Exit                  │
└───────────────────────────────────────────────────────┘
```

### State Management
- **Git History**: Complete audit trail of all changes through commits
- **Temporary Specs**: `specs/temp/iteration-*-improvements.md` contain exact fixes applied  
- **Simple State**: `.mmm/state.json` tracks basic session info (current score, run count)
- **Project Context**: `.mmm/PROJECT.md`, `ARCHITECTURE.md` provide Claude with project understanding
- All human-readable, git-friendly, no complex databases

### Supported Languages
MMM works with any language that Claude CLI can understand. The tool is language-agnostic and relies on Claude's ability to:
- Analyze code structure and patterns
- Generate appropriate improvements
- Run language-specific linting tools

Commonly used with:
- **Rust**: cargo fmt, clippy, cargo test
- **Python**: black, ruff, pytest
- **JavaScript/TypeScript**: prettier, eslint, jest
- **Go**: go fmt, go vet, go test
- **Others**: Any language with linting/formatting tools

## Safety

- **Git-Native**: Every change is a git commit - easy to inspect and revert
- **Automated Testing**: Each iteration runs tests to ensure nothing breaks
- **Incremental**: Makes small, focused improvements rather than large changes
- **Auditable**: Complete paper trail of what was changed and why
- **Validation**: Code is linted and formatted after each change

## Configuration - Flexible Development Loops

`mmm` works out of the box with smart defaults, but its real power comes from customizable workflows that create focused development loops.

### Configurable Workflows

Create a `workflow.yml` file to customize the improvement workflow:

```yaml
# workflow.yml
# Simple YAML format - dead simple and clean
commands:
  - mmm-code-review
  - mmm-implement-spec
  - mmm-lint
```

The default workflow runs these three commands in order:
1. `mmm-code-review` - Analyzes code and generates improvement specs
2. `mmm-implement-spec` - Implements the improvements (spec ID extracted automatically)
3. `mmm-lint` - Runs formatting and linting

You can create custom development loops by combining different Claude commands and focus areas.

#### Focus Arguments

You can specify focus areas for commands using clean YAML syntax:

```yaml
# Security-focused workflow with focus arguments
commands:
  - name: mmm-code-review
    focus: security
  - mmm-implement-spec
  - name: mmm-test-generate
    focus: security
  - mmm-implement-spec
  - mmm-lint
```

Alternative string format also works:
```yaml
commands:
  - mmm-code-review --focus security
  - mmm-implement-spec
  - mmm-lint
```

#### Workflow Examples

**Security-Focused Workflow:**
```yaml
commands:
  - name: mmm-security-audit
    focus: security
  - mmm-implement-spec
  - name: mmm-test-generate
    focus: security
  - mmm-implement-spec
  - mmm-lint
```

**Performance Workflow:**
```yaml
commands:
  - name: mmm-code-review
    focus: performance
  - mmm-implement-spec
  - name: mmm-test-generate
    focus: performance
  - mmm-implement-spec
  - mmm-lint
```

**Quick Fix Workflow:**
```yaml
commands:
  - name: mmm-code-review
    focus: critical
  - mmm-implement-spec
  - mmm-lint
```

### Parallel Sessions with Git Worktrees

Run multiple cooking sessions concurrently without conflicts:

```bash
# Enable worktree mode for this cooking session
mmm cook --worktree --focus "performance"

# In another terminal, run a different cooking focus
mmm cook --worktree --focus "security"

# Fully automated parallel sessions
mmm cook --worktree --yes --focus "testing" &
mmm cook --worktree --yes --focus "documentation" &

# List active worktree sessions
mmm worktree list

# Merge improvements back to main branch
mmm worktree merge mmm-performance-1234567890

# Merge all completed worktrees
mmm worktree merge --all

# Clean up completed worktrees
mmm worktree clean mmm-performance-1234567890
# Or clean all worktrees
mmm worktree clean --all
```

Each session runs in its own git worktree with an isolated branch, allowing multiple cooking efforts to proceed without interfering with each other. Worktrees are stored in `~/.mmm/worktrees/{project-name}/` and are preserved on failure for debugging and automatically suggested for cleanup on success.

### Initialize Commands

MMM requires Claude commands to be installed in your project. Use `mmm init` to set them up:

```bash
# Initialize all MMM commands in current project
mmm init

# Force overwrite existing commands
mmm init --force

# Install specific commands only
mmm init --commands mmm-code-review,mmm-lint

# Initialize in a different directory
mmm init --path /path/to/project
```

The init command will:
- Verify the directory is a git repository
- Create `.claude/commands/` directory structure
- Install the core MMM commands
- Handle existing commands gracefully (skip or overwrite with `--force`)
- Provide clear feedback on what was installed

## Project Structure

```
mmm/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── cook/             # Core cooking logic
│   ├── analyzer/         # Project analysis
│   ├── simple_state/     # Minimal state management
│   └── worktree/         # Git worktree management
├── .mmm/                 # Project context and state
└── README.md            # This file

# Worktrees are stored outside the project:
~/.mmm/worktrees/{project-name}/
├── mmm-session-1234567890/
├── mmm-performance-1234567891/
└── mmm-security-1234567892/
```

## Development

```bash
# Run tests
just test

# Lint with claude
claude /mmm-lint

# Run on sample project
cargo run -- cook --verbose
```

## Philosophy

1. **Self-Sufficient Development Loops**: Fully autonomous Claude-driven development cycles that run without manual intervention
2. **Highly Configurable**: Customize workflows to create focused loops for security, performance, testing, or any development aspect
3. **Git-Native**: Use git as the communication layer - simple, reliable, auditable
4. **Dead Simple**: One command to start, minimal options, works immediately
5. **Clear & Minimal**: Focus on enabling powerful development loops without over-engineering
6. **Language Agnostic**: Works with any programming language Claude can understand
7. **Parallel by Design**: Built-in support for running multiple improvement loops simultaneously

## Limitations

- Requires Claude CLI to be installed and configured
- Improvements are limited by Claude's capabilities and context window
- Each iteration runs independently (no memory between sessions beyond git history)
- Workflow configuration is intentionally simple (no complex conditionals or plugins)

## License

MIT

## Contributing

Focus on making the core `mmm cook` command more robust:
- Better language support
- Improved Claude context building
- Enhanced error handling
- Clearer progress feedback

Keep it simple. Keep it working.

Built with ❤️ in Rust as open source with best intentions for the world.
