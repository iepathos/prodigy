---
number: 49
title: MapReduce Parallel Execution for Workflows
category: parallel
priority: critical
status: draft
dependencies: []
created: 2025-08-18
---

# Specification 49: MapReduce Parallel Execution for Workflows

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

MMM currently executes workflows sequentially or with limited parallelism through worktrees. As codebases grow and AI-driven improvements become more sophisticated, there's a critical need for true MapReduce-style parallel execution that can:

1. Process hundreds of independent tasks simultaneously (e.g., fixing individual technical debt items)
2. Reduce total execution time from hours to minutes
3. Minimize LLM token usage through focused context per agent
4. Provide fault isolation where one agent's failure doesn't affect others
5. Enable cost-effective scaling of AI-driven code improvements

The integration with tools like Debtmap demonstrates the need: analyzing a codebase might identify 50-500 technical debt items that could all be fixed independently, but current sequential processing makes this prohibitively slow.

## Objective

Implement a MapReduce execution mode for MMM workflows that enables parallel processing of independent tasks with automatic result aggregation, supporting both local and distributed execution while maintaining git-native safety and rollback capabilities.

## Requirements

### Functional Requirements

1. **MapReduce Workflow Mode**
   - New `mode: mapreduce` option in workflow YAML
   - Support for `map` and `reduce` phases
   - Dynamic work item generation from external tools (e.g., Debtmap JSON output)
   - Template-based agent command generation with variable interpolation

2. **Parallel Execution Engine**
   - Spawn up to N parallel agents (configurable via `--parallel` flag)
   - Each agent runs in isolated git worktree
   - Automatic batching when items exceed max parallelism
   - Progress tracking and real-time status updates

3. **Input Data Processing**
   - Load work items from JSON files (`input: file.json`)
   - Support JSON path expressions for item extraction
   - Variable interpolation in commands (e.g., `${item.description}`, `${item.location.file}`)
   - Filter and sort capabilities for work item selection

4. **Agent Orchestration**
   - Independent agent lifecycle management
   - Per-agent timeout configuration
   - Retry logic for failed agents
   - Conditional execution based on previous step results

5. **Result Aggregation**
   - Automatic collection of agent outputs
   - Success/failure tracking per work item
   - Reduce phase for result synthesis
   - Generation of unified reports and metrics

6. **Failure Handling**
   - `on_failure` blocks for recovery actions
   - `fail_workflow` flag to control failure propagation
   - Maximum retry attempts per agent
   - Graceful degradation when agents fail

### Non-Functional Requirements

1. **Performance**
   - Support 100+ parallel agents on modern hardware
   - Minimal overhead for agent spawning (<100ms per agent)
   - Efficient resource usage (CPU, memory, disk I/O)
   - Token usage reduction of 50-90% vs sequential execution

2. **Reliability**
   - Fault isolation between agents
   - Atomic commits per successful agent
   - Clean rollback on catastrophic failure
   - Resume capability for interrupted executions

3. **Observability**
   - Real-time progress visualization
   - Per-agent logs and debugging information
   - Aggregate metrics and statistics
   - Performance profiling per phase

4. **Scalability**
   - Local execution for 1-50 agents
   - Distributed execution support for 100+ agents (future)
   - Adaptive resource allocation
   - Queue-based work distribution

## Acceptance Criteria

- [ ] Can execute a workflow with `mode: mapreduce` that processes items in parallel
- [ ] Successfully processes 50 Debtmap items with 10 parallel agents
- [ ] Each agent runs in an isolated git worktree with independent commits
- [ ] Failed agents don't block other agents from completing
- [ ] Reduce phase aggregates results from all successful map agents
- [ ] Progress bar shows real-time status of all parallel agents
- [ ] Token usage is reduced by >70% compared to sequential execution
- [ ] Can resume execution after interruption
- [ ] Generates comprehensive execution report with metrics
- [ ] All existing sequential workflows continue to work unchanged
- [ ] Documentation includes complete MapReduce workflow examples
- [ ] Integration tests cover parallel execution scenarios

## Technical Details

### Implementation Approach

1. **Workflow Parser Extensions**
   - Extend `workflow.rs` to recognize `mode: mapreduce`
   - Add `map` and `reduce` phase parsing
   - Implement variable interpolation engine
   - Support JSON input file loading

2. **Parallel Executor**
   - New `MapReduceExecutor` in `cook/execution/`
   - Thread pool for agent management
   - Work queue with batching logic
   - Result collector with aggregation

3. **Agent Isolation**
   - Leverage existing worktree manager
   - Per-agent branch creation
   - Independent session state tracking
   - Parallel-safe git operations

4. **Progress Tracking**
   - Multi-line progress display
   - Per-agent status indicators
   - Aggregate statistics updating
   - ETA calculation based on completion rate

### Architecture Changes

