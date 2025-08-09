# MMM - Declarative LLM Orchestration

[![CI](https://github.com/iepathos/mmm/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/mmm/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/mmm/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/release.yml)

Define declarative AI development workflows in YAML. Run bounded, testable improvement loops. Ship better code.

## What Is MMM?

MMM lets you declare LLM-powered development workflows in simple YAML files, similar to how you define CI/CD pipelines. Instead of copy-pasting between Claude and your terminal, you declare the workflow once and run it repeatedly.

```yaml
# Example: Improve test coverage
- claude: "/mmm-analyze-coverage"
  id: coverage_analysis
  
- claude: "/mmm-write-tests ${coverage_analysis.spec}"
  
- shell: "cargo test"
  on_failure:
    claude: "/mmm-fix-test --error ${shell.output}"
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

Run `mmm cook <playbook.yml>` and MMM will:

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

Basic improvement workflow:
```yaml
# improve.yml
- claude: "/mmm-code-review"
- claude: "/mmm-implement-spec"
- claude: "/mmm-lint"
```

### Advanced Workflows

Test-driven development workflow:
```yaml
# tdd.yml
- claude: "/mmm-analyze-coverage"
  id: coverage
  
- claude: "/mmm-write-tests ${coverage.spec}"
  
- shell: "cargo test"
  on_failure:
    claude: "/mmm-fix-test --error ${shell.output}"
    max_attempts: 3
    
- claude: "/mmm-lint"
```

Security audit workflow:
```yaml
# security.yml
- claude: "/mmm-security-audit"
  id: audit
  analysis:
    force_refresh: true
    
- claude: "/mmm-implement-spec ${audit.spec}"
  commit_required: true
  
- shell: "cargo audit"
  on_failure:
    fail_workflow: true
```

Performance optimization:
```yaml
# performance.yml  
- shell: "cargo bench --bench main -- --save-baseline before"

- claude: "/mmm-performance-review"
  id: perf
  
- claude: "/mmm-implement-spec ${perf.spec}"

- shell: "cargo bench --bench main -- --baseline before"
  capture_output: true
  id: bench_results
  
- claude: "/mmm-analyze-results --data ${bench_results.output}"
```

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
