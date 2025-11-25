# Prodigy Project Documentation

This document contains Prodigy-specific documentation for Claude. General development guidelines are in `~/.claude/CLAUDE.md`.

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It manages session state, tracks execution progress, and supports parallel execution through MapReduce patterns.

## Error Handling (Spec 101, 168)

### Core Rules
- **Production code**: Never use `unwrap()` or `panic!()` - use Result types and `?` operator
- **Test code**: May use `unwrap()` and `panic!()` for test failures
- **Static patterns**: Compile-time constants (like regex) may use `expect()`

### Error Types
- Storage: `StorageError`
- Worktree: `WorktreeError`
- Command execution: `CommandError`
- General: `anyhow::Error`

### Context Preservation
Prodigy uses Stillwater's `ContextError<E>` to preserve operation context through the call stack:

```rust
use prodigy::cook::error::ResultExt;

fn process_item(id: &str) -> Result<(), ContextError<ProcessError>> {
    create_worktree(id).with_context(|| format!("Creating worktree for {}", id))?;
    execute_commands(id).context("Executing commands")?;
    Ok(())
}
```

**Benefits**: Full context trail in error messages, DLQ integration, zero runtime overhead on success path.

## Claude Observability (Spec 121)

### JSON Log Tracking
Claude Code creates detailed JSON logs at `~/.local/state/claude/logs/session-{id}.json` containing:
- Complete message history and tool invocations
- Token usage and session metadata
- Error details and stack traces

**Access logs:**
- Verbose mode: `prodigy run workflow.yml -v` shows log path after each command
- Programmatically: `result.json_log_location()`
- MapReduce events: `AgentCompleted` and `AgentFailed` include `json_log_location`
- DLQ items: `FailureDetail` preserves log location

**Debug failed agents:**
```bash
# Get log path from DLQ
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Inspect the log
cat /path/to/log.json | jq '.messages[-3:]'
```

## Custom Merge Workflows

Define custom merge workflows with validation, testing, and conflict resolution:

```yaml
merge:
  commands:
    - shell: "git fetch origin && git merge origin/main"
    - shell: "cargo test && cargo clippy"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600
```

**Available variables:**
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch (worktree)
- `${merge.target_branch}` - Target branch (original branch)
- `${merge.session_id}` - Session ID

**Streaming**: Use `-v` flag or set `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` for real-time output.

## MapReduce Workflows

### Basic Structure
```yaml
name: workflow-name
mode: mapreduce

setup:
  - shell: "generate-work-items.sh"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 10

  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total}'"
```

### Commit Validation (Spec 163)
Commands with `commit_required: true` enforce commit creation. Agent fails if no commit is made.

```yaml
agent_template:
  - shell: |
      echo "data" > file.txt
      git add file.txt
      git commit -m "Add data"
    commit_required: true
```

**Validation behavior:**
- HEAD SHA checked before/after command
- No new commits → agent fails with `CommitValidationFailed`
- Failed agents added to DLQ with full context
- Agents with commits: merged to parent; without commits: cleaned up

### Checkpoint & Resume (Spec 134)
Prodigy checkpoints all phases (setup, map, reduce) for recovery:

**Resume commands:**
```bash
prodigy resume <session-or-job-id>
prodigy resume-job <job_id>
```

**State preservation:**
- Setup: Checkpoint after completion
- Map: Checkpoint after configurable work items processed
- Reduce: Checkpoint after each command
- Variables, outputs, and agent results preserved

**Storage:** `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`

### Concurrent Resume Protection (Spec 140)
RAII-based locking prevents multiple resume processes:
- Exclusive lock acquired automatically before resume
- Lock released on completion or failure
- Stale locks (crashed processes) auto-detected and cleaned
- Lock files: `~/.prodigy/resume_locks/{id}.lock`

### Worktree Isolation (Spec 127)
All phases execute in isolated worktrees:

```
original_branch → parent worktree (session-xxx)
                  ├→ Setup executes here
                  ├→ Agent worktrees (branch from parent, merge back)
                  ├→ Reduce executes here
                  └→ User prompt: Merge to {original_branch}?
```

**Benefits:** Main repo untouched, parallel execution, full isolation, user-controlled merge.

### Cleanup Handling (Spec 136)
Agent success independent of cleanup status:
- Successful agents preserved even if cleanup fails
- Orphaned worktrees registered: `~/.prodigy/orphaned_worktrees/{repo}/{job_id}.json`
- Clean orphaned: `prodigy worktree clean-orphaned <job_id>`

### Dead Letter Queue (DLQ)
Failed items stored in `~/.prodigy/dlq/{repo}/{job_id}/` with:
- Original work item data
- Failure reason, timestamp, error context
- JSON log location for debugging

**Retry failed items:**
```bash
prodigy dlq retry <job_id> [--max-parallel N] [--dry-run]
```

## Variable Aggregation (Spec 171)

### Semigroup-Based Aggregation with Validation

Prodigy uses a semigroup pattern with homogeneous validation for aggregating variables across parallel agents in MapReduce workflows. This provides a clean, composable abstraction for combining results while preventing type mismatches.

**Core concept**: Each aggregate type implements the `Semigroup` trait with an associative `combine` operation, enabling safe parallel aggregation. Following Stillwater's "pure core, imperative shell" philosophy, validation happens at boundaries before combining.

**Type Safety**: The `aggregate_map_results` function uses Stillwater's homogeneous validation to ensure all results have the same type before combining. If any type mismatches occur, ALL errors are accumulated and reported (not just the first).

### Available Aggregations

All aggregations are implemented via `AggregateResult` enum in `src/cook/execution/variables/semigroup.rs`:

- **Count**: Count items (`Count(n)`)
- **Sum**: Sum numeric values (`Sum(total)`)
- **Min/Max**: Track minimum/maximum values
- **Collect**: Collect all values into array
- **Average**: Calculate average (tracks sum and count internally)
- **Median**: Calculate median (collects all values, computes on finalize)
- **StdDev/Variance**: Calculate statistical measures
- **Unique**: Collect unique values (using HashSet)
- **Concat**: Concatenate strings
- **Merge**: Merge objects (first value wins for duplicate keys)
- **Flatten**: Flatten nested arrays
- **Sort**: Sort values (ascending or descending)
- **GroupBy**: Group values by key

### Using Aggregations in Workflows

```yaml
map:
  input: "items.json"
  json_path: "$.items[*]"

  variables:
    total_count:
      type: count
      initial: 0

    total_size:
      type: sum
      initial: 0

    all_tags:
      type: unique
      initial: []

reduce:
  - shell: "echo 'Processed ${total_count} items with total size ${total_size}'"
  - shell: "echo 'Unique tags: ${all_tags}'"
```

### Helper Functions with Validation

For programmatic aggregation, use the validation-aware helper functions:

```rust
use prodigy::cook::execution::variables::semigroup::{
    AggregateResult, aggregate_map_results, aggregate_with_initial
};
use stillwater::Validation;

// Combine multiple results with validation
let results = vec![
    AggregateResult::Count(5),
    AggregateResult::Count(3),
    AggregateResult::Count(2),
];

match aggregate_map_results(results) {
    Validation::Success(total) => {
        // All types matched: Count(10)
        println!("Total: {:?}", total);
    }
    Validation::Failure(errors) => {
        // Type mismatches found - ALL errors reported
        for error in errors {
            eprintln!("Error: {}", error);
        }
    }
}

// Combine with initial value (useful for checkpointed aggregation)
let initial = AggregateResult::Count(10);
let new_results = vec![
    AggregateResult::Count(5),
    AggregateResult::Count(3),
];

match aggregate_with_initial(initial, new_results) {
    Validation::Success(total) => {
        // Count(18)
        println!("Total: {:?}", total);
    }
    Validation::Failure(errors) => {
        for error in errors {
            eprintln!("Error: {}", error);
        }
    }
}
```