```rust
// New module structure
src/cook/execution/
  ├── mod.rs
  ├── runner.rs (existing)
  ├── claude.rs (existing)
  └── mapreduce.rs (new)

// Core types
pub struct MapReduceConfig {
    pub input: PathBuf,
    pub max_parallel: usize,
    pub timeout_per_agent: Duration,
    pub retry_on_failure: u32,
}

pub struct MapPhase {
    pub agent_template: WorkflowCommands,
    pub work_items: Vec<serde_json::Value>,
}

pub struct ReducePhase {
    pub commands: Vec<WorkflowCommand>,
}

pub struct AgentResult {
    pub item_id: String,
    pub status: AgentStatus,
    pub output: Option<String>,
    pub commits: Vec<String>,
    pub duration: Duration,
}
```

### Data Flow

```
1. Load input JSON → Parse work items
2. Create thread pool with max_parallel workers
3. For each work item:
   a. Spawn agent in new worktree
   b. Interpolate variables in commands
   c. Execute agent commands
   d. Collect results
4. Wait for all agents or timeout
5. Execute reduce phase with aggregated results
6. Generate final report
```

### APIs and Interfaces

```yaml
# Workflow YAML Interface
name: parallel-debt-elimination
mode: mapreduce

# Pre-map phase (optional)
setup:
  - shell: "debtmap analyze . --output items.json"

map:
  input: items.json
  json_path: "$.debt_items[*]"
  
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
  
  max_parallel: 10
  timeout_per_agent: 600s
  retry_on_failure: 2

reduce:
  commands:
    - claude: "/summarize-fixes ${map.results}"
    - shell: "git merge --no-ff agent-*"
```

```bash
# CLI Interface
mmm cook workflow.yml --mode mapreduce --parallel 20
mmm cook workflow.yml --map items.json --max-parallel 10
mmm cook workflow.yml --resume session-id  # Resume interrupted execution
```

## Dependencies

- **Prerequisites**: None (builds on existing MMM foundation)
- **Affected Components**:
  - `cook/orchestrator.rs` - Add MapReduce mode detection
  - `cook/workflow/executor.rs` - Integrate parallel executor
  - `worktree/manager.rs` - Ensure thread-safe operations
  - `session/state.rs` - Track parallel agent states
- **External Dependencies**:
  - `rayon` or `tokio` for parallel execution
  - `indicatif` for progress bars (already used)

## Testing Strategy

- **Unit Tests**:
  - Variable interpolation engine
  - Work item parsing and filtering
  - Agent result aggregation
  - Failure handling logic

- **Integration Tests**:
  - End-to-end MapReduce workflow execution
  - Parallel git operations safety
  - Resource limit enforcement
  - Interruption and resume

- **Performance Tests**:
  - Scaling from 1 to 100 parallel agents
  - Token usage comparison vs sequential
  - Memory and CPU usage under load
  - I/O bottleneck identification

- **User Acceptance**:
  - Real Debtmap integration test
  - Large codebase processing
  - Failure recovery scenarios
  - Progress tracking accuracy

## Documentation Requirements

- **Code Documentation**:
  - MapReduce executor implementation details
  - Thread safety considerations
  - Variable interpolation syntax

- **User Documentation**:
  - MapReduce workflow tutorial
  - Complete example with Debtmap
  - Performance tuning guide
  - Troubleshooting parallel execution

- **Architecture Updates**:
  - Add parallel execution flow diagram
  - Document agent isolation model
  - Update workflow execution pipeline

## Implementation Notes

### Phase 1: Core MapReduce Engine (Week 1)
- Basic map/reduce phase parsing
- Simple parallel execution with threads
- Variable interpolation
- Result collection

### Phase 2: Advanced Features (Week 2)
- JSON input loading
- on_failure handlers
- Retry logic
- Progress tracking

### Phase 3: Optimization (Week 3)
- Token usage optimization
- Resource management
- Performance tuning
- Distributed execution prep

### Key Considerations

1. **Git Safety**: Ensure all parallel git operations are atomic and don't conflict
2. **Resource Limits**: Prevent system overload with too many parallel agents
3. **Token Optimization**: Batch API calls where possible to reduce costs
4. **Error Isolation**: One agent's crash shouldn't kill the entire execution
5. **Debugging**: Provide clear logs for each agent's execution

## Migration and Compatibility

- **Breaking Changes**: None - new feature is additive
- **Migration Path**: Existing workflows continue to work unchanged
- **Compatibility**: 
  - Supports all existing workflow commands
  - Works with current worktree implementation
  - Compatible with existing session management
- **Rollback**: Can disable MapReduce mode and fall back to sequential

## Success Metrics

- Reduce 50-item technical debt processing from 2+ hours to <15 minutes
- 75%+ reduction in LLM token usage for parallel tasks
- 90%+ success rate for agent execution
- <5% performance overhead for orchestration
- Positive user feedback on execution speed and reliability

## Future Enhancements

1. **Distributed Execution**: Support for cloud-based agent pools
2. **Smart Scheduling**: ML-based work item prioritization
3. **Incremental Reduction**: Start reduce phase before all maps complete
4. **Caching**: Reuse results from previous runs
5. **Visual Dashboard**: Web UI for monitoring parallel execution