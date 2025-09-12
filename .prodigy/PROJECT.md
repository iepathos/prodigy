# Prodigy Project Status

## Current State: 95%

A workflow orchestration tool that executes Claude commands through structured YAML workflows with session state management and parallel execution through MapReduce patterns.

## What Exists

### Core Features ✅
- **Workflow Execution**: Sequential and parallel command execution
- **Session Management**: Persistent state tracking with timing
- **Claude Integration**: Direct Claude Code CLI integration
- **Shell Commands**: Full shell command support
- **MapReduce Processing**: Parallel execution across multiple agents
- **Error Handling**: Comprehensive failure recovery patterns
- **Git Integration**: Worktree management and commit tracking
- **Goal-Seeking Primitives**: Iterative refinement with validation ✅

### Command Types ✅
- `claude:` - Execute Claude commands via Claude Code CLI
- `shell:` - Run shell commands with environment context
- `goal_seek:` - Iterative refinement with validation feedback ✅
- `test:` - Test execution (deprecated, use shell instead)

### Storage Architecture ✅
- **Global Storage**: Centralized event and state management in `~/.prodigy/`
- **Event Tracking**: Cross-worktree event aggregation
- **Dead Letter Queue**: Failed item tracking and recovery
- **Session Persistence**: Session state and timing data

### CLI Commands ✅
- `prodigy cook` - Execute workflows
- `prodigy goal-seek` - Standalone goal-seeking operations ✅
- `prodigy worktree` - Git worktree management
- `prodigy init` - Claude command initialization
- `prodigy events` - Event viewing and analysis
- `prodigy dlq` - Failed item management
- `prodigy sessions` - Session management

## Recent Additions

### Goal-Seeking System ✅
- **Iterative Refinement Engine**: Multi-attempt execution with validation
- **Built-in Validators**: Test pass, spec coverage, output quality validators
- **CLI Integration**: `prodigy goal-seek` command for standalone usage
- **Workflow Integration**: `goal_seek:` command type in YAML workflows
- **Context Passing**: Environment variables for attempt history
- **Convergence Detection**: Automatic stopping when no improvement
- **Flexible Validation**: JSON and text score extraction

## Architecture

### Core Modules
- `cook/`: Workflow orchestration and execution
- `cook/goal_seek/`: Goal-seeking primitives and validators ✅
- `cook/execution/`: Command execution and MapReduce processing
- `cook/workflow/`: Workflow parsing and step management
- `config/`: Configuration management and command discovery
- `session/`: Session state and timing tracking

### Key Traits
- `CommandExecutor`: Pluggable command execution
- `Validator`: Goal-seeking validation framework ✅
- `SubprocessExecutor`: Shell command execution
- `SessionManager`: Session lifecycle management

## Capabilities

### Workflow Features
- Variable interpolation (`${var}`, `${shell.output}`)
- Conditional execution and error handling
- Output capture and cross-step communication
- Timeout configuration and resource limits
- Commit requirements and git integration

### Goal-Seeking Features ✅
- Score-based validation (0-100 range)
- Multiple termination conditions (success, timeout, convergence, max attempts)
- Environment context for refinement attempts
- Built-in and custom validator support
- CLI and YAML workflow integration

### Quality Assurance
- Comprehensive test suite with 1000+ tests
- Continuous integration pipeline
- Code coverage tracking
- Linting and formatting enforcement

## Usage Patterns

### Basic Workflow
```yaml
- claude: "/implement-feature"
  commit_required: true
- shell: "cargo test"
  timeout: 300
```

### Goal-Seeking Workflow ✅
```yaml
- goal_seek:
    goal: "Fix all failing tests"
    command: "claude: /debug-test-failure"
    validate: "cargo test && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 5
```

### MapReduce Processing
```yaml
agent_template:
  - claude: "/process-item ${item}"
map:
  - path: "*.md"
reduce:
  - claude: "/aggregate-results ${map.results}"
```

## Technical Debt

### Known Issues
- DLQ reprocessing not yet implemented (command exists but returns error)
- Job resumption shows status only (actual resumption not implemented)
- Context directory feature planned but not implemented

### Performance Optimizations Needed
- Event log cleanup and archival
- Session state compression
- Parallel validation in goal-seeking

## Dependencies

### Core Dependencies
- `clap`: Command-line parsing
- `serde`/`serde_yaml`: Configuration serialization
- `tokio`: Async runtime
- `anyhow`: Error handling
- `tracing`: Logging and observability

### Goal-Seeking Dependencies ✅
- `regex`: Score extraction from text
- `serde_json`: JSON validation parsing

## Development Status

The project is feature-complete for core workflow orchestration with the recent addition of goal-seeking primitives. Current focus is on stability, performance optimization, and expanding the built-in validator library.