### Migration Guide

**Migrating custom aggregation logic to semigroup pattern:**

#### Before (Custom Logic)
```rust
// Old approach: custom merge logic scattered across codebase
fn merge_counts(a: usize, b: usize) -> usize {
    a + b
}

fn merge_collections(mut a: Vec<Value>, b: Vec<Value>) -> Vec<Value> {
    a.extend(b);
    a
}

fn merge_averages(avg_a: f64, count_a: usize, avg_b: f64, count_b: usize) -> (f64, usize) {
    let sum_a = avg_a * count_a as f64;
    let sum_b = avg_b * count_b as f64;
    let total_sum = sum_a + sum_b;
    let total_count = count_a + count_b;
    (total_sum / total_count as f64, total_count)
}
```

#### After (Semigroup Pattern)
```rust
use prodigy::cook::execution::variables::semigroup::{AggregateResult, aggregate_results};
use stillwater::Semigroup;

// New approach: unified semigroup combine operation

// Count
let total = AggregateResult::Count(5).combine(AggregateResult::Count(3));
// Result: Count(8)

// Collections
let all_values = AggregateResult::Collect(vec![val1])
    .combine(AggregateResult::Collect(vec![val2]));
// Result: Collect([val1, val2])

// Averages (semigroup tracks sum and count internally)
let combined_avg = AggregateResult::Average(10.0, 2)  // avg: 5.0
    .combine(AggregateResult::Average(20.0, 3));      // avg: 6.67
// Result: Average(30.0, 5) which finalizes to 6.0

// Finalize to get the final value
let final_value = combined_avg.finalize(); // Value::Number(6.0)
```

**Key benefits:**
- **Unified interface**: Single `combine` method for all aggregations
- **Associativity**: Safe parallel execution and checkpointing
- **Type safety**: Validation catches type mismatches before combining
- **Error accumulation**: ALL errors reported, not just the first
- **No panics**: Production code never panics on type mismatches
- **Composability**: Easy to extend with new aggregation types
- **Clarity**: Separates state tracking (combine) from final computation (finalize)

**Migration steps:**
1. Identify custom aggregation logic in your code
2. Find matching `AggregateResult` variant (or add new one if needed)
3. Replace custom merge functions with `aggregate_map_results()` (with validation)
4. Handle `Validation::Success` and `Validation::Failure` cases
5. Use `.finalize()` to get final computed values

## Effect-Based Parallelism (Spec 173)

### Overview

Prodigy uses Stillwater's `Effect` pattern to separate pure business logic from I/O operations in MapReduce execution. This enables:
- **Testability**: Pure functions can be tested without I/O
- **Composability**: Effects chain together with `and_then`, `map`
- **Parallelism**: `par_all` and `par_all_limit` for bounded concurrency
- **Mock environments**: Test with fake dependencies

### Pure Work Planning

Work assignment planning is pure and deterministic:

```rust
use prodigy::cook::execution::mapreduce::pure::work_planning::{
    plan_work_assignments, WorkPlanConfig, FilterExpression
};

// Pure: no I/O, fully testable
let config = WorkPlanConfig {
    filter: Some(FilterExpression::Equals {
        field: "type".to_string(),
        value: json!("important"),
    }),
    offset: 0,
    max_items: Some(100),
};

let assignments = plan_work_assignments(items, &config);
```

### Dependency Analysis

Command dependencies are analyzed to enable safe parallel execution:

```rust
use prodigy::cook::execution::mapreduce::pure::dependency_analysis::{
    analyze_dependencies, Command
};

// Pure: analyzes read/write sets to detect dependencies
let commands = vec![
    Command { reads: set![], writes: set!["A"] },
    Command { reads: set!["A"], writes: set!["B"] },
    Command { reads: set!["A"], writes: set!["C"] },
];

let graph = analyze_dependencies(&commands);
let batches = graph.parallel_batches();

// Result: [[0], [1, 2]] - command 0 first, then 1 and 2 in parallel
```

