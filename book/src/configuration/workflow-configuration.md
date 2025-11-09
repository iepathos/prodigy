## Workflow Configuration

Workflow configuration defines the sequence of commands to execute and their execution environment. Workflows are the core of Prodigy's automation capabilities.

### Overview

Workflows are defined in `.prodigy/workflow.yml` files and specify:
- Commands to execute in sequence
- Iteration and loop control
- Error handling behavior
- Environment variables
- Timeout and retry settings

For detailed workflow syntax and MapReduce capabilities, see the [Workflow Basics](../workflow-basics.md) chapter.

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

Workflows can reference project variables and environment variables:

```yaml
# .prodigy/config.yml
variables:
  environment: staging
  api_url: https://staging.api.com
```

```yaml
# .prodigy/workflow.yml
commands:
  - name: deploy
    args: ["${environment}"]
    options:
      api_url: "${api_url}"
```

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

See the [MapReduce Workflows](../mapreduce-workflows.md) chapter for complete documentation.

### Workflow Precedence

When multiple workflow sources exist, Prodigy uses this precedence:

1. Explicit path via `prodigy run workflow.yml` (highest)
2. `.prodigy/workflow.yml` in project directory
3. Default workflow configuration (lowest)

### See Also

- [Workflow Basics](../workflow-basics.md) - Complete workflow syntax and features
- [MapReduce Workflows](../mapreduce-workflows.md) - Parallel execution patterns
- [Project Configuration Structure](project-configuration-structure.md) - Project variables for workflows
- [Environment Variables](environment-variables.md) - Using environment variables in workflows

