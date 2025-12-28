## MapReduce Environment Variables

In MapReduce workflows, environment variables provide powerful parameterization across all phases (setup, map, reduce, and merge). This enables workflows to be reusable across different projects and configurations.

### Overview

Environment variables in MapReduce workflows are available in all execution phases:

- **Setup phase**: Initialize environment, generate configuration
- **Map phase**: Parameterize agent templates, configure timeouts and parallelism
- **Reduce phase**: Aggregate results, format output
- **Merge phase**: Control merge behavior, validation

!!! info "Parse-Time vs Execution-Time Resolution"
    MapReduce configuration values like `max_parallel` and `agent_timeout_secs` are resolved at **parse time** (when the workflow loads), while command interpolation happens at **execution time** (when commands run). This distinction affects when errors appear and how dynamic values work.

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

#### Parameterizing max_parallel and agent_timeout_secs

Both `max_parallel` and `agent_timeout_secs` accept numeric values or environment variable references. These are resolved at **configuration parse time** (when the workflow is loaded), not at execution time.

=== "Environment Variables"
    ```yaml
    env:
      MAX_WORKERS: "5"   # Can be overridden per environment
      AGENT_TIMEOUT: "600"

    map:
      max_parallel: ${MAX_WORKERS}           # Resolved to 5 at parse time
      agent_timeout_secs: ${AGENT_TIMEOUT}   # Resolved to 600 at parse time
    ```

=== "Static Values"
    ```yaml
    map:
      max_parallel: 10                       # Static value
      agent_timeout_secs: 300                # Static value
    ```

!!! warning "Parse-Time Resolution Fails Fast"
    If an environment variable referenced by `max_parallel` or `agent_timeout_secs` is undefined, the workflow fails immediately at load time rather than during execution. This is intentional for early error detection.

**Source**: `src/config/mapreduce.rs:527-540` (`to_map_phase` method with `resolve_env_or_parse`)

#### Integration with MapReduce-specific variables

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

!!! note "write_file format field"
    The `format` field in `write_file` accepts enum literals only (`text`, `json`, or `yaml`), not environment variable interpolation. Only the `path` and `content` fields support variable interpolation.

    **Source**: `src/config/command.rs:303-314` (`WriteFileFormat` enum)

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

### Advanced Environment Features

MapReduce workflows support advanced environment features beyond simple static values.

#### Dynamic Environment Values

Environment values can be computed dynamically from command output using `DynamicEnv`:

```yaml
# Source: src/cook/environment/config.rs:62-70
env:
  # Static value
  PROJECT_NAME: my-project

  # Dynamic value computed from command output
  GIT_BRANCH:
    command: "git rev-parse --abbrev-ref HEAD"
    cache: true  # Cache result for repeated access

  BUILD_NUMBER:
    command: "cat build-number.txt"
    cache: false  # Re-evaluate each time

map:
  agent_template:
    - claude: "/deploy --branch $GIT_BRANCH --build $BUILD_NUMBER"
```

!!! tip "Dynamic Value Caching"
    Set `cache: true` for values that don't change during workflow execution (like git branch). Use `cache: false` for values that may change between phases or agents.

#### Conditional Environment Values

Environment values can be conditional based on expressions using `ConditionalEnv`:

```yaml
# Source: src/cook/environment/config.rs:72-81
env:
  # Conditional value based on expression
  LOG_LEVEL:
    condition: "branch == 'main'"
    when_true: "error"
    when_false: "debug"

  DEPLOY_TARGET:
    condition: "env.CI == 'true'"
    when_true: "production"
    when_false: "staging"

map:
  agent_template:
    - shell: "deploy --target $DEPLOY_TARGET --log-level $LOG_LEVEL"
```

#### Secrets in MapReduce Workflows

MapReduce workflows can use secrets that are masked in logs and output:

```yaml
# Source: src/cook/environment/config.rs:84-112
secrets:
  # Simple secret reference
  API_KEY: "${env:SECRET_API_KEY}"

  # Provider-based secrets
  DATABASE_URL:
    provider: env
    key: "DB_CONNECTION_STRING"

  SSH_DEPLOY_KEY:
    provider: file
    key: "~/.ssh/deploy_key"

map:
  agent_template:
    - shell: "curl -H 'Authorization: Bearer $API_KEY' https://api.example.com"
    - shell: "ssh -i $SSH_DEPLOY_KEY deploy@server '${item.command}'"

reduce:
  - shell: "psql $DATABASE_URL -c 'SELECT COUNT(*) FROM results'"
```

| Provider | Status | Description |
|----------|--------|-------------|
| `env` | Implemented | Reads from environment variables |
| `file` | Implemented | Reads from filesystem |
| `vault` | Planned | HashiCorp Vault integration |
| `aws` | Planned | AWS Secrets Manager |
| `custom` | Extensible | Custom provider via SecretStore |

For complete secrets documentation, see [Secrets Management](secrets-management.md).

#### Environment Files

Load environment variables from `.env` files in MapReduce workflows:

```yaml
# Source: src/cook/environment/config.rs:22-23
env_files:
  - .env                    # Base configuration
  - .env.local              # Local overrides (gitignored)
  - .env.${ENVIRONMENT}     # Environment-specific

env:
  PROJECT_NAME: my-project

map:
  input: items.json
  agent_template:
    - shell: "process --api-url $API_URL ${item.path}"  # API_URL from .env file
```

!!! note "Missing Files Are Silently Skipped"
    Environment files that don't exist are silently skipped with debug logging. This enables optional configuration files like `.env.local`.

For complete env_files documentation, see [Environment Files](environment-files.md).

#### Environment Profiles

Use profiles to define environment-specific configurations:

```yaml
# Source: src/cook/environment/config.rs:114-124
profiles:
  development:
    MAX_WORKERS: "2"
    AGENT_TIMEOUT: "60"
    API_URL: http://localhost:3000

  production:
    MAX_WORKERS: "20"
    AGENT_TIMEOUT: "600"
    API_URL: https://api.example.com

env:
  PROJECT_NAME: my-project

map:
  max_parallel: ${MAX_WORKERS}
  agent_timeout_secs: ${AGENT_TIMEOUT}

  agent_template:
    - shell: "curl $API_URL/process --data '${item}'"
```

Activate profiles via command line:

```bash
prodigy run workflow.yml --profile production
```

For complete profile documentation, see [Environment Profiles](environment-profiles.md).

### Complete Example: Parameterized MapReduce Workflow

```yaml
name: parameterized-processing
mode: mapreduce

# Environment files for layered configuration
env_files:
  - .env
  - .env.local

# Environment profiles
profiles:
  dev:
    MAX_WORKERS: "2"
    AGENT_TIMEOUT: "120"
  prod:
    MAX_WORKERS: "20"
    AGENT_TIMEOUT: "600"

# Global environment variables
env:
  # Project configuration
  PROJECT_NAME: my-project
  VERSION: "1.0.0"

  # Paths
  INPUT_FILE: items.json
  OUTPUT_DIR: results
  CONFIG_PATH: config/settings.json

  # Thresholds
  MIN_COVERAGE: "80"

# Secrets (masked in logs)
secrets:
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"

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

### Related Documentation

- [Environment Precedence](environment-precedence.md) - How environment values are resolved when multiple sources define the same variable
- [Environment Profiles](environment-profiles.md) - Profile-based configuration for different environments
- [Environment Files](environment-files.md) - Loading variables from `.env` files
- [Secrets Management](secrets-management.md) - Secure handling of sensitive values
- [Per-Command Environment Overrides](per-command-environment-overrides.md) - Step-level environment configuration