### Effect-Based I/O

All I/O operations are wrapped in Effects:

```rust
use prodigy::cook::execution::mapreduce::effects::{
    create_worktree_effect, execute_commands_effect, merge_to_parent_effect
};
use stillwater::Effect;

// Compose effects sequentially
let agent_effect = create_worktree_effect("agent-0", "main")
    .and_then(|worktree| {
        execute_commands_effect(&item, &worktree)
            .map(move |result| (worktree, result))
    })
    .and_then(|(worktree, result)| {
        merge_to_parent_effect(&worktree, "main")
            .map(move |_| result)
    });

// Execute with environment
let result = agent_effect.run_async(&env).await?;
```

### Parallel Execution

Use `par_all_limit` for bounded parallel execution:

```rust
let effects: Vec<_> = assignments
    .into_iter()
    .map(|assignment| execute_agent_effect(assignment))
    .collect();

// Execute with concurrency limit
let results = Effect::par_all_limit(effects, max_parallel)
    .run_async(&env)
    .await?;
```

### Environment Types

Dependencies are provided through environment types:

```rust
use prodigy::cook::execution::mapreduce::environment::{MapEnv, PhaseEnv};

// Map phase environment
let env = MapEnv::new(
    config,
    worktree_manager,
    command_executor,
    agent_template,
    storage,
    workflow_env,
);

// Phase environment (setup/reduce)
let phase_env = PhaseEnv::new(
    command_executor,
    storage,
    variables,
    workflow_env,
);
```

### Testing with Effects

Pure functions are tested without I/O:

```rust
#[test]
fn test_work_planning() {
    let items = vec![json!({"type": "a"}), json!({"type": "b"})];
    let config = WorkPlanConfig { /* ... */ };

    let assignments = plan_work_assignments(items, &config);

    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].item["type"], "a");
}
```

Effects are tested with mock environments:

```rust
#[tokio::test]
async fn test_agent_execution() {
    let mock_env = MockMapEnv::default();
    let effect = execute_agent_effect(assignment);

    let result = effect.run_async(&mock_env).await;
    assert!(result.is_ok());
}
```

### Architecture

The MapReduce implementation follows "pure core, imperative shell":

```
src/cook/execution/mapreduce/
├── pure/                       # Pure functions (no I/O)
│   ├── work_planning.rs        # Work assignment planning
│   └── dependency_analysis.rs  # Command dependency graphs
├── effects/                    # I/O effects
│   ├── worktree.rs            # Worktree operations
│   ├── commands.rs            # Command execution
│   └── merge.rs               # Merge operations
└── environment.rs             # Environment types (MapEnv, PhaseEnv)
```

**Benefits:**
- Pure core: Testable, composable, deterministic
- Effects: Type-safe I/O with error handling
- Parallelism: Safe bounded concurrency
- Mocking: Test without actual I/O

## Reader Pattern Environment Access (Spec 175)

### Overview

Prodigy provides Reader pattern helpers for clean, type-safe environment access in Effect-based code. These helpers use Stillwater's `Effect::asks` for reading environment values and `Effect::local` for scoped modifications.

### Environment Access Helpers

Access environment values without manual `Effect::asks` boilerplate:

```rust
use prodigy::cook::execution::mapreduce::environment_helpers::*;

// MapEnv accessors
let max_parallel = get_max_parallel().run_async(&env).await?;
let job_id = get_job_id().run_async(&env).await?;
let config_value = get_config_value("timeout").run_async(&env).await?;
let storage = get_storage().run_async(&env).await?;
let worktree_manager = get_worktree_manager().run_async(&env).await?;

// PhaseEnv accessors
let variable = get_variable("count").run_async(&phase_env).await?;
let all_variables = get_variables().run_async(&phase_env).await?;
let workflow_var = get_workflow_env_value("debug").run_async(&phase_env).await?;
```

### Local Override Utilities

Use `Effect::local` wrappers for scoped environment modifications:

