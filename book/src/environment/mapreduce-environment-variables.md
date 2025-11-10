## MapReduce Environment Variables

In MapReduce workflows, environment variables provide powerful parameterization across all phases (setup, map, reduce, and merge). This enables workflows to be reusable across different projects and configurations.

### Overview

Environment variables in MapReduce workflows are available in all execution phases:
- **Setup phase**: Initialize environment, generate configuration
- **Map phase**: Parameterize agent templates, configure timeouts and parallelism
- **Reduce phase**: Aggregate results, format output
- **Merge phase**: Control merge behavior, validation

### Setup Phase Environment Variables

Environment variables are fully available in setup phase commands:

```yaml
env:
  PROJECT_NAME: prodigy
  DATA_SOURCE: https://api.example.com/items
  TIMEOUT: "30"

setup:
  - shell: "curl $DATA_SOURCE > items.json"
  - shell: "echo 'Processing $PROJECT_NAME workflow'"
  - shell: "mkdir -p output/$PROJECT_NAME"
```

**Use cases in setup:**
- Configure data sources and API endpoints
- Set project-specific paths
- Initialize environment-specific settings

### Map Phase Environment Variables

Environment variables can be used throughout the map phase configuration:

```yaml
env:
  MAX_WORKERS: "10"
  AGENT_TIMEOUT: "600"
  PROJECT_DIR: /path/to/project

map:
  input: items.json
  max_parallel: ${MAX_WORKERS}     # Parameterize parallelism
  agent_timeout_secs: ${AGENT_TIMEOUT}

  agent_template:
    - claude: "/process-item '${item.name}' --project $PROJECT_NAME"
    - shell: "test -f $PROJECT_DIR/${item.file}"
    - shell: "timeout ${AGENT_TIMEOUT} ./analyze.sh ${item.path}"
```

**Parameterizing max_parallel and agent_timeout_secs:**

Both `max_parallel` and `agent_timeout_secs` accept numeric values or environment variable references. These are resolved at **configuration parse time** (when the workflow is loaded), not at execution time.

```yaml
env:
  MAX_WORKERS: "5"   # Can be overridden per environment
  AGENT_TIMEOUT: "600"

map:
  max_parallel: ${MAX_WORKERS}           # Resolved to 5 at parse time
  agent_timeout_secs: ${AGENT_TIMEOUT}   # Resolved to 600 at parse time
  # OR
  max_parallel: 10                       # Static value
  agent_timeout_secs: 300                # Static value
```

**Environment Variable Resolution Timing:**

- **Parse time** (when workflow is loaded): `max_parallel` and `agent_timeout_secs` are resolved using `resolve_env_or_parse` method
- **Execution time** (when commands run): Command interpolation happens (e.g., `${item.name}`, `$PROJECT_NAME`)

This distinction matters because parse-time resolution fails fast if environment variables are undefined, while execution-time interpolation happens dynamically.

Source: src/config/mapreduce.rs:527-540 (to_map_phase method with resolve_env_or_parse)

**Integration with MapReduce-specific variables:**

Environment variables work seamlessly with MapReduce variables like `${item}` and `${map.results}`:

```yaml
env:
  OUTPUT_DIR: /tmp/results
  CONFIG_FILE: config.json

map:
  agent_template:
    - shell: "process --config $CONFIG_FILE --input ${item.path} --output $OUTPUT_DIR/${item.id}.json"
```

### Reduce Phase Environment Variables

Use environment variables in reduce commands to parameterize aggregation:

```yaml
env:
  OUTPUT_PATH: results/summary.json
  MIN_SUCCESS_RATE: "80"

reduce:
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"
  - write_file:
      path: "${OUTPUT_PATH}"
      content: "${map.results}"
      format: json
  - shell: |
      SUCCESS_RATE=$((${map.successful} * 100 / ${map.total}))
      if [ $SUCCESS_RATE -lt $MIN_SUCCESS_RATE ]; then
        echo "Warning: Success rate below threshold"
        exit 1
      fi
```

**Important: write_file format field**

The `format` field in `write_file` accepts enum literals only (`text`, `json`, or `yaml`), not environment variable interpolation. Only the `path` and `content` fields support variable interpolation.

Source: src/config/command.rs:303-314 (WriteFileFormat enum)

### Merge Phase Environment Variables

Environment variables are available in merge workflows alongside merge-specific variables:

```yaml
env:
  CI_MODE: "true"
  TEST_TIMEOUT: "300"

merge:
  commands:
    - shell: "git fetch origin"
    - shell: "timeout $TEST_TIMEOUT cargo test"
    - shell: |
        if [ "$CI_MODE" = "true" ]; then
          git merge --no-edit ${merge.source_branch}
        else
          claude "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
        fi
```

### Complete Example: Parameterized MapReduce Workflow

