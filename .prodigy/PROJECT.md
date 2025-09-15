# Prodigy Project Status

## Current State: 100%

A workflow orchestration tool that executes Claude commands through structured YAML workflows with session state management and parallel execution through MapReduce patterns.

## What Exists

### Core Features ✅
- **Workflow Execution**: Sequential and parallel command execution
- **Session Management**: Persistent state tracking with timing
- **Claude Integration**: Direct Claude Code CLI integration
- **Shell Commands**: Full shell command support
- **MapReduce Processing**: Parallel execution across multiple agents with setup phase ✅
- **Foreach Iteration**: Simple parallel iteration without MapReduce complexity ✅
- **Error Handling**: Comprehensive failure recovery patterns
- **Retry Strategies**: Enhanced retry with configurable backoff, jitter, and circuit breakers ✅
- **Git Integration**: Worktree management and commit tracking
- **Goal-Seeking Primitives**: Iterative refinement with validation ✅

### Command Types ✅
- `claude:` - Execute Claude commands via Claude Code CLI
- `shell:` - Run shell commands with environment context
- `goal_seek:` - Iterative refinement with validation feedback ✅
- `foreach:` - Simple parallel iteration over items ✅
- `test:` - Test execution (deprecated, use shell instead)

### Storage Architecture ✅
- **Global Storage**: Centralized event and state management in `~/.prodigy/`
- **Event Tracking**: Cross-worktree event aggregation
- **Dead Letter Queue**: Failed item tracking and recovery
- **Session Persistence**: Session state and timing data

### CLI Commands ✅
- `prodigy run` - Execute workflows (simplified alias for cook) ✅
- `prodigy exec` - Execute single command with retry support ✅
- `prodigy batch` - Process multiple files in parallel ✅
- `prodigy resume` - Resume interrupted workflows with checkpoint recovery ✅
- `prodigy cook` - Execute workflows (original command)
- `prodigy goal-seek` - Standalone goal-seeking operations ✅
- `prodigy worktree` - Git worktree management
- `prodigy init` - Claude command initialization
- `prodigy events` - Event viewing and analysis
- `prodigy dlq` - Failed item management
- `prodigy sessions` - Session management

## Recent Additions

### Enhanced Retry Strategies ✅
- **Multiple Backoff Strategies**: Fixed, linear, exponential, fibonacci, custom
- **Configurable Jitter**: Prevents thundering herd with randomized delays
- **Selective Retry**: Retry only on specific error types (network, timeout, rate limit)
- **Retry Budget**: Maximum total time limit for retries
- **Circuit Breaker**: Automatic failure protection with recovery timeout
- **Per-Step Configuration**: Override global retry defaults at step level
- **Retry Metrics**: Detailed tracking of attempts and delays

### Step-Level On-Failure Handlers ✅
- **Single Command Handlers**: Execute a single recovery command on failure
- **Multiple Command Handlers**: Execute a sequence of recovery commands
- **Detailed Handler Configuration**: Strategy, timeout, and error handling options
- **Handler Strategies**: Recovery, fallback, cleanup, or custom approaches
- **Error Context Variables**: Access to error.message, error.exit_code, error.step
- **Recovery Detection**: Successful recovery handlers mark steps as recovered
- **Continue on Error**: Optionally continue execution after handler failures
- **Handler Timeouts**: Configurable timeout for handler execution

### Foreach Parallel Iteration ✅
- **Simple Iteration**: Alternative to MapReduce for simpler parallel operations
- **Command or List Input**: Execute command or iterate static list
- **Parallel Configuration**: Boolean or numeric parallel count
- **Do Block Execution**: Nested commands executed per item
- **Variable Interpolation**: ${item} available in nested commands
- **Continue on Error**: Optional failure tolerance
- **Progress Tracking**: Status updates during execution

### MapReduce Setup Phase ✅
- **Setup Command Execution**: Sequential setup commands before map phase
- **Dynamic Work Item Generation**: Setup can generate input files for map phase
- **Variable Capture and Passing**: Variables from setup available in map phase
- **File Creation Detection**: Automatic detection of generated work-items.json
- **Setup Failure Prevention**: Failed setup prevents map phase execution
- **Main Worktree Execution**: Setup runs in main worktree for consistency

