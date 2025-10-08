# Prodigy 🚀

[![CI](https://github.com/iepathos/prodigy/actions/workflows/ci.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/ci.yml)
[![Security](https://github.com/iepathos/prodigy/actions/workflows/security.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/security.yml)
[![Release](https://github.com/iepathos/prodigy/actions/workflows/release.yml/badge.svg)](https://github.com/iepathos/prodigy/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/prodigy)](https://crates.io/crates/prodigy)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/prodigy)](https://crates.io/crates/prodigy)

> Transform ad-hoc Claude sessions into reproducible development pipelines with parallel execution, automatic retry, and full state management.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [Using Cargo (Recommended)](#using-cargo-recommended)
  - [From Source](#from-source)
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

+ ✨ **Workflow Orchestration** - Define complex development workflows in simple YAML
+ ⚡ **Parallel Execution** - Run multiple Claude agents simultaneously with MapReduce
+ 🔄 **Automatic Retry** - Smart retry strategies with exponential backoff and circuit breakers
+ 💾 **Full State Management** - Checkpoint and resume interrupted workflows exactly where they left off
+ 🎯 **Goal-Seeking** - Iterative refinement until specifications are met
+ 🌳 **Git Integration** - Automatic worktree isolation for every workflow execution with commit tracking
+ 🛡️ **Error Recovery** - Comprehensive failure handling with on-failure handlers
+ 📊 **Analytics** - Cost tracking, performance metrics, and optimization recommendations
+ 🔧 **Extensible** - Custom validators, handlers, and workflow composition
+ 📚 **Documentation** - Comprehensive man pages and built-in help system

## Installation

### Using Cargo (Recommended)

```bash
cargo install prodigy
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

# Manage worktrees (all workflow executions use isolated git worktrees by default)
prodigy worktree ls                    # List active worktrees
prodigy worktree ls --detailed        # Show enhanced session information
prodigy worktree ls --json            # Output in JSON format
prodigy worktree ls --detailed --json # Combine detailed info with JSON output
prodigy worktree clean                # Clean up inactive worktrees
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

#### Git Context Variables

Prodigy automatically tracks git changes during workflow execution and provides context variables for accessing file changes, commits, and statistics:

##### Step-level Variables (Current Step)
- `${step.files_added}` - Files added in the current step
- `${step.files_modified}` - Files modified in the current step
- `${step.files_deleted}` - Files deleted in the current step
- `${step.files_changed}` - All files changed (added + modified + deleted)
- `${step.commits}` - Commit hashes created in the current step
- `${step.commit_count}` - Number of commits in the current step
- `${step.insertions}` - Lines inserted in the current step
- `${step.deletions}` - Lines deleted in the current step

##### Workflow-level Variables (Cumulative)
- `${workflow.files_added}` - All files added across the workflow
- `${workflow.files_modified}` - All files modified across the workflow
- `${workflow.files_deleted}` - All files deleted across the workflow
- `${workflow.files_changed}` - All files changed across the workflow
- `${workflow.commits}` - All commit hashes across the workflow
- `${workflow.commit_count}` - Total commits across the workflow
- `${workflow.insertions}` - Total lines inserted across the workflow
- `${workflow.deletions}` - Total lines deleted across the workflow

##### Pattern Filtering
Variables support pattern filtering using glob patterns:
```yaml
# Get only markdown files added
- shell: "echo '${step.files_added:*.md}'"

# Get only Rust source files modified
- claude: "/review ${step.files_modified:*.rs}"

# Get specific directory changes
- shell: "echo '${workflow.files_changed:src/*}'"
```

##### Format Modifiers
Control output format with modifiers:
```yaml
# JSON array format
- shell: "echo '${step.files_added:json}'"  # ["file1.rs", "file2.rs"]

# Newline-separated (for scripts)
- shell: "echo '${step.files_added:lines}'" # file1.rs\nfile2.rs

# Comma-separated
- shell: "echo '${step.files_added:csv}'"   # file1.rs,file2.rs

# Space-separated (default)
- shell: "echo '${step.files_added}'"       # file1.rs file2.rs
```

##### Example Usage
```yaml
name: code-review-workflow
steps:
  # Make changes
  - claude: "/implement feature X"
    commit_required: true

  # Review only the changed Rust files
  - claude: "/review-code ${step.files_modified:*.rs}"

  # Generate changelog for markdown files
  - shell: "echo 'Changed docs:' && echo '${step.files_added:*.md:lines}'"

  # Conditional execution based on changes
  - shell: "cargo test"
    when: "${step.files_modified:*.rs}"  # Only run if Rust files changed

  # Summary at the end
  - claude: |
      /summarize-changes
      Total files changed: ${workflow.files_changed:json}
      Commits created: ${workflow.commit_count}
      Lines added: ${workflow.insertions}
      Lines removed: ${workflow.deletions}
```

### Workflow Syntax

#### Write File Command

The `write_file` command allows workflows to create files with content, supporting multiple formats with validation and automatic formatting.

**Basic Syntax:**
```yaml
- write_file:
    path: "output/results.txt"
    content: "Processing complete!"
    format: text  # text, json, or yaml
    mode: "0644"  # Unix permissions (default: 0644)
    create_dirs: false  # Create parent directories (default: false)
```

**Supported Formats:**

1. **Text** (default) - Plain text with no processing:
```yaml
- write_file:
    path: "logs/build.log"
    content: "Build started at ${timestamp}"
    format: text
```

2. **JSON** - Validates and pretty-prints JSON:
```yaml
- write_file:
    path: "output/results.json"
    content: '{"status": "success", "items_processed": ${map.total}}'
    format: json
    create_dirs: true
```

3. **YAML** - Validates and formats YAML:
```yaml
- write_file:
    path: "config/settings.yml"
    content: |
      environment: production
      server:
        port: 8080
        host: localhost
    format: yaml
```

**Variable Interpolation:**

All fields support variable interpolation:
```yaml
# In MapReduce map phase
- write_file:
    path: "output/${item.name}.json"
    content: '{"id": "${item.id}", "processed": true}'
    format: json
    create_dirs: true

# In reduce phase
- write_file:
    path: "summary.txt"
    content: "Processed ${map.total} items, ${map.successful} successful"
    format: text
```

**Security Features:**
- Path traversal protection (rejects paths containing `..`)
- JSON/YAML validation before writing
- Configurable file permissions (Unix systems only)

**Common Use Cases:**

1. **Aggregating MapReduce results:**
```yaml
reduce:
  - write_file:
      path: "results/summary.json"
      content: '{"total": ${map.total}, "successful": ${map.successful}, "failed": ${map.failed}}'
      format: json
```

2. **Generating configuration files:**
```yaml
- write_file:
    path: ".config/app.yml"
    content: |
      name: ${PROJECT_NAME}
      version: ${VERSION}
      features:
        - authentication
        - caching
    format: yaml
```

3. **Creating executable scripts:**
```yaml
- write_file:
    path: "scripts/deploy.sh"
    content: |
      #!/bin/bash
      echo "Deploying ${APP_NAME}"
      ./deploy.sh --env production
    mode: "0755"
    create_dirs: true
```

#### Validation and Error Recovery

Prodigy supports multi-step validation and error recovery with two formats:

**Array Format** (for simple command sequences):
```yaml
validate:
  - shell: "prep-command-1"
  - shell: "prep-command-2"
  - claude: "/validate-result"
```

**Object Format** (when you need metadata like threshold, max_attempts, etc.):
```yaml
validate:
  commands:
    - shell: "prep-command-1"
    - shell: "prep-command-2"
    - claude: "/validate-result"
  result_file: "validation-results.json"
  threshold: 75  # Validation must score at least 75/100
  on_incomplete:
    commands:
      - claude: "/fix-gaps --gaps ${validation.gaps}"
      - shell: "rebuild-and-revalidate.sh"
    max_attempts: 3
    fail_workflow: false
```

**Key Points:**
- Use **array format** when you only need to run commands
- Use **object format** when you need to set `threshold`, `result_file`, `max_attempts`, or `fail_workflow`
- Fields like `threshold` and `max_attempts` belong at the config level, not on individual commands
- `on_incomplete` supports the same two formats (array or object with `commands:`)

**Example: Multi-step validation workflow**
```yaml
- claude: "/implement-feature spec.md"
  commit_required: true
  validate:
    commands:
      - shell: "cargo test"
      - shell: "cargo clippy"
      - claude: "/validate-implementation spec.md"
    result_file: ".prodigy/validation.json"
    threshold: 90
    on_incomplete:
      commands:
        - claude: "/fix-issues --gaps ${validation.gaps}"
        - shell: "cargo test"
      max_attempts: 5
      fail_workflow: true
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

📚 **Full documentation is available at [https://iepathos.github.io/prodigy](https://iepathos.github.io/prodigy)**

Quick links:
- [Getting Started](https://iepathos.github.io/prodigy/intro.html)
- [Workflow Basics](https://iepathos.github.io/prodigy/workflow-basics.html)
- [MapReduce Guide](https://iepathos.github.io/prodigy/mapreduce.html)
- [Command Reference](https://iepathos.github.io/prodigy/commands.html)
- [Examples](https://iepathos.github.io/prodigy/examples.html)

### Building Documentation Locally

```bash
# Install mdBook
cargo install mdbook

# Serve with live reload
mdbook serve book --open
```

### Additional Resources

- 📝 [Workflow Syntax (Single Page)](docs/workflow-syntax.md) - Complete syntax reference in one file
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
<summary><strong>Performance: Workflows running slowly</strong></summary>

1. Check parallel execution limits:
```bash
prodigy run workflow.yml --max-parallel 20
```

2. Enable verbose mode to identify bottlenecks:
```bash
prodigy run workflow.yml -v
```

Note: The `-v` flag also enables Claude streaming JSON output for debugging Claude interactions.

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
prodigy dlq retry <job-id> --max-parallel 5
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

<details>
<summary><strong>Verbosity: Controlling Claude streaming output</strong></summary>

Prodigy provides fine-grained control over Claude interaction visibility:

**Default behavior (no flags):**
```bash
prodigy run workflow.yml
# Shows progress and results, but no Claude JSON streaming output
```

**Verbose mode (-v):**
```bash
prodigy run workflow.yml -v
# Shows Claude streaming JSON output for debugging interactions
```

**Debug mode (-vv) and trace mode (-vvv):**
```bash
prodigy run workflow.yml -vv
prodigy run workflow.yml -vvv
# Also shows Claude streaming output plus additional internal logs
```

**Force Claude output (environment override):**
```bash
PRODIGY_CLAUDE_CONSOLE_OUTPUT=true prodigy run workflow.yml
# Shows Claude streaming output regardless of verbosity level
```

This allows you to keep normal runs clean while enabling detailed debugging when needed.
</details>

### Getting Help

- 🐛 [Report Issues](https://github.com/iepathos/prodigy/issues)
- 💬 [Discussions](https://github.com/iepathos/prodigy/discussions)
- 📧 [Email Support](mailto:iepathos@gmail.com)

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

Prodigy is licensed under MIT. See [LICENSE](LICENSE) for details.

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
# Test merge
