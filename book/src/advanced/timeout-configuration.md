## Timeout Configuration

Set execution timeouts to prevent workflows from hanging indefinitely. Prodigy supports two distinct timeout mechanisms: command-level timeouts for standard workflows and MapReduce-specific timeouts with advanced configuration options.

### Command-Level Timeouts

Command-level timeouts apply to individual commands in standard workflows. These accept numeric values only (in seconds).

**Source**: `src/config/command.rs:384` - `pub timeout: Option<u64>`

```yaml
commands:
  # Shell command with 10 minute timeout
  - shell: "cargo bench"
    timeout: 600

  # Claude command with 30 minute timeout
  - claude: "/analyze-codebase"
    timeout: 1800

  # No timeout specified = no limit
  - shell: "cargo build"
```

**Real-world examples from workflows:**

From `workflows/complex-build-pipeline.yml:12`:
```yaml
- shell: "cargo bench"
  timeout: 600  # 10 minutes
  capture_output: "benchmark_results"
```

From `workflows/documentation-drift.yml:15`:
```yaml
- shell: "cargo test --doc"
  timeout: 300  # 5 minutes
  on_failure:
    claude: "/prodigy-debug-test-failure --output ${shell.output}"
```

**Important**: Command-level timeouts only accept numeric values. For environment variable support, use MapReduce timeouts (see below).

### MapReduce Timeouts

MapReduce workflows support more sophisticated timeout configuration with environment variable support and advanced policies.

#### Setup Phase Timeout

Control how long the setup phase can run before timing out.

**Source**: `src/config/mapreduce.rs:148` - Uses `deserialize_optional_u64_or_string`

```yaml
mode: mapreduce

setup:
  timeout: 300  # 5 minutes for setup
  commands:
    - shell: "generate-work-items.sh"
```

**With environment variables:**
```yaml
setup:
  timeout: $SETUP_TIMEOUT  # References environment variable
  commands:
    - shell: "cargo build"
```

#### Map Phase Agent Timeout

Set a global timeout for all map agents or configure per-agent policies.

**Source**: `src/config/mapreduce.rs:269` - `pub agent_timeout_secs: Option<String>`

**Simple agent timeout:**
```yaml
map:
  agent_timeout_secs: 600  # 10 minutes per agent
  agent_template:
    - claude: "/process '${item}'"
```

**With environment variable:**
```yaml
map:
  agent_timeout_secs: $AGENT_TIMEOUT  # Configurable via environment
  agent_template:
    - claude: "/process '${item}'"
```

#### Advanced Timeout Configuration

For fine-grained control, use `timeout_config` to specify policies, per-command overrides, and timeout actions.

**Source**: `src/cook/execution/mapreduce/timeout.rs:38-63` - `TimeoutConfig` struct

```yaml
map:
  timeout_config:
    agent_timeout_secs: 600          # Global 10 minute timeout
    timeout_policy: hybrid           # Apply per-agent with overrides
    cleanup_grace_period_secs: 30    # 30s to clean up after timeout
    timeout_action: dlq              # Send timed-out items to DLQ
    enable_monitoring: true          # Track timeout metrics

    # Per-command timeout overrides
    command_timeouts:
      claude: 300                    # Claude commands: 5 minutes
      shell: 60                      # Shell commands: 1 minute
      claude_0: 600                  # First Claude command: 10 minutes

  agent_template:
    - claude: "/analyze '${item}'"   # Uses 300s from command_timeouts
    - shell: "test ${item.path}"     # Uses 60s from command_timeouts
```

**Real example from tests** (`tests/timeout_integration_test.rs:215-233`):
```yaml
agent_timeout_secs: 600
timeout_config:
  timeout_policy: hybrid
  cleanup_grace_period_secs: 30
  timeout_action: dlq
  enable_monitoring: true
  command_timeouts:
    claude: 300
    shell: 60
    claude_0: 600
```

#### Timeout Policies

**Source**: `src/cook/execution/mapreduce/timeout.rs:79-88` - `TimeoutPolicy` enum

