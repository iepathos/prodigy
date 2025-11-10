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

**Parameterizing max_parallel:**

The `max_parallel` field accepts both numeric values and environment variable references:

```yaml
env:
  MAX_WORKERS: "5"   # Can be overridden per environment

map:
  max_parallel: ${MAX_WORKERS}   # Uses env var
  # OR
  max_parallel: 10               # Static value
```

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
  REPORT_FORMAT: json
  OUTPUT_PATH: results/summary.json
  MIN_SUCCESS_RATE: "80"

reduce:
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"
  - write_file:
      path: ${OUTPUT_PATH}
      content: ${map.results}
      format: ${REPORT_FORMAT}
  - shell: |
      SUCCESS_RATE=$((${map.successful} * 100 / ${map.total}))
      if [ $SUCCESS_RATE -lt $MIN_SUCCESS_RATE ]; then
        echo "Warning: Success rate below threshold"
        exit 1
      fi
```

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
      path: ${OUTPUT_DIR}/summary.json
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

**Dynamic input generation:**

```yaml
env:
  DATA_SOURCE: https://api.example.com/v1/items

secrets:
  API_KEY: "${env:SECRET_API_KEY}"

setup:
  - shell: "curl -H 'Authorization: Bearer ${API_KEY}' $DATA_SOURCE > items.json"

map:
  input: items.json
  agent_template:
    - shell: "process --api-key ${API_KEY} ${item.id}"
```

See also:
- [Environment Profiles](environment-profiles.md) for multi-environment configurations
- [Secrets Management](secrets-management.md) for handling sensitive variables
- [Environment Precedence](environment-precedence.md) for understanding resolution order
