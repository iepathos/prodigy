# Usage in Workflow Phases

This page covers how to use environment variables across all phases of Prodigy workflows.

## Usage in Workflow Phases

Environment variables are available in all workflow phases:

### Standard Workflows

```yaml
# Source: workflows/environment-example.yml:42-52
commands:
  - name: "Show environment"
    shell: "echo NODE_ENV=$NODE_ENV API_URL=$API_URL"

  - name: "Build frontend"
    shell: "echo 'Building with NODE_ENV='$NODE_ENV"
    env:
      BUILD_TARGET: production
      OPTIMIZE: "true"
    working_dir: ./frontend
```

### MapReduce Setup Phase

```yaml
# Source: workflows/mapreduce-env-example.yml:42-49
setup:
  - shell: "echo Starting $PROJECT_NAME workflow"
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "echo Created output directory: ${OUTPUT_DIR}"
  - shell: "echo Debug mode: $DEBUG_MODE"
```

### MapReduce Map Phase

Environment variables are available in agent templates:

```yaml
# Source: workflows/mapreduce-env-example.yml:56-68
map:
  agent_template:
    # In Claude commands
    - claude: "/process-item '${item.name}' --project $PROJECT_NAME"

    # In shell commands
    - shell: "echo Processing ${item.name} for $PROJECT_NAME"
    - shell: "echo Output: $OUTPUT_DIR"

    # In failure handlers
    - shell: "timeout ${TIMEOUT_SECONDS}s ./process.sh"
      on_failure:
        - claude: "/fix-issue --max-retries $MAX_RETRIES"
```

!!! note "MapReduce Agent Isolation"
    Each MapReduce agent runs in an isolated git worktree with its own execution context. Environment variables defined in the workflow are automatically inherited by all agents. Secret masking is maintained across agent boundaries to ensure credentials remain protected.

### MapReduce Reduce Phase

```yaml
# Source: workflows/mapreduce-env-example.yml:72-79
reduce:
  - shell: "echo Aggregating results for $PROJECT_NAME"
  - claude: "/summarize ${map.results} --format $REPORT_FORMAT"
  - shell: "cp summary.$REPORT_FORMAT $OUTPUT_DIR/${PROJECT_NAME}-summary.$REPORT_FORMAT"
  - shell: "echo Processed ${map.successful}/${map.total} items"
```

### Merge Phase

```yaml
# Source: workflows/mapreduce-env-example.yml:82-93
merge:
  commands:
    - shell: "echo Merging changes for $PROJECT_NAME"
    - claude: "/validate-merge --branch ${merge.source_branch} --project $PROJECT_NAME"
    - shell: "echo Merge completed for ${PROJECT_NAME}"
```

## Per-Step Environment

Override or add variables for specific commands:

```yaml
# Source: workflows/environment-example.yml:54-60
commands:
  - name: "Run tests"
    shell: "pytest tests/"
    env:
      PYTHONPATH: "./src:./tests"
      TEST_ENV: "true"
    working_dir: ./backend
    temporary: true  # Environment restored after this step
```

**Options:**

- `temporary: true` - Restore environment after step completes
- `clear_env: true` - Clear all inherited variables, use only step-specific ones
