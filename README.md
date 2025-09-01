# Prodigy

[![CI](https://github.com/iepathos/prodigy/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/prodigy/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/prodigy/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/release.yml)

> 🚧 **Early Prototype** - This project is under active development and APIs may change

**AI pair programming orchestrator** - Run reproducible code improvement workflows with Claude. Fix bugs, improve coverage, eliminate tech debt - all through simple YAML workflows.

## What Is Prodigy?

Prodigy orchestrates AI-powered development workflows, turning ad-hoc Claude sessions into reproducible, bounded automation. Like CI/CD for your AI pair programmer - define the workflow once, run it repeatedly, ship better code.

### Transform This:
```
You: "Fix the failing tests"
Claude: "I'll help you fix those tests..."
You: *copy-paste error*
Claude: "Try this fix..."
You: *copy-paste, run tests*
You: "Still failing"
Claude: "Let me see the new error..."
[Repeat endlessly...]
```

### Into This:
```yaml
# One command: prodigy cook workflows/debug.yml
- shell: "cargo test"
  on_failure:
    claude: "/prodigy-debug-test-failure --output ${shell.output}"
    max_attempts: 3
```

### What's New in v0.1.0+

🚀 **MapReduce Orchestration**: Process work items in parallel across multiple Claude agents
- Run up to N agents concurrently in isolated worktrees
- Automatic work distribution and result aggregation
- Smart filtering and sorting of work items
- Persistent state and checkpointing for job recovery
- Automatic retry logic with configurable attempts
- Custom variable capture and interpolation

⚡ **Optimized Context Generation**: 90%+ reduction in context file sizes
- Technical debt files: 8.2MB → <500KB
- Test coverage: 266KB → <30KB  
- Dependency graphs: 155KB → <20KB

🛠️ **Enhanced Error Recovery**: Sophisticated error handling with auto-recovery
- Automatic retry of failed formatting/linting
- Full subprocess stdout/stderr capture
- Flexible failure modes per command
- MapReduce job resumption from checkpoints
- on_success handlers for conditional execution

📊 **Data Pipeline Features**: Advanced filtering and transformation
- Regex pattern matching: `path matches '\.rs$'`
- Nested field access: `${item.nested.field}`
- Complex expressions: `priority > 5 && severity == 'critical'`

## Why Use Prodigy?

### Real Impact
- **10x faster fixes**: What takes hours of back-and-forth happens in minutes
- **Parallel scale**: Fix 10 bugs simultaneously instead of one at a time  
- **Knowledge capture**: Your best prompts become reusable workflows
- **Team multiplier**: Share workflows that work, standardize AI usage
- **Cost control**: Set limits, prevent runaway sessions, track usage

### Developer Experience
```bash
# Monday: Discover a workflow that perfectly fixes test failures
prodigy cook workflows/debug.yml

# Tuesday: Share it with your team
git add workflows/debug.yml && git commit -m "Best debug workflow ever"

# Wednesday: Everyone uses the same proven approach
prodigy cook workflows/debug.yml  # Same great results for everyone
```

## Architecture: How Prodigy Orchestrates Claude Commands

```
┌──────────────────────────────────────────────────────┐
│                   Prodigy Orchestration                  │
│  • Workflow management (YAML configs)                │
│  • Git operations & commit tracking                  │
│  • Iteration control & state management              │
│  • Parallel worktree sessions                        │
│  • MapReduce orchestration for parallel execution    │
│  • Test validation & static code analysis            │
│  • Context generation for Claude commands            │
└───────────────────┬──────────────────────────────────┘
                    │ Orchestrates & provides context to
┌───────────────────▼──────────────────────────────────┐
│              Claude Commands Layer                   │
│  • /prodigy-code-review - Analyzes & generates specs     │
│  • /prodigy-implement-spec - Applies improvements        │
│  • /prodigy-lint - Formats & validates code              │
│  • /prodigy-security-audit - Security analysis           │
│  • /prodigy-performance - Performance optimization       │
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

Every developer using AI faces the same frustrations:
- **Copy-paste hell** - Constantly moving code between Claude and your terminal
- **No memory** - Explaining the same context over and over
- **Unbounded costs** - AI sessions that run forever without clear outcomes
- **Lost knowledge** - That perfect prompt that fixed everything? Gone forever
- **No parallelism** - One conversation, one fix at a time

Prodigy is the orchestration layer that makes AI development **scalable, reproducible, and safe**.

## What Prodigy Is

- **🎯 Bounded Execution**: Set limits, prevent runaway costs, always maintain control
- **🔄 Reproducible Workflows**: Same YAML, same results - share what works with your team
- **🚀 Parallel at Scale**: Run 10+ Claude agents simultaneously fixing different issues
- **🔒 Git-Native Safety**: Every change tracked, every decision logged, easy rollback
- **✅ Test-Driven**: Changes must pass tests or they don't ship

## What Prodigy Is NOT

- ❌ Not an autonomous agent that runs forever without supervision
- ❌ Not a replacement for developers - it's a force multiplier
- ❌ Not another chat interface - it's workflow automation
- ❌ Not magic - it's engineering discipline applied to AI

## Core Features

### Context Management
Prodigy maintains comprehensive project context in `.prodigy/` directory:
- **Analysis Data**: Code structure, dependencies, architecture patterns
- **Technical Debt**: Complexity hotspots, duplication, code issues
- **Test Coverage**: Coverage gaps, untested critical functions
- **Metrics History**: Performance trends, quality improvements

Context is automatically provided to Claude commands via environment variables:
- `Prodigy_CONTEXT_AVAILABLE="true"` - Indicates context is ready
- `Prodigy_CONTEXT_DIR="/path/to/.prodigy/context"` - Path to analysis data
- `Prodigy_AUTOMATION="true"` - Signals automated execution mode

For detailed context documentation, see `CLAUDE.md`.

## Core Concepts

### Bounded Autonomy
Unlike AutoGPT-style agents, Prodigy workflows have:
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

```bash
# Install Prodigy
cargo install --git https://github.com/iepathos/prodigy

# Initialize in your project
prodigy init

# Fix all your failing tests
prodigy cook workflows/debug.yml

# Eliminate tech debt in parallel
prodigy cook workflows/debtmap-mapreduce.yml --worktree
```

**What happens**: Prodigy will spawn Claude agents that analyze, fix, test, and commit improvements to your code. All changes are tracked in git. Nothing ships without passing tests.

## Installation

```bash
# Clone and build
git clone https://github.com/iepathos/prodigy
cd prodigy
cargo build --release

# Add to PATH or use directly
./target/release/prodigy cook workflows/implement.yml
```

## Usage

### Getting Started
```bash
# First time setup - install Prodigy commands in your project
prodigy init

# Then cook your code to improve it with available workflows
prodigy cook workflows/implement.yml
```

### Basic Usage
```bash
# Cook your code in current directory
prodigy cook workflows/implement.yml

# Cook code in a specific directory
prodigy cook workflows/implement.yml --path /path/to/repo
prodigy cook workflows/implement.yml --path ./relative/path
prodigy cook workflows/implement.yml --path ~/projects/myapp

# Cook with a security-focused workflow
prodigy cook workflows/security.yml

# Run with more iterations
prodigy cook workflows/implement.yml --max-iterations 20

# Run in an isolated git worktree for parallel execution
prodigy cook workflows/performance-workflow.yml --worktree

# Fully automated mode (auto-accept merge prompts)
prodigy cook workflows/implement.yml --worktree --yes

# Process multiple files with mapping
prodigy cook workflows/implement.yml --map "specs/*.md"

# Resume an interrupted session
prodigy cook workflows/implement.yml --resume session-abc123

# See detailed progress
prodigy cook workflows/implement.yml --verbose

# Track metrics during cooking
prodigy cook workflows/implement.yml --metrics
```

### MapReduce Workflows (NEW)
```bash
# Run parallel technical debt elimination
prodigy cook workflows/debtmap-reduce.yml --worktree

# Process multiple files in parallel with custom workflow
prodigy cook workflows/fix-files-mapreduce.yml --worktree

# Auto-merge results from parallel agents
prodigy cook workflows/mapreduce-example.yml --worktree --yes

# Resume an interrupted MapReduce job from checkpoint
prodigy cook workflows/debtmap-reduce.yml --worktree --resume

# Run with custom parallelism limit
prodigy cook workflows/mapreduce-example.yml --worktree --max-parallel 20
```

### Available Workflows

Prodigy includes several pre-built workflows in the `workflows/` directory:

#### Sequential Workflows
- **analysis-workflow.yml**: Code analysis and metrics generation
- **code-review.yml**: Code review and quality improvements
- **complex-build-pipeline.yml**: Complex build pipeline with multiple stages
- **coverage.yml**: Test coverage improvement
- **coverage-simplified.yml**: Simplified coverage workflow
- **coverage-with-test-debug.yml**: Coverage with integrated test debugging
- **custom-analyzer.yml**: Custom code analysis workflow
- **debtmap.yml**: Technical debt analysis using debtmap tool
- **debug.yml**: Debug and fix test failures
- **debug-with-spec.yml**: Debug with spec generation
- **documentation-workflow.yml**: Documentation generation and updates
- **implement.yml**: General implementation workflow with testing
- **implement-with-tests.yml**: Implementation with test generation
- **performance-workflow.yml**: Performance optimization and profiling
- **product-enhancement-workflow.yml**: Product enhancement workflow
- **security.yml**: Security-focused analysis and fixes
- **tech-debt.yml**: Technical debt cleanup

#### MapReduce Workflows (Parallel Execution)
- **debtmap-reduce.yml**: Parallel technical debt elimination using debtmap integration
- **fix-files-mapreduce.yml**: Fix issues in multiple files concurrently
- **mapreduce-example.yml**: Complete example showing all MapReduce features
- **test-mapreduce.yml**: Simple test workflow for MapReduce functionality

Create custom workflows for your project needs!

### What Happens (Git-Native Flow)
1. **Code Review**: Claude analyzes code and generates improvement specs
2. **Spec Commit**: Creates `specs/temp/iteration-*-improvements.md` and commits it
3. **Implementation**: Applies fixes from the spec and commits changes
4. **Linting**: Runs appropriate linting tools for your language, commits if changes
5. **Progress Tracking**: Repeats until target iterations reached or no more improvements found

Each step creates git commits for complete auditability.

### Requirements
- [Claude Code CLI](https://claude.ai/code) installed and configured (v0.6.0 or later)
- Git repository (for commit tracking and worktree support)
- A project with code files

## Real-World Examples

### Example 1: Fix All Clippy Warnings
```bash
$ prodigy cook workflows/tech-debt.yml
🔍 Found 47 clippy warnings across 12 files
🤖 Claude is fixing them...
✅ Fixed 47/47 warnings
🧪 All tests still passing
📝 Committed fixes to git

Time saved: ~2 hours of manual fixes
```

### Example 2: Parallel Bug Squashing
```bash
$ prodigy cook workflows/debtmap-reduce.yml --worktree
🔍 Analyzing codebase...
📊 Found 23 high-priority issues
🚀 Spawning 10 parallel Claude agents...

[Agent 1] Fixing: Memory leak in cache.rs
[Agent 2] Fixing: SQL injection risk in query.rs
[Agent 3] Fixing: Race condition in worker.rs
...
[Agent 10] Fixing: Unchecked array access in parser.rs

⏱️ All agents completed in 4 minutes
🔀 Merging fixes from all agents...
✅ Successfully fixed 22/23 issues

What would have taken a day was done in minutes.
```

### Example 3: Test Coverage Sprint
```bash
$ prodigy cook workflows/coverage.yml --metrics
📊 Starting coverage: 43%
🤖 Generating tests for uncovered code...
✅ Added 67 new test cases
📊 Final coverage: 78%
📈 Coverage improved by 35%

All tests passing. All changes committed.
```

## How It Works

### Declarative Workflow Execution
```
prodigy cook <playbook.yml>
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
- **Simple State**: `.prodigy/state.json` tracks basic session info (current score, run count)
- **Project Context**: `.prodigy/PROJECT.md`, `ARCHITECTURE.md` provide Claude with project understanding
- **Optimized Context (v0.1.0+)**: Context files reduced by 90%+ through smart aggregation (see CLAUDE.md for details)
- All human-readable, git-friendly, no complex databases

## Why Prodigy vs Other Approaches?

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
Prodigy is currently **Rust-first** during early development as we refine the tool by using it to build itself. While the architecture is designed to be language-agnostic (our end goal), we're prioritizing Rust to ensure a solid foundation.

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

Prodigy supports two workflow execution modes:
1. **Sequential** - Traditional step-by-step execution
2. **MapReduce** - Parallel execution across multiple worktrees (NEW)

### Simple Workflows

Basic implementation workflow:
```yaml
# Simple array format - each item is a command step
- claude: "/prodigy-implement-spec $ARG"
- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure --spec $ARG --output ${shell.output}"
- claude: "/prodigy-lint"
```

### Advanced Workflows

Security audit workflow:
```yaml
# Security-focused workflow
- claude: "/prodigy-security-audit"
  id: audit
  outputs:
    spec:
      file_pattern: "specs/temp/*-security.md"
- claude: "/prodigy-implement-spec ${audit.spec}"
- claude: "/prodigy-security-validate"
```

Performance optimization:
```yaml
# Performance workflow with metrics
- claude: "/prodigy-performance"
- claude: "/prodigy-implement-spec $ARG"
- shell: "cargo bench"
  on_failure:
    claude: "/prodigy-debug-test-failure --output ${shell.output}"
```

Test coverage workflow:
```yaml
# Coverage improvement workflow
- claude: "/prodigy-coverage"
- claude: "/prodigy-implement-spec $ARG"
- shell: "cargo test"
- claude: "/prodigy-test-generate --coverage"
```

### MapReduce Workflows (NEW)

Enable massive parallelization by processing work items across multiple Claude agents:

```yaml
name: parallel-debt-elimination
mode: mapreduce

# Optional setup phase to generate work items
setup:
  - shell: "debtmap analyze . --output debt_items.json"

# Map phase: Process each debt item in parallel
map:
  input: debt_items.json
  json_path: "$.debt_items[*]"
  
  # Commands to execute for each work item
  agent_template:
    commands:
      - claude: "/fix-issue ${item.description}"
        context:
          file: "${item.location.file}"
          line: "${item.location.line}"
      
      - shell: "cargo test"
        on_failure:
          claude: "/debug-test ${shell.output}"
          max_attempts: 3
          fail_workflow: false
  
  # Parallelization settings
  max_parallel: 10
  timeout_per_agent: 600s
  retry_on_failure: 2
  
  # Optional filtering and sorting
  filter: "severity == 'high' || severity == 'critical'"
  sort_by: "priority"

# Reduce phase: Aggregate results
reduce:
  commands:
    - claude: "/summarize-fixes ${map.results}"
      capture_output: true
    
    - shell: "git merge --no-ff prodigy-agent-*"
      commit_required: true
    
    - claude: "/generate-report"
      env:
        TOTAL_FIXED: "${map.successful}"
        TOTAL_FAILED: "${map.failed}"
```

#### MapReduce Features

- **Variable Interpolation**: Access work item fields with `${item.field}`, nested properties with `${item.nested.field}`
- **Data Pipeline**: Filter items with expressions like `priority > 5` or `path matches '\.rs$'`
- **Parallel Execution**: Run up to N agents concurrently (configurable)
- **Automatic Merging**: Merge all agent branches back to main
- **Error Recovery**: Retry failed agents, continue on partial failures
- **Persistent State**: Checkpoint-based recovery for interrupted jobs
- **Custom Variables**: Capture command output with custom variable names via `capture_output`
- **Conditional Execution**: on_success and on_failure handlers for both map and reduce phases
- **Progress Tracking**: Real-time progress bars for parallel agent execution
- **Job Resumption**: Resume failed MapReduce jobs from last checkpoint with `--resume`

#### Command Arguments & Error Handling

Prodigy provides sophisticated error handling with automatic recovery:

```yaml
# Implementation workflow with advanced error handling
- claude: "/prodigy-implement-spec $ARG"
  commit_required: true
  
- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure --spec $ARG --output ${shell.output}"
    max_attempts: 3
    fail_workflow: false  # Continue even if tests can't be fixed
    
- shell: "just fmt && just lint"
  on_failure:
    # Auto-recovery: Automatically retry formatting/linting after Claude fixes
    shell: "just fmt && just lint"
    max_attempts: 2
    fail_workflow: false
```

**Error Handling Features**:
- **Automatic Recovery**: Failed formatting/linting commands can auto-retry after fixes
- **Subprocess Feedback**: Full stdout/stderr capture for debugging
- **Flexible Failure Modes**: Choose whether to fail the workflow or continue
- **Retry Logic**: Configure max attempts for each recovery action
- **Context Preservation**: Error outputs passed to recovery commands via `${shell.output}`

#### Commit Requirements

By default, Prodigy expects every command to create git commits. However, some commands like linting may not always make changes. Use `commit_required: false` to allow these commands to succeed without creating commits:

```yaml
# Example: Linting may not always create commits
- claude: "/prodigy-implement-spec $ARG"
  
- claude: "/prodigy-lint"
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
prodigy cook workflows/performance-workflow.yml --worktree

# In another terminal, run a different workflow
prodigy cook workflows/security.yml --worktree

# Fully automated parallel sessions
prodigy cook workflows/tech-debt.yml --worktree --yes &
prodigy cook workflows/documentation-workflow.yml --worktree --yes &

# List active worktree sessions
prodigy worktree ls

# Merge improvements back to main branch
prodigy worktree merge prodigy-performance-1234567890

# Merge all completed worktrees
prodigy worktree merge --all

# Clean up completed worktrees (shorthand: -f)
prodigy worktree clean prodigy-performance-1234567890
prodigy worktree clean -f  # Clean specific worktree

# Clean all worktrees (shorthand: -a)
prodigy worktree clean --all
prodigy worktree clean -a  # Clean all worktrees
```

Each session runs in its own git worktree with an isolated branch, allowing multiple cooking efforts to proceed without interfering with each other. Worktrees are stored in `~/.prodigy/worktrees/{project-name}/` and are preserved on failure for debugging and automatically suggested for cleanup on success.

### Initialize Commands

Prodigy requires Claude commands to be installed in your project. Use `prodigy init` to set them up:

```bash
# Initialize all Prodigy commands in current project
prodigy init

# Force overwrite existing commands
prodigy init --force

# Install specific commands only
prodigy init --commands prodigy-code-review,prodigy-lint

# Initialize in a different directory
prodigy init --path /path/to/project
```

The init command will:
- Verify the directory is a git repository
- Create `.claude/commands/` directory structure
- Install the core Prodigy commands
- Handle existing commands gracefully (skip or overwrite with `--force`)
- Provide clear feedback on what was installed

## Project Structure

```
prodigy/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── cook/             # Core cooking logic
│   │   ├── execution/    # Execution engines
│   │   │   ├── mapreduce.rs     # MapReduce orchestration
│   │   │   ├── state.rs         # Persistent state & checkpointing
│   │   │   ├── data_pipeline.rs # Data filtering & sorting
│   │   │   └── interpolation.rs # Variable interpolation
│   │   └── workflow/     # Workflow processing
│   ├── config/           # Configuration management
│   │   └── mapreduce.rs  # MapReduce config parsing
│   ├── metrics/          # Metrics tracking and analysis
│   ├── session/          # Session state management
│   ├── simple_state/     # Minimal state management
│   ├── subprocess/       # Subprocess abstraction layer
│   └── worktree/         # Git worktree management
├── workflows/            # Pre-built workflow definitions
├── .claude/commands/     # Prodigy command definitions
├── .prodigy/                 # Project context and state
└── README.md            # This file

# Worktrees are stored outside the project:
~/.prodigy/worktrees/{project-name}/
├── prodigy-session-1234567890/
├── prodigy-performance-1234567891/
├── prodigy-agent-1234567893/  # MapReduce agent worktrees
└── prodigy-security-1234567892/
```

## Development

```bash
# Run tests
cargo test

# Build and run
cargo build --release
./target/release/prodigy cook workflows/implement.yml --verbose

# Run with metrics tracking
cargo run -- cook workflows/implement.yml --metrics

# Test worktree functionality
cargo run -- cook workflows/security.yml --worktree
```

## Command Discovery

Prodigy commands in `.claude/commands/` follow a discovery pattern:

```bash
# Commands are automatically discovered by Claude Code
.claude/commands/
├── prodigy-code-review.md         # Analyzes code and generates specs
├── prodigy-implement-spec.md      # Implements improvements from specs
├── prodigy-lint.md                # Formats and validates code
├── prodigy-debug-test-failure.md  # Debugs failing tests
├── debtmap.md                 # Technical debt analysis
├── fix-debt-item.md          # Fixes individual debt items
└── [your-custom-commands].md  # Add your own commands
```

Each command receives:
- Project context via environment variables
- Command arguments from workflow
- Current iteration state

## Philosophy

1. **Self-Sufficient Development Loops**: Fully autonomous Claude-driven development cycles that run without manual intervention
2. **Highly Configurable**: Customize workflows to create targeted loops for security, performance, testing, or any development aspect
3. **Git-Native**: Use git as the communication layer - simple, reliable, auditable
4. **Dead Simple**: One command to start, minimal options, works immediately
5. **Clear & Minimal**: Enable powerful development loops without over-engineering
6. **Language Agnostic**: Works with any programming language Claude can understand
7. **Parallel by Design**: Built-in support for running multiple improvement loops simultaneously

## Limitations

- Requires Claude Code CLI to be installed and configured (v0.6.0+)
- Improvements are limited by Claude's capabilities and context window
- Each iteration runs independently (no memory between sessions beyond git history and checkpoints)
- Workflow configuration is intentionally simple (no complex conditionals or plugins)
- MapReduce jobs require sufficient disk space for multiple worktrees
- Some features are experimental and may change in future releases

## License

MIT

## Start Using Prodigy Today

```bash
# Install and start improving your code in under a minute
cargo install --git https://github.com/iepathos/prodigy
cd your-project
prodigy init
prodigy cook workflows/implement.yml
```

Your AI pair programmer is waiting. Let's ship better code, faster.

## Contributing

We're building the future of AI-assisted development. Join us:

- **More languages**: Extend beyond Rust to Python, TypeScript, Go
- **More workflows**: Share your best patterns with the community
- **More integrations**: VSCode, IntelliJ, CI/CD pipelines
- **More safety**: Better bounds, smarter limits, clearer guardrails

Keep it simple. Keep it working. Keep it bounded.

---

Built with ❤️ in Rust. Open source because AI-assisted development should be accessible to everyone.
