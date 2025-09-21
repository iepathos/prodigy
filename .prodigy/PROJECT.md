# Prodigy Project Status

## Current State: 100%

A workflow orchestration tool that executes Claude commands through structured YAML workflows with session state management and parallel execution through MapReduce patterns.

## What Exists

### Core Features ✅
- **Workflow Execution**: Sequential and parallel command execution
- **Session Management**: Persistent state tracking with timing
- **Claude Integration**: Direct Claude Code CLI integration with transparent streaming
- **Shell Commands**: Full shell command support
- **MapReduce Processing**: Parallel execution across multiple agents with setup phase ✅
- **Foreach Iteration**: Simple parallel iteration without MapReduce complexity ✅
- **Error Handling**: Comprehensive failure recovery patterns with unified error system ✅
- **Retry Strategies**: Enhanced retry with configurable backoff, jitter, and circuit breakers ✅
- **Git Integration**: Worktree management and commit tracking
- **Git Context Variables**: Automatic tracking of file changes exposed as workflow variables ✅
- **Goal-Seeking Primitives**: Iterative refinement with validation ✅
- **Worktree Pool Management**: Sophisticated worktree pooling with allocation strategies ✅
- **Workflow Composition**: Build complex workflows from reusable components ✅
- **Environment Management**: Comprehensive environment variable and working directory control ✅
- **Real-time Streaming Infrastructure**: Line-by-line output capture with stream processors ✅
- **Claude Streaming Integration**: Real-time Claude output visibility with tool invocation tracking ✅
- **Storage Abstraction Layer**: Trait-based storage supporting file and database backends ✅
- **Configurable Merge Workflows**: Custom merge strategies with variable interpolation ✅
- **Transparent Claude Logging**: Consistent streaming output for all Claude operations ✅

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
- `prodigy run` - Execute workflows (primary command) ✅
- `prodigy exec` - Execute single command with retry support ✅
- `prodigy batch` - Process multiple files in parallel ✅
- `prodigy resume` - Resume interrupted workflows with checkpoint recovery ✅
- `prodigy resume-job` - Resume MapReduce jobs with detailed progress reporting ✅
- `prodigy goal-seek` - Standalone goal-seeking operations ✅
- `prodigy worktree` - Git worktree management
- `prodigy init` - Claude command initialization
- `prodigy events` - Event viewing and analysis
- `prodigy dlq` - Failed item management
- `prodigy sessions` - Session management
- `prodigy analytics` - Claude session correlation and analytics ✅
- **Man Pages**: Comprehensive Unix-style documentation for all commands ✅

## Recent Additions

### Unified Error Handling System ✅
- **Structured Error Types**: Hierarchical error categories for all modules
- **Error Codes**: Machine-readable codes (E0001-E9999) for automation
- **Context Chaining**: Full error chain with source tracking
- **User-Friendly Messages**: Separate user and developer error messages
- **Recovery Detection**: Built-in identification of recoverable errors
- **Migration Helpers**: Extension traits and macros for easy adoption
- **Comprehensive Coverage**: 8 error categories covering all operations
- **Type-Safe Conversions**: Automatic conversion from common error types

### Git Context Variables ✅
- **Automatic Git Change Tracking**: File changes tracked for each workflow step
- **Step-Level Variables**: Access files added/modified/deleted per step
- **Workflow-Level Variables**: Cumulative changes across all steps
- **Variable Types**: files_added, files_modified, files_deleted, commits, insertions, deletions
- **Pattern Filtering**: Filter files by glob patterns (e.g., `${step.files_added:*.md}`)
- **Format Options**: Space-separated (default), newline, JSON array, comma-separated
- **Lazy Evaluation**: Changes calculated only when variables are used
- **Non-Git Support**: Gracefully handles workflows outside git repositories

