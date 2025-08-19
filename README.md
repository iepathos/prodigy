# MMM - Memento Mori Management

[![CI](https://github.com/iepathos/mmm/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/mmm/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/mmm/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/mmm/actions/workflows/release.yml)

**AI pair programming orchestrator** - Run reproducible code improvement workflows with Claude. Fix bugs, improve coverage, eliminate tech debt - all through simple YAML workflows.

## What Is MMM?

MMM orchestrates AI-powered development workflows, turning ad-hoc Claude sessions into reproducible, bounded automation. Like CI/CD for your AI pair programmer - define the workflow once, run it repeatedly, ship better code.

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
# One command: mmm cook workflows/debug.yml
- shell: "cargo test"
  on_failure:
    claude: "/mmm-debug-test-failure --output ${shell.output}"
    max_attempts: 3
    
- shell: "cargo test"  # Verify fix worked
```

### What's New in v0.1.0+

🚀 **MapReduce Orchestration**: Process work items in parallel across multiple Claude agents
- Run up to N agents concurrently in isolated worktrees
- Automatic work distribution and result aggregation
- Smart filtering and sorting of work items

⚡ **Optimized Context Generation**: 90%+ reduction in context file sizes
- Technical debt files: 8.2MB → <500KB
- Test coverage: 266KB → <30KB  
- Dependency graphs: 155KB → <20KB

🛠️ **Enhanced Error Recovery**: Sophisticated error handling with auto-recovery
- Automatic retry of failed formatting/linting
- Full subprocess stdout/stderr capture
- Flexible failure modes per command

📊 **Data Pipeline Features**: Advanced filtering and transformation
- Regex pattern matching: `path matches '\.rs$'`
- Nested field access: `${item.nested.field}`
- Complex expressions: `priority > 5 && severity == 'critical'`

## Why Use MMM?

### Real Impact
- **10x faster fixes**: What takes hours of back-and-forth happens in minutes
- **Parallel scale**: Fix 10 bugs simultaneously instead of one at a time  
- **Knowledge capture**: Your best prompts become reusable workflows
- **Team multiplier**: Share workflows that work, standardize AI usage
- **Cost control**: Set limits, prevent runaway sessions, track usage

### Developer Experience
```bash
# Monday: Discover a workflow that perfectly fixes test failures
mmm cook workflows/debug.yml

# Tuesday: Share it with your team
git add workflows/debug.yml && git commit -m "Best debug workflow ever"

# Wednesday: Everyone uses the same proven approach
mmm cook workflows/debug.yml  # Same great results for everyone
```

## Architecture: How MMM Orchestrates Claude Commands

```
┌──────────────────────────────────────────────────────┐
│                   MMM Orchestration                  │
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

Every developer using AI faces the same frustrations:
- **Copy-paste hell** - Constantly moving code between Claude and your terminal
- **No memory** - Explaining the same context over and over
- **Unbounded costs** - AI sessions that run forever without clear outcomes
- **Lost knowledge** - That perfect prompt that fixed everything? Gone forever
- **No parallelism** - One conversation, one fix at a time

MMM is the orchestration layer that makes AI development **scalable, reproducible, and safe**.

## What MMM Is

- **🎯 Bounded Execution**: Set limits, prevent runaway costs, always maintain control
- **🔄 Reproducible Workflows**: Same YAML, same results - share what works with your team
- **🚀 Parallel at Scale**: Run 10+ Claude agents simultaneously fixing different issues
- **🔒 Git-Native Safety**: Every change tracked, every decision logged, easy rollback
- **✅ Test-Driven**: Changes must pass tests or they don't ship

## What MMM Is NOT

- ❌ Not an autonomous agent that runs forever without supervision
- ❌ Not a replacement for developers - it's a force multiplier
- ❌ Not another chat interface - it's workflow automation
- ❌ Not magic - it's engineering discipline applied to AI

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

```bash
# Install MMM
cargo install --git https://github.com/iepathos/mmm

# Initialize in your project
mmm init

# Fix all your failing tests
mmm cook workflows/debug.yml

# Eliminate tech debt in parallel
mmm cook workflows/debtmap-mapreduce.yml --worktree
```

**What happens**: MMM will spawn Claude agents that analyze, fix, test, and commit improvements to your code. All changes are tracked in git. Nothing ships without passing tests.

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

### MapReduce Workflows (NEW)
```bash
# Run parallel technical debt elimination
mmm cook workflows/debtmap-mapreduce.yml --worktree

# Process multiple files in parallel with custom workflow
mmm cook workflows/fix-files-mapreduce.yml --worktree

# Auto-merge results from parallel agents
mmm cook workflows/mapreduce-example.yml --worktree --yes
```

### Available Workflows

MMM includes several pre-built workflows in the `workflows/` directory:

#### Sequential Workflows
- **implement.yml**: General implementation workflow with testing
- **security.yml**: Security-focused analysis and fixes
- **performance-workflow.yml**: Performance optimization and profiling
- **coverage.yml**: Test coverage improvement
- **tech-debt.yml**: Technical debt cleanup
- **code-review.yml**: Code review and quality improvements
- **debug.yml**: Debug and fix test failures
- **documentation-workflow.yml**: Documentation generation and updates

#### MapReduce Workflows (Parallel Execution)
- **debtmap-mapreduce.yml**: Parallel technical debt elimination across the codebase
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
- [Claude CLI](https://claude.ai/cli) installed and configured (v0.6.0 or later)
- Git repository (for commit tracking and worktree support)
- A project with code files

## Real-World Examples

### Example 1: Fix All Clippy Warnings
```bash
$ mmm cook workflows/tech-debt.yml
🔍 Found 47 clippy warnings across 12 files
🤖 Claude is fixing them...
✅ Fixed 47/47 warnings
🧪 All tests still passing
📝 Committed fixes to git

Time saved: ~2 hours of manual fixes
```

### Example 2: Parallel Bug Squashing
```bash
$ mmm cook workflows/debtmap-mapreduce.yml --worktree
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
$ mmm cook workflows/coverage.yml --metrics
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
- **Optimized Context (v0.1.0+)**: Context files reduced by 90%+ through smart aggregation
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

MMM supports two workflow execution modes:
1. **Sequential** - Traditional step-by-step execution
2. **MapReduce** - Parallel execution across multiple worktrees (NEW)

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
    - shell: "git merge --no-ff mmm-agent-*"
    - claude: "/generate-report"
```

#### MapReduce Features

- **Variable Interpolation**: Access work item fields with `${item.field}`, nested properties with `${item.nested.field}`
- **Data Pipeline**: Filter items with expressions like `priority > 5` or `path matches '\.rs$'`
- **Parallel Execution**: Run up to N agents concurrently (configurable)
- **Automatic Merging**: Merge all agent branches back to main
- **Error Recovery**: Retry failed agents, continue on partial failures

#### Command Arguments & Error Handling

MMM provides sophisticated error handling with automatic recovery:

```yaml
# Implementation workflow with advanced error handling
- claude: "/mmm-implement-spec $ARG"
  commit_required: true
  
- shell: "just test"
  on_failure:
    claude: "/mmm-debug-test-failure --spec $ARG --output ${shell.output}"
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
│   │   ├── execution/    # Execution engines
│   │   │   ├── mapreduce.rs     # MapReduce orchestration
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
├── .claude/commands/     # MMM command definitions
├── .mmm/                 # Project context and state
└── README.md            # This file

# Worktrees are stored outside the project:
~/.mmm/worktrees/{project-name}/
├── mmm-session-1234567890/
├── mmm-performance-1234567891/
├── mmm-agent-1234567893/  # MapReduce agent worktrees
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

## Start Using MMM Today

```bash
# Install and start improving your code in under a minute
cargo install --git https://github.com/iepathos/mmm
cd your-project
mmm init
mmm cook workflows/implement.yml
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