```rust
use prodigy::cook::execution::mapreduce::environment_helpers::*;

// Override max_parallel for a specific effect
let effect = with_max_parallel(50, expensive_operation());
let result = effect.run_async(&env).await?;
// Original env unchanged - override only applies within effect

// Enable debug mode for an effect
let debug_effect = with_debug(true, agent_execution());

// Add config values for an effect
let effect = with_config("timeout", json!(60), long_running_operation());

// Override multiple values
let effect = with_overrides(
    |env| MapEnv {
        max_parallel: 100,
        job_id: "override-job".to_string(),
        ..env.clone()
    },
    batch_operation(),
);

// PhaseEnv: Override variables for an effect
let effect = with_variables(
    HashMap::from([("count".to_string(), json!(42))]),
    compute_result(),
);
```

### Composing Effects with Environment Access

Combine environment access with effect composition:

```rust
use prodigy::cook::execution::mapreduce::environment_helpers::*;
use stillwater::Effect;

// Read config, then execute based on value
let effect = get_config_value("batch_size")
    .and_then(|batch_size| {
        let size = batch_size.unwrap_or(json!(10)).as_u64().unwrap() as usize;
        process_in_batches(items, size)
    });

// Use local override for nested operation
let effect = get_max_parallel()
    .and_then(|current_max| {
        let reduced = current_max / 2;
        with_max_parallel(reduced, memory_intensive_operation())
    });
```

### Mock Environment Builders

Test Reader pattern code without production dependencies:

```rust
use prodigy::cook::execution::mapreduce::mock_environment::*;

#[tokio::test]
async fn test_agent_with_custom_config() {
    // Build mock MapEnv
    let env = MockMapEnvBuilder::new()
        .with_max_parallel(8)
        .with_job_id("test-job-123")
        .with_config("timeout", json!(30))
        .with_debug()
        .build();

    // Test Reader pattern effects
    let max = get_max_parallel().run_async(&env).await.unwrap();
    assert_eq!(max, 8);

    let job_id = get_job_id().run_async(&env).await.unwrap();
    assert_eq!(job_id, "test-job-123");
}

#[tokio::test]
async fn test_phase_variables() {
    // Build mock PhaseEnv
    let env = MockPhaseEnvBuilder::new()
        .with_variable("count", json!(42))
        .with_variable("name", json!("test"))
        .build();

    let count = get_variable("count").run_async(&env).await.unwrap();
    assert_eq!(count, Some(json!(42)));
}

// Convenience functions for simple tests
let env = mock_map_env();                    // Default mock
let env = mock_map_env_debug();              // With debug enabled
let env = mock_map_env_with_parallel(10);    // Custom parallelism
let env = mock_phase_env();                  // Default phase env
```

### Architecture

```
src/cook/execution/mapreduce/
├── environment_helpers.rs    # Reader pattern helpers
│   ├── get_* functions       # Environment accessors (Effect::asks)
│   └── with_* functions      # Local override utilities (Effect::local)
└── mock_environment.rs       # Mock builders for testing
    ├── MockMapEnvBuilder     # Fluent API for MapEnv
    └── MockPhaseEnvBuilder   # Fluent API for PhaseEnv
```

**Key benefits:**
- **Type safety**: Compile-time environment access verification
- **Testability**: Mock environments for unit testing
- **Composability**: Chain with other Effect operations
- **Scoped overrides**: Local changes don't leak to parent scope
- **Clean API**: No boilerplate `Effect::asks` calls

## Environment Variables (Spec 120)

Define variables at workflow root:

```yaml
env:
  PROJECT_NAME: "prodigy"
  API_KEY:
    secret: true
    value: "sk-abc123"
  DATABASE_URL:
    default: "postgres://localhost/dev"
    prod: "postgres://prod-server/db"
```

**Usage:** `$VAR` or `${VAR}` in all phases (setup, map, reduce, merge)
**Profiles:** `prodigy run workflow.yml --profile prod`
**Secrets:** Masked in logs, errors, events, checkpoints