### CLI Documentation (Man Pages) ✅
- **Comprehensive Man Pages**: Auto-generated man pages for all commands and subcommands
- **Standard Unix Format**: Follows groff/nroff conventions with proper sections
- **Automatic Generation**: Build-time generation from CLI definitions using clap_mangen
- **Installation Script**: Easy installation to system or user man directories
- **Complete Coverage**: NAME, SYNOPSIS, DESCRIPTION, OPTIONS, EXAMPLES, ENVIRONMENT, EXIT STATUS
- **Compressed Support**: Both uncompressed and gzipped versions generated
- **Cross-Reference**: SEE ALSO sections link related commands

### Claude Streaming Integration ✅
- **Real-time Claude Output**: See Claude's tool invocations and messages as they happen
- **Claude JSON Processor**: Specialized handler for Claude's stream-json format
- **Event-Based Processing**: Tool invocations, token usage, and session events
- **Console Display Options**: Configurable real-time output with emojis for tool activity
- **CommandRunner Extension**: Optional streaming method with fallback to buffered mode
- **Environment Control**: PRODIGY_CLAUDE_STREAMING=true enables real-time mode
- **Event Logger Integration**: Automatic MapReduceEvent logging for analytics
- **Mock Testing Support**: MockCommandRunner extended for streaming scenarios

### Real-time Streaming Infrastructure ✅
- **Line-by-Line Output Capture**: Process command output as it arrives
- **Stream Processors**: JSON line parsing, pattern matching, logging
- **Backpressure Management**: Handle fast producers with configurable strategies
- **Full Backward Compatibility**: Opt-in streaming, batch mode remains default
- **Integration with CommandRunner**: Seamless ExecutionContext configuration
- **Rate Limiting**: Prevent downstream system overload
- **Buffer Management**: Memory-efficient line buffering

### Claude Session Analytics ✅
- **Session Correlation**: Extract and correlate session IDs with JSONL files
- **Analytics Engine**: Calculate costs, tool usage, and performance metrics
- **Session Replay**: Step through session history with bookmarks and playback
- **Cost Tracking**: Token usage and cost projections based on Claude pricing
- **Performance Analysis**: Identify bottlenecks and optimization opportunities
- **Tool Usage Statistics**: Analyze tool invocation patterns and success rates
- **Optimization Recommendations**: Automated suggestions for cost and performance

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

### Workflow Composition and Reusability ✅
- **Import Workflows**: Import workflows from other files with aliasing
- **Template System**: Define and use reusable workflow templates
- **Parameter Support**: Parameterize workflows with type-safe inputs
- **Sub-Workflows**: Compose workflows from sub-workflow components
- **Template Registry**: Store and retrieve workflow templates
- **Inheritance**: Extend base workflows with overrides
- **Selective Imports**: Import specific components from workflows
- **Circular Dependency Detection**: Validate composition dependencies

### Worktree Pool Management ✅
- **Pool Allocation Strategies**: OnDemand, Pooled, Reuse, and Dedicated modes
- **Named Worktrees**: Support for experiment-specific named worktrees
- **Resource Limits**: Configurable disk, memory, and CPU limits per worktree
- **Cleanup Policies**: Automatic cleanup with idle timeout and age limits
- **Worktree Reuse**: Intelligent reuse based on branch prefix and usage patterns
- **Pool Metrics**: Tracking of creation, reuse, and utilization statistics
- **Handle-Based Access**: Automatic release with RAII pattern
- **Cross-Job Sharing**: Worktree pools can be shared across MapReduce jobs

### Environment Variables and Working Directory Control ✅
- **Global Environment Variables**: Workflow-wide environment configuration
- **Per-Step Environment Override**: Step-specific environment variables
- **Working Directory Control**: Per-step working directory specification
- **Dynamic Environment Values**: Command-based value computation with caching
- **Conditional Environment**: Environment based on expressions and conditions
- **Secret Management**: Secure handling with masking in logs
- **Environment Profiles**: Named profiles for different contexts (dev, test, prod)
- **Environment Files**: Support for .env file loading
- **Path Expansion**: Cross-platform path resolution with variable expansion
- **Environment Inheritance**: Configurable parent process inheritance
- **Temporary Environments**: Restore after step completion