### Workflow Resume Capability ✅
- **Checkpoint-Based Recovery**: Automatic checkpoint creation at configurable intervals
- **Step-Level Granularity**: Resume from exact point of failure
- **Variable State Preservation**: Full context restoration on resume
- **MapReduce Resume Support**: Partial completion handling for parallel jobs
- **Atomic Checkpoint Writes**: Corruption-resistant checkpoint persistence
- **Resume Options**: Force resume, step selection, failure reset
- **Checkpoint Management**: List, load, and delete checkpoint operations

### Simplified CLI Interface ✅
- **`prodigy run`**: Intuitive alias for cook command
- **`prodigy exec`**: Single command execution with retry support
- **`prodigy batch`**: Parallel file processing with MapReduce
- **`prodigy resume`**: Resume interrupted workflows with full state recovery
- **Workflow Generation**: Automatic YAML generation for simple operations
- **Smart Defaults**: Sensible retry, timeout, and parallelism settings

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
- `cli/`: CLI command handlers and workflow generation ✅
- `cli/workflow_generator/`: Dynamic workflow generation for simple commands ✅
- `cook/`: Workflow orchestration and execution
- `cook/goal_seek/`: Goal-seeking primitives and validators ✅
- `cook/execution/`: Command execution and MapReduce processing
- `cook/workflow/`: Workflow parsing and step management
- `cook/workflow/checkpoint/`: Checkpoint creation and persistence ✅
- `cook/workflow/resume/`: Resume execution from checkpoints ✅
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

### Simplified CLI Commands ✅
```bash
# Run a workflow
prodigy run workflow.yml

# Execute single command with retries
prodigy exec "claude: /refactor app.py" --retry 3

# Process files in parallel
prodigy batch "*.py" --command "claude: /add-types" --parallel 5

# Resume interrupted workflow
prodigy resume workflow-123
```

### Basic Workflow
```yaml
- claude: "/implement-feature"
  commit_required: true
- shell: "cargo test"
  timeout: 300
```

### Workflow with Retry Strategies ✅
```yaml
# Global retry defaults for all steps
retry_defaults:
  attempts: 3
  backoff: exponential
  initial_delay: 2s
  max_delay: 30s
  jitter: true

steps:
  - shell: "curl https://api.example.com/data"
    retry:
      attempts: 5
      backoff:
        exponential:
          base: 2.0
      retry_on: [network, timeout, rate_limit]
      retry_budget: 2m

  - claude: "/process-critical-data"
    retry:
      attempts: 1  # No retry for critical operations
```

### Workflow with On-Failure Handlers ✅
```yaml
# Simple handler
- shell: "npm run build"
  on_failure: "npm cache clean --force && npm install"

# Multiple commands
- shell: "cargo test"
  on_failure:
    - "cargo clean"
    - "cargo build"
    - "cargo test"

# Detailed configuration with recovery strategy
- shell: "deploy.sh production"
  on_failure:
    strategy: recovery
    commands:
      - shell: "rollback.sh"
      - shell: "validate-rollback.sh"
    timeout: 300
    fail_workflow: false

# Using error context variables
- shell: "critical-operation"
  on_failure:
    - "echo 'Operation failed with exit code: ${error.exit_code}'"
    - "send-alert '${error.step} failed: ${error.message}'"
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

### Foreach Workflow ✅
```yaml
- foreach:
    foreach: "find . -name '*.js'"
    parallel: 10
    do:
      - shell: "cp ${item} ${item}.backup"
      - claude: "/convert-to-typescript ${item}"
    continue_on_error: true
```

### MapReduce with Setup Phase ✅
```yaml
setup:
  - shell: "npm run analyze"
  - shell: "generate-work-items > work-items.json"

map:
  input: work-items.json
  json_path: "$[*]"
  agent_template:
    commands:
      - claude: "/process-item ${item}"

reduce:
  commands:
    - claude: "/aggregate-results ${map.results}"
```

## Technical Debt

### Known Issues
- DLQ reprocessing not yet implemented (command exists but returns error)
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