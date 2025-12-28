## Core Options

Essential configuration options for all command types.

### timeout

Sets a maximum execution time for the command (in seconds). If the command exceeds this duration, it will be terminated.

**Type**: `Option<u64>` (optional, no default timeout)

**Source**: `src/config/command.rs:383`

```yaml
commands:
  # 5 minute timeout for test suite
  - shell: "npm test"
    timeout: 300

  # 10 minute timeout for Claude implementation
  - claude: "/implement feature"
    timeout: 600

  # No timeout (runs until completion)
  - shell: "cargo build --release"
```

**Real-world examples**:
- `workflows/debtmap-reduce.yml:6` - 15 minute timeout for coverage generation
- `workflows/complex-build-pipeline.yml:23` - 10 minute timeout for benchmarks
- `workflows/documentation-drift.yml:48` - 5 minute timeout for doc tests

### id

Assigns an identifier to the command for referencing its outputs in subsequent commands via workflow variables.

**Type**: `Option<String>` (optional)

**Source**: `src/config/command.rs:351-352`

```yaml
commands:
  - shell: "git rev-parse --short HEAD"
    id: "get_commit"
    capture_output: "commit_hash"

  # Use the captured output
  - shell: "echo 'Building commit ${commit_hash}'"
```

### commit_required

Specifies whether the command is expected to create git commits. Prodigy tracks commits for workflow provenance and rollback.

**Type**: `bool` (default: `false`)

**Source**: `src/config/command.rs:354-356`

```yaml
commands:
  # Claude commands that modify code should commit
  - claude: "/prodigy-coverage"
    commit_required: true

  # Test commands typically don't commit
  - shell: "cargo test"
    commit_required: false

  # Linting fixes may commit changes
  - claude: "/prodigy-lint"
    commit_required: true
```

**Real-world examples**:
- `workflows/coverage.yml:5,11` - Coverage and implementation commands
- `workflows/documentation-drift.yml:19,23,27` - Documentation update commands
- `workflows/implement-with-tests.yml:27,31` - Test vs implementation distinction

#### MapReduce-Specific Behavior (Spec 163)

In MapReduce workflows, `commit_required: true` is **strictly enforced** with validation to prevent silent data loss. When a command in `agent_template` is marked with `commit_required: true`, Prodigy validates that at least one commit was created in the agent's isolated worktree.

**Validation Behavior**:
- Before each `commit_required` command executes, the current git HEAD is captured
- After the command completes, the git HEAD is checked again
- If no new commits are detected (HEAD unchanged), the agent fails with `CommitValidationFailed` error
- Failed agents are added to the Dead Letter Queue (DLQ) with `manual_review_required: true`

**MapReduce Example**:
```yaml
name: process-items
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"

  agent_template:
    # This command MUST create a commit
    - shell: |
        echo "${item.data}" > "${item.file}"
        git add "${item.file}"
        git commit -m "Process item ${item.id}"
      commit_required: true
```

**Error Message Format**:
```
Agent execution failed: Commit required but no commit was created

Worktree: /Users/user/.prodigy/worktrees/repo/agent-1
Expected behavior: Command should create at least one git commit
Command: shell: process-item.sh
```

**Merge Behavior**:
- Agents with commits: Merged to parent worktree via queue
- Agents without commits: Worktree cleaned up, merge skipped (logged at INFO level)
- Agents with validation errors: Worktree cleaned up, added to DLQ

**Event Tracking**:
- `AgentCompleted` events include `commits: Vec<String>` field with commit SHAs
- `AgentFailed` events include `failure_reason: CommitValidationFailed` for validation errors
- Both events include `json_log_location` for debugging

**Troubleshooting**:
See [MapReduce Troubleshooting Guide](../mapreduce/troubleshooting.md#commit-validation-failures) for common issues and solutions.
