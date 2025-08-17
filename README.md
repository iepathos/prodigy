# MMM - Memento Mori Management

[![CI](https://github.com/iepathos/mmm/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/mmm/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/mmm/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/release.yml)

Define declarative AI development workflows in YAML. Run bounded, testable improvement loops. Ship better code.

## What Is MMM?

MMM lets you declare LLM-powered development workflows in simple YAML files, similar to how you define CI/CD pipelines. Instead of copy-pasting between Claude and your terminal, you declare the workflow once and run it repeatedly.

```yaml
# Example: Simple workflow
- claude: "/mmm-code-review"
  
- claude: "/mmm-implement-spec $ARG"
  
- shell: "just test"
  on_failure:
    claude: "/mmm-debug-test-failure --spec $ARG --output ${shell.output}"
```

## Why Declarative?

**Reproducible**: Same YAML, same workflow, every time.  
**Shareable**: Check workflows into git, share with your team.  
**Testable**: Define success criteria, verify outcomes.  
**Bounded**: Set iteration limits, prevent runaway costs.  
**Observable**: Every decision is logged, every change tracked.

## Architecture: How MMM Orchestrates Claude Commands

```
┌──────────────────────────────────────────────────────┐
│                   MMM Orchestration                  │
│  • Workflow management (YAML configs)                │
│  • Git operations & commit tracking                  │
│  • Iteration control & state management              │
│  • Parallel worktree sessions                        │
│  • Test validation & static code analysis            │
│  • Context generation for Claude commands            │
└───────────────────┬──────────────────────────────────┘
                    │ Orchestrates & provides context to
┌───────────────────▼──────────────────────────────────┐
│              Claude Commands Layer                   │
│  • /mmm-code-review - Analyzes & generates specs     │
│  • /mmm-implement-spec - Applies improvements        │
│  • /mmm-lint - Formats & validates code              │
│  • /mmm-security-audit - Security analysis           │
│  • /mmm-performance - Performance optimization       │
│  • [Custom commands for your workflow]               │
└───────────────────┬──────────────────────────────────┘
                    │ Executed by
┌───────────────────▼──────────────────────────────────┐
│                  Claude Code CLI                     │
│  • Runs the actual AI analysis & implementation      │
│  • Understands your codebase context                 │
│  • Makes intelligent improvement decisions           │
└──────────────────────────────────────────────────────┘
```

## The Problem We Solve

Developers are increasingly using LLMs for code improvements, but managing these interactions is ad-hoc:
- Copy-pasting between Claude and your editor
- Manually running tests after AI suggestions  
- No reproducibility across team members
- No way to share successful improvement patterns
- Runaway costs from uncontrolled AI loops

MMM provides the orchestration layer that makes AI-assisted development **reproducible, shareable, and safe**.

## What MMM Is

- **Bounded LLM Orchestration**: Run AI-assisted development tasks with iteration limits and clear success criteria
- **Declarative Workflows**: Define complex AI workflows in simple YAML
- **Test-Driven Validation**: Ensure AI changes actually work through integrated testing
- **Git-Native Isolation**: All work happens in isolated worktrees, preserving your main branch
- **Composable Commands**: Mix Claude commands, shell scripts, and custom handlers

## What MMM Is NOT

- ❌ Not an autonomous agent that runs forever
- ❌ Not a replacement for human developers  
- ❌ Not a general-purpose AI framework
- ❌ Not another chatbot interface

## Core Concepts

### Bounded Autonomy
Unlike AutoGPT-style agents, MMM workflows have:
- Maximum iteration limits
- Required success criteria (tests must pass)
- Isolated execution environments (git worktrees)
- Automatic rollback on failure

### Observable Decisions
Every step is logged, every change is in git, every decision can be audited.

### Practical Focus
Built for real development tasks:
- Improving test coverage
- Fixing linting issues
- Implementing specifications
- Refactoring code
- Debugging test failures

## Quick Start

Run `mmm cook <workflow.yml>` and MMM will:

1. **Execute** your declared workflow steps in order
2. **Validate** changes with tests and linting
3. **Commit** each successful change to git
4. **Iterate** within bounded limits
5. **Stop** when success criteria are met or limits reached

## Installation

```bash
# Clone and build
git clone https://github.com/iepathos/mmm
cd mmm
cargo build --release

# Add to PATH or use directly
./target/release/mmm cook workflows/implement.yml
```

## Usage

### Getting Started
```bash
# First time setup - install MMM commands in your project
mmm init

# Then cook your code to improve it with available workflows
mmm cook workflows/implement.yml
```

### Basic Usage
```bash
# Cook your code in current directory
mmm cook workflows/implement.yml

# Cook code in a specific directory
mmm cook workflows/implement.yml --path /path/to/repo
mmm cook workflows/implement.yml --path ./relative/path
mmm cook workflows/implement.yml --path ~/projects/myapp

# Cook with a security-focused workflow
mmm cook workflows/security.yml

# Run with more iterations
mmm cook workflows/implement.yml --max-iterations 20

# Run in an isolated git worktree for parallel execution
mmm cook workflows/performance-workflow.yml --worktree

# Fully automated mode (auto-accept merge prompts)
mmm cook workflows/implement.yml --worktree --yes

# Process multiple files with mapping
mmm cook workflows/implement.yml --map "specs/*.md"

# Resume an interrupted session
mmm cook workflows/implement.yml --resume session-abc123

# See detailed progress
mmm cook workflows/implement.yml --verbose

# Track metrics during cooking
mmm cook workflows/implement.yml --metrics
```

### Available Workflows

MMM includes several pre-built workflows in the `workflows/` directory:
- **implement.yml**: General implementation workflow with testing
- **security.yml**: Security-focused analysis and fixes
- **performance-workflow.yml**: Performance optimization and profiling
- **coverage.yml**: Test coverage improvement
- **tech-debt.yml**: Technical debt cleanup
- **code-review.yml**: Code review and quality improvements
- **debug.yml**: Debug and fix test failures
- **documentation-workflow.yml**: Documentation generation and updates
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
$ mmm cook workflows/implement.yml
🔍 Starting improvement loop...
📋 Workflow: Implementation workflow
🔄 Iteration 1/10...
🤖 Running /mmm-implement-spec...
✅ Implementation completed
🧪 Running tests...
✅ Tests passed
🧹 Running linting...
✅ Linting completed

✅ Improvement session finished early:
   Iterations: 2/10
   Files improved: 3
   Reason: No more issues found

# Security-focused improvement workflow
$ mmm cook workflows/security.yml
📝 Starting workflow with security analysis
📋 Workflow: Security improvements
🔄 Workflow iteration 1/8...
📋 Step 1/3: mmm-security-audit
📋 Step 2/3: mmm-implement-spec
📋 Step 3/3: mmm-security-validate
✅ Improvement session completed

# Parallel worktree sessions
$ mmm cook workflows/performance-workflow.yml --worktree
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

### Declarative Workflow Execution
```
mmm cook <playbook.yml>
    ↓
Parse YAML workflow definition
    ↓
┌─────────────────── WORKFLOW LOOP ─────────────────────┐
│  For each step in workflow:                           │
│      ↓                                                │
│  Execute command (claude:, shell:, etc.)              │
│      ↓                                                │
│  Validate results                                     │
│      ↓                                                │
│  Handle on_failure/on_success conditions              │
│      ↓                                                │
│  Commit changes if successful                         │
│      ↓                                                │
│  Check iteration limit → Continue or Exit             │
└───────────────────────────────────────────────────────┘
```

### State Management
- **Git History**: Complete audit trail of all changes through commits
- **Temporary Specs**: `specs/temp/iteration-*-improvements.md` contain exact fixes applied  
- **Simple State**: `.mmm/state.json` tracks basic session info (current score, run count)
- **Project Context**: `.mmm/PROJECT.md`, `ARCHITECTURE.md` provide Claude with project understanding
- All human-readable, git-friendly, no complex databases

## Why MMM vs Other Approaches?

### vs AutoGPT/Agent Frameworks
- **Bounded**: Limited iterations prevent runaway costs
- **Declarative**: Define workflows in YAML, not code
- **Development-focused**: Built specifically for code improvements

### vs Custom Scripts
- **Reusable**: Share workflows across projects and teams
- **Composable**: Mix and match commands to build complex workflows
- **Tested**: Battle-tested patterns for common tasks

### vs LangChain/LlamaIndex
- **Practical**: Focused on real development tasks, not general AI chains
- **Git-native**: Every change tracked, easy rollback
- **Test-driven**: Validation built into the workflow

### vs Manual LLM Interaction
- **Reproducible**: Same workflow produces consistent results
- **Automated**: No copy-paste between Claude and terminal
- **Auditable**: Complete history of all changes

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

## Workflow Configuration

### Simple Workflows

Basic implementation workflow:
```yaml
# Simple array format - each item is a command step
- claude: "/mmm-implement-spec $ARG"
- shell: "just test"
  on_failure:
    claude: "/mmm-debug-test-failure --spec $ARG --output ${shell.output}"
- claude: "/mmm-lint"
```

### Advanced Workflows

Security audit workflow:
```yaml
# Security-focused workflow
- claude: "/mmm-security-audit"
  id: audit
  outputs:
    spec:
      file_pattern: "specs/temp/*-security.md"
- claude: "/mmm-implement-spec ${audit.spec}"
- claude: "/mmm-security-validate"
```

Performance optimization:
```yaml
# Performance workflow with metrics
- claude: "/mmm-performance"
- claude: "/mmm-implement-spec $ARG"
- shell: "cargo bench"
  on_failure:
    claude: "/mmm-debug-test-failure --output ${shell.output}"
```

Test coverage workflow:
```yaml
# Coverage improvement workflow
- claude: "/mmm-coverage"
- claude: "/mmm-implement-spec $ARG"
- shell: "cargo test"
- claude: "/mmm-test-generate --coverage"
```

#### Command Arguments

You can specify arguments for commands and handle failures:

```yaml
# Implementation workflow with error handling
- claude: "/mmm-implement-spec $ARG"
  commit_required: true
  
- shell: "just test"
  on_failure:
    claude: "/mmm-debug-test-failure --spec $ARG --output ${shell.output}"
    max_attempts: 3
    fail_workflow: false  # Continue even if tests can't be fixed
    
- shell: "just fmt-check && just lint"
  on_failure:
    claude: "/mmm-lint ${shell.output}"
    max_attempts: 3
    fail_workflow: false
```

#### Commit Requirements

By default, MMM expects every command to create git commits. However, some commands like linting may not always make changes. Use `commit_required: false` to allow these commands to succeed without creating commits:

```yaml
# Example: Linting may not always create commits
- claude: "/mmm-implement-spec $ARG"
  
- claude: "/mmm-lint"
  commit_required: false  # Allow to proceed even if no changes made
```

This is especially useful for:
- Linting/formatting commands that may find nothing to fix
- Validation commands that only check code without modifying it
- Optional cleanup steps that may have already been addressed

### Parallel Sessions with Git Worktrees

Run multiple cooking sessions concurrently without conflicts:

```bash
# Enable worktree mode for this cooking session
mmm cook workflows/performance-workflow.yml --worktree

# In another terminal, run a different workflow
mmm cook workflows/security.yml --worktree

# Fully automated parallel sessions
mmm cook workflows/tech-debt.yml --worktree --yes &
mmm cook workflows/documentation-workflow.yml --worktree --yes &

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
│   ├── config/           # Configuration management  
│   ├── metrics/          # Metrics tracking and analysis
│   ├── session/          # Session state management
│   ├── simple_state/     # Minimal state management
│   ├── subprocess/       # Subprocess abstraction layer
│   └── worktree/         # Git worktree management
├── workflows/            # Pre-built workflow definitions
├── .claude/commands/     # MMM command definitions
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
cargo test

# Build and run
cargo build --release
./target/release/mmm cook workflows/implement.yml --verbose

# Run with metrics tracking
cargo run -- cook workflows/implement.yml --metrics

# Test worktree functionality
cargo run -- cook workflows/security.yml --worktree
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
