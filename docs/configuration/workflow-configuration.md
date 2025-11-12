## Workflow Configuration

Workflow configuration defines the sequence of commands to execute and their execution environment. Workflows are the core of Prodigy's automation capabilities.

### Overview

Workflows are defined in `.prodigy/workflow.yml` files and specify:
- Commands to execute in sequence
- Iteration and loop control
- Error handling behavior
- Environment variables
- Timeout and retry settings

For detailed workflow syntax and MapReduce capabilities, see the [Workflow Basics](../workflow-basics/index.md) chapter.

### Basic Workflow Structure

```yaml
# .prodigy/workflow.yml
commands:
  - prodigy-code-review
  - prodigy-lint
  - prodigy-test
```

### Advanced Workflow Features

#### Structured Commands

Commands can include arguments and options:

```yaml
commands:
  - name: prodigy-implement-spec
    args: ["${SPEC_ID}"]
    options:
      focus: performance
  - name: prodigy-code-review
    options:
      severity: high
```

#### Iteration Control

```yaml
max_iterations: 5
commands:
  - prodigy-fix-issues
  - prodigy-test
```

#### Error Handling

```yaml
commands:
  - name: prodigy-risky-operation
    metadata:
      continue_on_error: true
      retries: 3
      timeout: 600
```

### Configuration Location

Workflows can be specified in multiple ways:

1. **Explicit path**: `prodigy run /path/to/workflow.yml`
2. **Project default**: `.prodigy/workflow.yml` in project directory
3. **Embedded in config**: Nested under `workflow:` in config files

### Environment Variables in Workflows

Workflows have a dedicated `env:` block for defining environment variables with advanced features like secrets, profiles, and step-level overrides:

```yaml
# .prodigy/workflow.yml
name: deployment

env:
  # Plain variables
  ENVIRONMENT: staging
  API_URL: https://staging.api.com

  # Secret variables (masked in logs)
  API_KEY:
    secret: true
    value: "${STAGING_API_KEY}"  # From system env

  # Profile-specific values
  DEPLOY_TARGET:
    default: dev-server
    staging: staging-cluster
    prod: prod-cluster

commands:
  - shell: "deploy --env ${ENVIRONMENT} --url ${API_URL} --key ${API_KEY}"
    # Output: deploy --env staging --url https://staging.api.com --key ***
```

**Key Features**:
- **Secrets**: Automatically masked in all output (`secret: true`)
- **Profiles**: Different values for dev/staging/prod environments
- **Step Overrides**: Override variables for specific commands
- **Interpolation**: Reference system environment variables

See [Environment Variables - Workflow Section](environment-variables.md#workflow-environment-variables) for complete documentation.

**Note**: Project config `variables` are separate from workflow `env` and serve different purposes:
- **Workflow `env:`**: Runtime environment variables, supports secrets and profiles
- **Config `variables:`**: Project metadata and settings (deprecated for workflow use)

### MapReduce Workflows

For parallel processing of large datasets, use MapReduce mode:

```yaml
name: process-all-files
mode: mapreduce

map:
  input: items.json
  json_path: "$.files[*]"
  agent_template:
    - claude: "/process-file '${item.path}'"
  max_parallel: 10

reduce:
  - claude: "/summarize ${map.results}"
```

See the [MapReduce Workflows](../mapreduce/index.md) chapter for complete documentation.

### Workflow Precedence

When multiple workflow sources exist, Prodigy uses this precedence:

1. Explicit path via `prodigy run workflow.yml` (highest)
2. `.prodigy/workflow.yml` in project directory
3. Default workflow configuration (lowest)