## Storage Architecture

### Global Storage (Default)
```
~/.prodigy/
├── events/{repo}/{job_id}/         # Event logs
├── dlq/{repo}/{job_id}/            # Failed items
├── state/{repo}/mapreduce/jobs/    # Checkpoints
├── sessions/                       # Unified sessions
├── resume_locks/                   # Resume locks
└── worktrees/{repo}/               # Git worktrees
```

### Session Management
Sessions stored as `UnifiedSession` in `~/.prodigy/sessions/{id}.json`:
- Status: Running|Paused|Completed|Failed|Cancelled
- Metadata: Execution timing, progress, step tracking
- Checkpoints: Full state snapshots for resume
- Workflow/MapReduce data: Variables, results, worktree info

**Session-Job mapping:** Bidirectional mapping enables resume with either ID.

## Git Integration

### Branch Tracking (Spec 110)
- Worktrees capture original branch at creation
- Merge targets original branch (not hardcoded main/master)
- Fallback to default branch if original deleted
- Branch shown in merge confirmation prompt

### Worktree Management
- Located in `~/.prodigy/worktrees/{project}/`
- Automatic branch creation and cleanup
- Commit tracking with full audit trail

## Workflow Execution

### Command Types
- `claude:` - Execute Claude commands
- `shell:` - Run shell commands
- `goal_seek:` - Goal-seeking with validation
- `foreach:` - Iterate with nested commands

### Variable Interpolation
- `${item.field}` - Work item fields
- `${shell.output}` - Command output
- `${map.results}` - Map phase results
- `$ARG` - CLI arguments

### Claude Streaming Control
- **Default (verbosity 0):** Clean output, no streaming
- **Verbose (`-v`):** Real-time JSON streaming
- **Override:** `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true`

### Environment Variables
- `PRODIGY_AUTOMATION=true` - Signals automated execution
- `PRODIGY_CLAUDE_STREAMING=false` - Disable JSON streaming

## Best Practices

1. **Session Hygiene**: Clean completed worktrees: `prodigy worktree clean`
2. **Error Recovery**: Check DLQ after MapReduce: `prodigy dlq show <job_id>`
3. **Workflow Design**: Keep simple and focused, include test steps
4. **Monitoring**: Use appropriate verbosity (`-v` for Claude output, `-vv`/`-vvv` for internals)
5. **Documentation**: Book workflow includes automatic drift/gap detection

## Common Commands

Use `prodigy --help` for full CLI reference. Key commands:
- `prodigy run <workflow.yml>` - Execute workflow
- `prodigy resume <id>` - Resume interrupted workflow
- `prodigy dlq retry <job_id>` - Retry failed items
- `prodigy worktree clean` - Clean completed worktrees
- `prodigy sessions list` - List all sessions
- `prodigy logs --latest` - View recent Claude logs

## Troubleshooting

### MapReduce Issues
```bash
# Check failed items
prodigy dlq show <job_id>

# Get Claude log from failed agent
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Resume from checkpoint
prodigy resume <job_id>

# Retry failed items
prodigy dlq retry <job_id>
```

### Worktree Issues
```bash
# List worktrees
prodigy worktree ls

# Clean orphaned worktrees
prodigy worktree clean-orphaned <job_id>

# Force clean stuck worktrees
prodigy worktree clean -f
```

### Debug with Verbosity
- `-v` - Shows Claude streaming output
- `-vv` - Adds debug logs
- `-vvv` - Adds trace-level logs

### View Claude Logs
```bash
# Latest log
prodigy logs --latest

# Follow live execution
prodigy logs --latest --tail

# Analyze completed log
cat ~/.claude/projects/.../uuid.jsonl | jq -c 'select(.type == "assistant")'
```

## Limitations

- No automatic context generation
- Iterations run independently (state via checkpoints)
- Limited to Claude commands in `.claude/commands/`
- Resume requires workflow files present
