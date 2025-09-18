# Prodigy 🚀

[![CI](https://github.com/iepathos/prodigy/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/prodigy/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/prodigy/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/prodigy-cli)](https://crates.io/crates/prodigy-cli)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/prodigy-cli)](https://crates.io/crates/prodigy-cli)

> Transform ad-hoc Claude sessions into reproducible development pipelines with parallel execution, automatic retry, and full state management.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [Using Cargo (Recommended)](#using-cargo-recommended)
  - [Using Homebrew (macOS/Linux)](#using-homebrew-macoslinux)
  - [From Source](#from-source)
  - [From Package Managers](#from-package-managers)
- [Quick Start](#quick-start)
  - [Your First Workflow](#your-first-workflow)
  - [Parallel Execution Example](#parallel-execution-example)
  - [Goal-Seeking Example](#goal-seeking-example)
- [Usage](#usage)
  - [Basic Commands](#basic-commands)
  - [Advanced Workflows](#advanced-workflows)
  - [Configuration](#configuration)
- [Examples](#examples)
  - [Automated Testing Pipeline](#example-1-automated-testing-pipeline)
  - [Parallel Code Analysis](#example-2-parallel-code-analysis)
  - [Goal-Seeking Optimization](#example-3-goal-seeking-optimization)
- [Documentation](#documentation)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## Features

✨ **Workflow Orchestration** - Define complex development workflows in simple YAML
⚡ **Parallel Execution** - Run multiple Claude agents simultaneously with MapReduce
🔄 **Automatic Retry** - Smart retry strategies with exponential backoff and circuit breakers
💾 **Full State Management** - Checkpoint and resume interrupted workflows exactly where they left off
🎯 **Goal-Seeking** - Iterative refinement until specifications are met
🌳 **Git Integration** - Automatic worktree management and commit tracking
🛡️ **Error Recovery** - Comprehensive failure handling with on-failure handlers
📊 **Analytics** - Cost tracking, performance metrics, and optimization recommendations
🔧 **Extensible** - Custom validators, handlers, and workflow composition
📚 **Documentation** - Comprehensive man pages and built-in help system

## Installation

### Using Cargo (Recommended)

```bash
cargo install prodigy-cli
```

### Using Homebrew (macOS/Linux)

```bash
# Coming soon - use cargo install for now
brew install prodigy
```

### From Source

```bash
# Clone the repository
git clone https://github.com/iepathos/prodigy
cd prodigy

# Build and install
cargo build --release
cargo install --path .

# Optional: Install man pages
./scripts/install-man-pages.sh
```

### From Package Managers

#### Arch Linux (AUR)
```bash
# Coming soon
yay -S prodigy-cli
```

#### Debian/Ubuntu
```bash
# Coming soon - use cargo install for now
apt install prodigy-cli
```

## Quick Start

Get up and running in under 5 minutes with these simple examples.

### Your First Workflow

1. Initialize Prodigy in your project:
```bash
prodigy init
```

2. Create a simple workflow (`fix-tests.yml`):
```yaml
name: fix-failing-tests
steps:
  - shell: "cargo test"
    on_failure:
      claude: "/fix-test-failures"
      max_attempts: 3
```

3. Run the workflow:
```bash
prodigy run fix-tests.yml
```

### Parallel Execution Example

Process multiple files simultaneously with MapReduce:

```yaml
name: add-documentation
mode: mapreduce

setup:
  - shell: "find src -name '*.rs' -type f > files.json"

map:
  input: files.json
  agent_template:
    - claude: "/add-rust-docs ${item}"
  max_parallel: 10

reduce:
  - claude: "/summarize Documentation added to ${map.successful} files"
```

Run with:
```bash
prodigy run add-documentation.yml
```

### Goal-Seeking Example

Iteratively improve code until all tests pass:

```yaml
name: achieve-full-coverage
steps:
  - goal_seek:
      goal: "Achieve 100% test coverage"
      command: "claude: /improve-test-coverage"
      validate: "cargo tarpaulin --print-summary | grep '100.00%'"
      max_attempts: 5
```

## Usage

### Basic Commands

```bash
# Run a workflow
prodigy run workflow.yml

# Execute a single command with retries
prodigy exec "claude: /refactor main.rs" --retry 3

# Process files in parallel
prodigy batch "*.py" --command "claude: /add-types" --parallel 5

# Resume an interrupted workflow
prodigy resume workflow-123

# Goal-seeking operation
prodigy goal-seek --goal "Fix all linting errors" --command "claude: /fix-lint"

# View analytics and costs
prodigy analytics --session abc123

# Manage worktrees
prodigy worktree ls
prodigy worktree clean
```

### Advanced Workflows

#### Retry Configuration
```yaml
retry_defaults:
  attempts: 3
  backoff: exponential
  initial_delay: 2s
  max_delay: 30s
  jitter: true

steps:
  - shell: "deploy.sh"
    retry:
      attempts: 5
      backoff:
        fibonacci:
          initial: 1s
      retry_on: [network, timeout]
      retry_budget: 5m
```

#### Environment Management
```yaml
env:
  NODE_ENV: production
  WORKERS:
    command: "nproc"
    cache: true

secrets:
  API_KEY: ${vault:api/keys/production}

steps:
  - shell: "npm run build"
    env:
      BUILD_TARGET: production
    working_dir: ./frontend
```

#### Workflow Composition
```yaml
imports:
  - path: ./common/base.yml
    alias: base

templates:
  test-suite:
    parameters:
      - name: language
        type: string
    steps:
      - shell: "${language} test"

workflows:
  main:
    extends: base.default
    steps:
      - use: test-suite
        with:
          language: cargo
```

### Configuration

Prodigy looks for configuration in these locations (in order):
1. `.prodigy/config.yml` - Project-specific configuration
2. `~/.config/prodigy/config.yml` - User configuration
3. `/etc/prodigy/config.yml` - System-wide configuration

Example configuration:
```yaml
# .prodigy/config.yml
claude:
  model: claude-3-opus
  max_tokens: 4096

worktree:
  max_parallel: 20
  cleanup_policy:
    idle_timeout: 300
    max_age: 3600

retry:
  default_attempts: 3
  default_backoff: exponential

storage:
  events_dir: ~/.prodigy/events
  state_dir: ~/.prodigy/state
```

## Examples

### Example 1: Automated Testing Pipeline

Fix all test failures automatically with intelligent retry:

```yaml
name: test-pipeline
steps:
  - shell: "cargo test"
    on_failure:
      - claude: "/analyze-test-failure ${shell.output}"
      - claude: "/fix-test-failure"
      - shell: "cargo test"
    retry:
      attempts: 3
      backoff: exponential

  - shell: "cargo fmt -- --check"
    on_failure: "cargo fmt"

  - shell: "cargo clippy -- -D warnings"
    on_failure:
      claude: "/fix-clippy-warnings"
```

### Example 2: Parallel Code Analysis

Analyze and improve multiple files concurrently:

```yaml
name: parallel-analysis
mode: mapreduce

setup:
  - shell: |
      find . -name "*.rs" -exec wc -l {} + |
      sort -rn |
      head -20 |
      awk '{print $2}' > complex-files.json

map:
  input: complex-files.json
  agent_template:
    - claude: "/analyze-complexity ${item}"
    - claude: "/suggest-refactoring ${item}"
    - shell: "cargo test --lib $(basename ${item} .rs)"
  max_parallel: 10

reduce:
  - claude: "/generate-refactoring-report ${map.results}"
  - shell: "echo 'Analyzed ${map.total} files, ${map.successful} successful'"
```

### Example 3: Goal-Seeking Optimization

Iteratively improve performance until benchmarks pass:

```yaml
name: performance-optimization
steps:
  - goal_seek:
      goal: "Reduce benchmark time below 100ms"
      command: "claude: /optimize-performance benches/main.rs"
      validate: |
        cargo bench --bench main |
        grep "time:" |
        awk '{print ($2 < 100) ? "score: 100" : "score: " int(100 - $2)}'
      threshold: 100
      max_attempts: 10
      timeout: 1800

  - shell: "cargo bench --bench main > benchmark-results.txt"
  - claude: "/document-optimization benchmark-results.txt"
```

## Documentation

- 📖 [User Guide](docs/user-guide.md) - Complete guide to using Prodigy
- 🔧 [API Reference](docs/api.md) - Detailed API documentation
- 📝 [Workflow Syntax](docs/workflows.md) - YAML workflow configuration reference
- 🏗️ [Architecture](ARCHITECTURE.md) - System design and internals
- 🤝 [Contributing Guide](CONTRIBUTING.md) - How to contribute to Prodigy
- 📚 [Man Pages](man/) - Unix-style manual pages for all commands

### Quick Reference

| Command | Description |
|---------|-------------|
| `prodigy run <workflow>` | Execute a workflow |
| `prodigy exec <command>` | Run a single command |
| `prodigy batch <pattern>` | Process files in parallel |
| `prodigy resume <id>` | Resume interrupted workflow |
| `prodigy goal-seek` | Run goal-seeking operation |
| `prodigy analytics` | View session analytics |
| `prodigy worktree` | Manage git worktrees |
| `prodigy init` | Initialize Prodigy in project |

## Troubleshooting

### Common Issues and Solutions

<details>
<summary><strong>Error: Claude command not found</strong></summary>

Ensure Claude Code CLI is installed and available in PATH:
```bash
# Check if Claude is installed
which claude

# If not installed, follow Claude Code installation guide
# https://claude.ai/code
```
</details>

<details>
<summary><strong>Error: Workflow fails with "worktree already exists"</strong></summary>

Clean up stale worktrees:
```bash
prodigy worktree clean -f
```
</details>

<details>
<summary><strong>Error: "PRODIGY_AUTOMATION not set" when running workflows</strong></summary>

This is expected behavior. Prodigy sets this automatically during workflow execution.
If you're testing manually, you can set it:
```bash
export PRODIGY_AUTOMATION=true
```
</details>

<details>
<summary><strong>Performance: Workflows running slowly</strong></summary>

1. Check parallel execution limits:
```bash
prodigy run workflow.yml --max-parallel 20
```

2. Enable verbose mode to identify bottlenecks:
```bash
prodigy run workflow.yml -v
```

3. Review analytics for optimization opportunities:
```bash
prodigy analytics --session <session-id>
```
</details>

<details>
<summary><strong>Resume: How to recover from interrupted workflows</strong></summary>

Prodigy automatically creates checkpoints. To resume:
```bash
# List available checkpoints
prodigy checkpoints list

# Resume from latest checkpoint
prodigy resume

# Resume specific workflow
prodigy resume workflow-abc123
```
</details>

<details>
<summary><strong>MapReduce: Jobs failing with "DLQ not empty"</strong></summary>

Review and reprocess failed items:
```bash
# View failed items
prodigy dlq view <job-id>

# Reprocess failed items
prodigy dlq reprocess <job-id> --max-parallel 5
```
</details>

<details>
<summary><strong>Git: Merge conflicts after workflow completion</strong></summary>

Prodigy uses isolated worktrees. To resolve:
```bash
# List worktrees
prodigy worktree ls

# Clean completed worktrees
prodigy worktree clean

# Manual merge if needed
git worktree remove <path>
git branch -D <branch>
```
</details>

<details>
<summary><strong>Configuration: Settings not being applied</strong></summary>

Check configuration precedence:
```bash
# Show effective configuration
prodigy config show

# Validate configuration
prodigy config validate
```
</details>

<details>
<summary><strong>Installation: Man pages not available</strong></summary>

Install man pages manually:
```bash
cd prodigy
./scripts/install-man-pages.sh

# Or install to user directory
./scripts/install-man-pages.sh --user
```
</details>

<details>
<summary><strong>Debugging: Need more information about failures</strong></summary>

Enable debug logging:
```bash
# Set log level
export RUST_LOG=debug
prodigy run workflow.yml -vv

# View detailed events
prodigy events --job-id <job-id> --verbose
```
</details>

### Getting Help

- 🐛 [Report Issues](https://github.com/iepathos/prodigy/issues)
- 💬 [Discussions](https://github.com/iepathos/prodigy/discussions)
- 📧 [Email Support](mailto:support@prodigy.dev)

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Quick Start for Contributors

```bash
# Fork and clone the repository
git clone https://github.com/YOUR-USERNAME/prodigy
cd prodigy

# Set up development environment
cargo build
cargo test

# Run with verbose output
RUST_LOG=debug cargo run -- run test.yml

# Before submitting PR
cargo fmt
cargo clippy -- -D warnings
cargo test
```

### Areas We Need Help

- 📦 Package manager distributions (brew, apt, yum)
- 🌍 Internationalization and translations
- 📚 Documentation and examples
- 🧪 Testing and bug reports
- ⚡ Performance optimizations
- 🎨 UI/UX improvements

## License

Prodigy is dual-licensed under MIT and Apache 2.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

Prodigy builds on the shoulders of giants:

- [Claude Code CLI](https://claude.ai/code) - The AI pair programmer that powers Prodigy
- [Tokio](https://tokio.rs) - Async runtime for Rust
- [Clap](https://github.com/clap-rs/clap) - Command-line argument parsing
- [Serde](https://serde.rs) - Serialization framework

Special thanks to all [contributors](https://github.com/iepathos/prodigy/graphs/contributors) who have helped make Prodigy better!

---

<p align="center">
  Made with ❤️ by developers, for developers
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#documentation">Docs</a> •
  <a href="#contributing">Contributing</a>
</p>