```yaml
name: parameterized-processing
mode: mapreduce

env:
  # Project configuration
  PROJECT_NAME: my-project
  VERSION: "1.0.0"

  # Execution parameters
  MAX_WORKERS: "10"
  AGENT_TIMEOUT: "600"

  # Paths
  INPUT_FILE: items.json
  OUTPUT_DIR: results
  CONFIG_PATH: config/settings.json

  # Thresholds
  MIN_COVERAGE: "80"

setup:
  - shell: "echo 'Starting $PROJECT_NAME v$VERSION'"
  - shell: "mkdir -p $OUTPUT_DIR"
  - shell: "generate-items.sh > $INPUT_FILE"

map:
  input: ${INPUT_FILE}
  max_parallel: ${MAX_WORKERS}
  agent_timeout_secs: ${AGENT_TIMEOUT}

  agent_template:
    - claude: "/process '${item.name}' --project $PROJECT_NAME --config $CONFIG_PATH"
    - shell: "test -f $OUTPUT_DIR/${item.id}.result"

reduce:
  - shell: "echo 'Project: $PROJECT_NAME, Version: $VERSION'"
  - shell: "echo 'Results: ${map.successful}/${map.total} succeeded'"
  - write_file:
      path: "${OUTPUT_DIR}/summary.json"
      content: |
        {
          "project": "$PROJECT_NAME",
          "version": "$VERSION",
          "total": ${map.total},
          "successful": ${map.successful},
          "failed": ${map.failed}
        }
      format: json

merge:
  commands:
    - shell: "cargo test --timeout $AGENT_TIMEOUT"
    - claude: "/validate-merge --project $PROJECT_NAME"
```

### Best Practices for MapReduce Environment Variables

1. **Parameterize resource limits**: Use env vars for `max_parallel`, `agent_timeout_secs`, and `timeout`
2. **Project-agnostic workflows**: Use `PROJECT_NAME` env var to make workflows reusable
3. **Environment-specific values**: Combine with profiles for dev/staging/prod configurations
4. **Path parameterization**: Use env vars for input files, output directories, and config paths
5. **Threshold configuration**: Parameterize quality thresholds, success rates, and limits
6. **Combine with MapReduce variables**: Use both `${ENV_VAR}` and `${item.field}` together
7. **Understand precedence chain**: Environment variables are resolved in this order:
   - Step-level env (per-command overrides)
   - Workflow profile env (when `--profile` is active)
   - Workflow env (global env block)
   - System environment variables

**Environment Variable Precedence:**

When the same variable is defined in multiple places, Prodigy uses this precedence chain (highest to lowest priority):

1. **Step env** - Environment variables defined at the command level
2. **Workflow profile** - Profile-specific values (activated with `--profile`)
3. **Workflow env** - Global env block at workflow level
4. **System env** - Environment variables from the parent process

Source: .prodigy/book-analysis/features.json:596 (configuration.precedence_chain)

### Common Patterns

**Multi-environment deployment:**

```yaml
env:
  MAX_WORKERS:
    default: "5"
    dev: "2"
    prod: "20"
  API_URL:
    default: http://localhost:3000
    prod: https://api.production.com

map:
  max_parallel: ${MAX_WORKERS}
  agent_template:
    - shell: "curl $API_URL/process -d '${item}'"
```

**Resource-constrained execution:**

```yaml
env:
  CPU_LIMIT: "4"
  MEMORY_LIMIT: "8G"
  TIMEOUT: "300"

map:
  max_parallel: ${CPU_LIMIT}
  agent_timeout_secs: ${TIMEOUT}

  agent_template:
    - shell: "docker run --cpus=${CPU_LIMIT} --memory=${MEMORY_LIMIT} processor ${item.path}"
```

**Dynamic input generation with secrets:**

```yaml
env:
  DATA_SOURCE: https://api.example.com/v1/items
  API_KEY:
    secret: true
    value: "sk-actual-api-key-here"

setup:
  - shell: "curl -H 'Authorization: Bearer ${API_KEY}' $DATA_SOURCE > items.json"

map:
  input: items.json
  agent_template:
    - shell: "process --api-key ${API_KEY} ${item.id}"
```

**Secret Configuration:**

Secrets are defined within the `env` block using the `secret: true` flag. This masks the value in logs and output. There is no separate `secrets:` block.

```yaml
env:
  REGULAR_VAR: "visible-value"
  SECRET_API_KEY:
    secret: true
    value: "sk-secret-value"   # Masked as *** in logs
```

Source: src/config/workflow.rs (WorkflowConfig.env), features.json:environment.secrets

See also:
- [Environment Profiles](environment-profiles.md) for multi-environment configurations
- [Secrets Management](secrets-management.md) for handling sensitive variables
- [Environment Precedence](environment-precedence.md) for understanding resolution order