### MapReduce Job Resumption ✅
- **Job Status Reporting**: Detailed progress and completion statistics
- **Checkpoint-Based Recovery**: Resume MapReduce jobs from exact failure point
- **Progress Monitoring**: Real-time tracking of agent execution status
- **Failed Item Reprocessing**: Automatic retry of failed work items
- **Force Resume Option**: Override complete status to reprocess items
- **DLQ Integration**: Direct access to Dead Letter Queue for failed items
- **Verbose Status Display**: Comprehensive job state visualization
- **Enhanced Resume Manager**: Robust state restoration and work item management ✅
- **Cross-Worktree Synchronization**: Event log continuation and agent coordination ✅
- **Phase-Based Resume**: Resume from setup, map, or reduce phases ✅
- **Environment Validation**: Consistency checks during resume ✅

## Architecture

### Core Modules
- `cli/`: CLI command handlers and workflow generation ✅
- `cli/workflow_generator/`: Dynamic workflow generation for simple commands ✅
- `cook/`: Workflow orchestration and execution
- `cook/goal_seek/`: Goal-seeking primitives and validators ✅
- `cook/execution/`: Command execution and MapReduce processing
- `cook/execution/mapreduce/phases/`: Phase execution orchestration ✅
- `cook/workflow/`: Workflow parsing and step management
- `cook/workflow/checkpoint/`: Checkpoint creation and persistence ✅
- `cook/workflow/resume/`: Resume execution from checkpoints ✅
- `config/`: Configuration management and command discovery
- `session/`: Session state and timing tracking
- `worktree/`: Git worktree management and pooling ✅
- `worktree/pool.rs`: Worktree pool allocation and lifecycle ✅

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
- Input module testing with 30 tests (27 passing)
- CLI integration tests covering all commands ✅
- Continuous integration pipeline
- Code coverage tracking (CLI module now 60%+) ✅
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

### Worktree Pool Configuration ✅
```yaml
# Global worktree pool configuration
worktree_config:
  parallel_worktrees: 20
  allocation_strategy: pooled
  cleanup_policy:
    idle_timeout_secs: 300
    max_age_secs: 3600
    cleanup_on_complete: true
    keep_failed: false
  resource_limits:
    max_disk_mb: 1000
    max_memory_mb: 512

# Named worktrees for experiments
tasks:
  - name: "Experiment A"
    worktree: "experiment-a"  # Named worktree
    commands:
      - claude: "/approach-a"

  - name: "Experiment B"
    worktree: "experiment-b"  # Different named worktree
    commands:
      - claude: "/approach-b"
```

### Environment Configuration ✅
```yaml
# Global environment configuration
env:
  NODE_ENV: production
  API_URL: https://api.example.com
  WORKERS:
    command: "nproc"
    cache: true

secrets:
  API_KEY: ${vault:api/keys/production}
  DB_PASSWORD: ${env:SECRET_DB_PASS}

env_files:
  - .env.production

profiles:
  development:
    NODE_ENV: development
    API_URL: http://localhost:3000

# Step with environment override
steps:
  - shell: "npm run build"
    env:
      BUILD_TARGET: production
    working_dir: ./frontend

  - shell: "pytest"
    working_dir: ./backend
    env:
      PYTHONPATH: ./src:./tests
    temporary: true  # Restore environment after step
```

## Technical Debt

### Known Issues
- DLQ reprocessing not yet implemented (command exists but returns error)
- Context directory feature planned but not implemented

### Performance Optimizations Needed
- Event log cleanup and archival
- Session state compression
- Parallel validation in goal-seeking

### Recent Improvements
- **Error Handling**: Replaced critical `.unwrap()` calls with proper error handling in:
  - Orchestrator module (MapReduce configuration validation)
  - Storage backends (duration conversion, timestamp comparisons)
  - Main CLI (file operations, job selection)
  - Worktree manager (path operations)
  - Data pipeline (expression parsing)
- **Error Context**: Added descriptive error messages with context for better debugging
- **Graceful Degradation**: System now fails gracefully with helpful messages instead of panicking

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