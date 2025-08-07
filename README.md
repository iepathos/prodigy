# mmm

[![CI](https://github.com/iepathos/mmm/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/mmm/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/mmm/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/release.yml)


Orchestration layer for Claude Code that enables autonomous code improvement through self-sufficient development loops. Combine Claude commands with elegant YAML workflows to run continuous review → fix → enhance cycles with a single command.

## Architecture: How MMM Orchestrates Claude Commands

```
┌─────────────────────────────────────────────────────┐
│                   MMM Orchestration                  │
│  • Workflow management (YAML configs)                │
│  • Git operations & commit tracking                  │
│  • Iteration control & state management              │
│  • Parallel worktree sessions                        │
│  • Test validation & static code analysis            │
│  • Context generation for Claude commands            │
└──────────────────┬──────────────────────────────────┘
                   │ Orchestrates & provides context to
┌──────────────────▼──────────────────────────────────┐
│              Claude Commands Layer                   │
│  • /mmm-code-review - Analyzes & generates specs     │
│  • /mmm-implement-spec - Applies improvements        │
│  • /mmm-lint - Formats & validates code              │
│  • /mmm-security-audit - Security analysis           │
│  • /mmm-performance - Performance optimization       │
│  • [Custom commands for your workflow]               │
└──────────────────┬──────────────────────────────────┘
                   │ Executed by
┌──────────────────▼──────────────────────────────────┐
│                  Claude Code CLI                     │
│  • Runs the actual AI analysis & implementation      │
│  • Understands your codebase context                 │
│  • Makes intelligent improvement decisions           │
└──────────────────────────────────────────────────────┘
```

## What Are Self-Sufficient Claude Development Loops?

Self-sufficient Claude development loops are fully autonomous improvement cycles where Claude AI acts as both the reviewer and implementor of code changes. These loops run without human intervention, making decisions about what to improve, how to implement changes, and when to stop - creating a continuous improvement process that enhances code quality automatically.

### Why Are They Powerful?

1. **Autonomous Operation**: Once started, the loop runs completely independently, analyzing code, identifying issues, implementing fixes, and validating changes without manual oversight
2. **Consistent Quality**: Every iteration follows the same high standards, applying best practices uniformly across your entire codebase
3. **Parallel Execution**: Multiple loops can run simultaneously on different aspects (security, performance, testing) using git worktrees
4. **Git-Native Workflow**: Every change is tracked through commits, providing complete auditability and easy rollback if needed
5. **Customizable Workflows**: Create targeted improvement loops for security, performance, testing, or any development aspect

## How MMM Simplifies Running These Loops

`mmm` makes self-sufficient development loops accessible with a single command:

```bash
# Start an autonomous improvement loop
mmm cook examples/default.yml
```

Behind this simplicity, `mmm` handles all the complexity:

1. **Automated Claude CLI Integration**: Manages all interactions with Claude CLI, handling authentication, retries, and error recovery
2. **Git Workflow Management**: Automatically creates commits for each step (review, implementation, linting) with meaningful messages
3. **Worktree Isolation**: Optionally runs improvements in isolated git worktrees, enabling parallel improvement sessions
4. **Configurable Workflows**: Define custom sequences of Claude commands via simple YAML files
5. **Smart State Management**: Tracks progress, handles interruptions, and provides clear status updates
6. **Workflow Persistence**: Maintains improvement strategy across all iterations in a session

## What It Does

`mmm` orchestrates self-sufficient Claude development loops that continuously improve your codebase. Run `mmm cook <playbook>` and it automatically:

1. **Reviews** code with Claude CLI and creates improvement specs
2. **Implements** the improvements by applying fixes to your code
3. **Lints** and formats the code with automated tools
4. **Repeats** until your target iterations is reached or no more improvements are found

Each iteration is fully autonomous - Claude handles review, implementation, and validation without manual intervention. All changes are committed to git with clear audit trails. Configure workflows to target security, performance, testing, or any development aspect you need.

## Installation

```bash
# Clone and build
git clone https://github.com/iepathos/mmm
cd mmm
cargo build --release

# Add to PATH or use directly
./target/release/mmm cook examples/default.yml
```

## Usage

### Getting Started
```bash
# First time setup - install MMM commands in your project
mmm init

# Then cook your code to improve it with default workflow
mmm cook examples/default.yml
```

### Basic Usage
```bash
# Cook your code in current directory with default workflow
mmm cook examples/default.yml

# Cook code in a specific directory
mmm cook examples/default.yml --path /path/to/repo
mmm cook examples/default.yml --path ./relative/path
mmm cook examples/default.yml --path ~/projects/myapp

# Cook with a security-focused workflow
mmm cook examples/security.yml

# Run with more iterations
mmm cook examples/default.yml --max-iterations 20

# Run in an isolated git worktree for parallel execution
mmm cook examples/performance.yml --worktree

# Fully automated mode (auto-accept merge prompts)
mmm cook examples/default.yml --worktree --yes

# Process multiple files with mapping
mmm cook examples/implement.yml --map "specs/*.md"

# See detailed progress
mmm cook examples/default.yml --verbose
```

### Workflow Types

Configure workflows for different improvement goals:
- **security**: Security vulnerabilities, input validation, authentication
- **performance**: Speed optimizations, memory usage, algorithmic improvements
- **testing**: Test coverage, test quality, edge cases
- **architecture**: Code structure, design patterns, modularity
- **critical**: Only critical issues and bugs
- Create custom workflows for your project needs

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
$ mmm cook examples/default.yml
🔍 Starting improvement loop...
📋 Workflow: General improvements
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

# Security-focused improvement workflow
$ mmm cook examples/security.yml
📝 Starting workflow with 5 commands
📋 Workflow: Security improvements
🔄 Workflow iteration 1/8...
📋 Step 1/5: mmm-security-audit
📋 Step 2/5: mmm-implement-spec
📋 Step 3/5: mmm-test-generate
📋 Step 4/5: mmm-implement-spec
📋 Step 5/5: mmm-lint
✅ Improvement session completed

# Parallel worktree sessions
$ mmm cook examples/performance.yml --worktree
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
mmm cook <playbook.yml>
    ↓
Load playbook configuration
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
MMM is currently **Rust-first** during early development as we refine the tool by using it to build itself. While the architecture is designed to be language-agnostic (our end goal), we're prioritizing Rust to ensure a solid foundation.

**Current Support:**
- **Rust**: Full support with cargo fmt, clippy, cargo test

**Planned Support:**
We plan to expand to these languages as the tool matures:
- **Python**: black, ruff, pytest
- **JavaScript/TypeScript**: prettier, eslint, jest
- **Go**: go fmt, go vet, go test
- **Others**: Any language with linting/formatting tools

The tool's core architecture is language-agnostic and relies on Claude's ability to analyze code structure, generate improvements, and run language-specific tooling.

## Safety

- **Git-Native**: Every change is a git commit - easy to inspect and revert
- **Automated Testing**: Each iteration runs tests to ensure nothing breaks
- **Incremental**: Makes small, targeted improvements rather than large changes
- **Auditable**: Complete paper trail of what was changed and why
- **Validation**: Code is linted and formatted after each change

## Configuration - Flexible Development Loops

`mmm` works out of the box with smart defaults, but its real power comes from customizable workflows that create targeted development loops.

### Configurable Workflows

Create custom playbook files to define your improvement workflows:

```yaml
# my-playbook.yml
# Simple YAML format - dead simple and clean
commands:
  - mmm-code-review
  - mmm-implement-spec
  - name: mmm-lint
    commit_required: false  # Linting may not always make changes
```

The default workflow runs these three commands in order:
1. `mmm-code-review` - Analyzes code and generates improvement specs
2. `mmm-implement-spec` - Implements the improvements (spec ID extracted automatically)
3. `mmm-lint` - Runs formatting and linting (won't fail if no changes needed)

Run your custom playbook:
```bash
mmm cook my-playbook.yml
```

You can create custom development loops by combining different Claude commands.

#### Command Arguments

You can specify arguments for commands using clean YAML syntax:

```yaml
# Security workflow with targeted commands
commands:
  - name: mmm-security-audit
  - mmm-implement-spec
  - name: mmm-test-generate
    args: ["--security"]
  - mmm-implement-spec
  - mmm-lint
```

Alternative string format also works:
```yaml
commands:
  - mmm-security-audit
  - mmm-implement-spec
  - mmm-lint
```

#### Commit Requirements

By default, MMM expects every command to create git commits. However, some commands like linting may not always make changes. Use `commit_required: false` to allow these commands to succeed without creating commits:

```yaml
# Example: Linting may not always create commits
commands:
  - name: mmm-implement-spec
    args: ["$ARG"]
  
  - name: mmm-lint
    commit_required: false  # Allow to proceed even if no changes made
```

This is especially useful for:
- Linting/formatting commands that may find nothing to fix
- Validation commands that only check code without modifying it
- Optional cleanup steps that may have already been addressed

#### Workflow Examples

**Security Workflow:**
```yaml
commands:
  - mmm-security-audit
  - mmm-implement-spec
  - name: mmm-test-generate
    args: ["--security"]
  - mmm-implement-spec
  - mmm-lint
```

**Performance Workflow:**
```yaml
commands:
  - mmm-performance
  - mmm-implement-spec
  - name: mmm-test-generate
    args: ["--performance"]
  - mmm-implement-spec
  - mmm-lint
```

**Quick Fix Workflow:**
```yaml
commands:
  - name: mmm-code-review
    args: ["--critical"]
  - mmm-implement-spec
  - name: mmm-lint
    commit_required: false  # Linting may not find issues after critical fixes
```

**Test Coverage Workflow:**
```yaml
commands:
  - mmm-coverage
  - mmm-implement-spec
  - name: mmm-test-run
    commit_required: false  # Test runs don't modify code
  - name: mmm-lint
    commit_required: false  # Code may already be clean
```

### Parallel Sessions with Git Worktrees

Run multiple cooking sessions concurrently without conflicts:

```bash
# Enable worktree mode for this cooking session
mmm cook examples/performance.yml --worktree

# In another terminal, run a different workflow
mmm cook examples/security.yml --worktree

# Fully automated parallel sessions
mmm cook examples/test-driven.yml --worktree --yes &
mmm cook examples/documentation.yml --worktree --yes &

# List active worktree sessions
mmm worktree ls

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
2. **Highly Configurable**: Customize workflows to create targeted loops for security, performance, testing, or any development aspect
3. **Git-Native**: Use git as the communication layer - simple, reliable, auditable
4. **Dead Simple**: One command to start, minimal options, works immediately
5. **Clear & Minimal**: Enable powerful development loops without over-engineering
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

Help make the core `mmm cook` command more robust:
- Better language support
- Improved Claude context building
- Enhanced error handling
- Clearer progress feedback

Keep it simple. Keep it working.

Built with ❤️ in Rust as open source with best intentions for the world.