- **`per_agent`** (default): Timeout applies to entire agent execution
  - Agent must complete all commands within timeout
  - Best for workflows where total time matters

- **`per_command`**: Timeout applies to each command individually
  - Each command gets full timeout duration
  - Best for workflows with highly variable command durations

- **`hybrid`**: Per-agent timeout with command-specific overrides
  - Commands use `command_timeouts` if specified, otherwise agent timeout
  - Most flexible option

**Example: Per-command policy**
```yaml
timeout_config:
  agent_timeout_secs: 100
  timeout_policy: per_command  # Each command gets 100 seconds
```

#### Timeout Actions

**Source**: `src/cook/execution/mapreduce/timeout.rs:91-102` - `TimeoutAction` enum

- **`dlq`** (default): Send item to Dead Letter Queue for retry
- **`skip`**: Skip the item and continue with other items
- **`fail`**: Fail the entire MapReduce job
- **`graceful_terminate`**: Attempt graceful shutdown before force kill

```yaml
timeout_config:
  timeout_action: skip  # Skip timed-out items instead of retrying
```

#### Default Values

**Source**: `src/cook/execution/mapreduce/timeout.rs` - Default implementations

| Configuration | Default Value | Description |
|---------------|---------------|-------------|
| `agent_timeout_secs` | 600 (10 minutes) | Global agent timeout |
| `cleanup_grace_period_secs` | 30 seconds | Time for cleanup after timeout |
| `enable_monitoring` | true | Track timeout metrics |
| `timeout_policy` | `per_agent` | Apply timeout to entire agent |
| `timeout_action` | `dlq` | Send timed-out items to DLQ |

### Best Practices

**Set Appropriate Timeouts:**
- Set timeouts high enough to complete under normal conditions
- Consider worst-case scenarios (slow CI, cold caches, network latency)
- Use shorter timeouts for quick operations to fail fast
- Test timeout values in your environment before production use

**MapReduce Timeout Strategy:**
- Start with default `per_agent` policy and adjust based on metrics
- Use `hybrid` policy when some commands need more time than others
- Set `cleanup_grace_period_secs` to allow proper resource cleanup
- Choose `timeout_action` based on retry strategy:
  - `dlq` for retriable operations
  - `skip` for optional/best-effort work items
  - `fail` for critical operations where partial completion is unacceptable

**Environment Variables for Flexibility:**
- Use environment variables in MapReduce workflows to parameterize timeouts
- Define different timeout values for dev/staging/production
- Document expected timeout ranges in workflow comments
- Example: `AGENT_TIMEOUT=300 prodigy run workflow.yml` for faster iteration

**Monitoring and Adjustment:**
- Enable `enable_monitoring: true` to track timeout patterns
- Review timeout events in `.prodigy/events/` to identify patterns
- Adjust timeouts based on actual execution times
- Consider using `timeout_config.command_timeouts` for frequently timing-out commands

### Troubleshooting

**Commands timing out unexpectedly:**
1. Check event logs in `.prodigy/events/{repo_name}/{job_id}/` for timeout events
2. Verify timeout value is appropriate for the operation
3. For MapReduce workflows, check if `timeout_policy` is appropriate
4. Review `cleanup_grace_period_secs` if cleanup is slow

**Items repeatedly sent to DLQ due to timeout:**
1. Increase `agent_timeout_secs` or specific command timeout
2. Consider changing `timeout_action` to `skip` if items aren't critical
3. Use `hybrid` policy with higher timeout for slow commands
4. Review work item complexity - may need to split items

**Timeout not being enforced:**
1. Verify timeout is set (defaults to no timeout for command-level)
2. Check that numeric timeout value is positive
3. For MapReduce, ensure `timeout_config` or `agent_timeout_secs` is specified
4. Review logs to confirm timeout monitoring is enabled

### See Also

- [MapReduce Documentation](../mapreduce/index.md) - Overview of MapReduce workflows
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) - Handling timed-out items
- [Environment Variables](../mapreduce/environment-variables-in-configuration.md) - Parameterizing MapReduce workflows
- [Performance Tuning](../mapreduce/performance-tuning.md) - Optimizing workflow execution times
