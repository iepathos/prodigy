## Workflow Configuration

Workflow configuration defines the sequence of commands to execute and their execution environment. Workflows are the core of Prodigy's automation capabilities.

!!! tip "Quick Reference"
    **Available command types:** `claude`, `shell`, `foreach`, `write_file`, `validate`, `analyze`
    **Error handling:** `on_failure`, `on_success`, `when` (conditional)
    **Output control:** `capture_output`, `capture_format`, `output_file`

### Overview

Workflows are defined in `.prodigy/workflow.yml` files and specify:
- Commands to execute in sequence
- Iteration and loop control
- Error handling behavior
- Environment variables
- Timeout and retry settings

For detailed workflow syntax and MapReduce capabilities, see the [Workflow Basics](../workflow-basics/index.md) chapter.

### Basic Workflow Structure

```yaml title=".prodigy/workflow.yml"
commands:
  - prodigy-code-review
  - prodigy-lint
  - prodigy-test
```

### Command Types

=== "claude"
    Execute Claude CLI commands with arguments:

    ```yaml
    commands:
      - claude: "/prodigy-implement-spec ${SPEC_ID}"
      - claude: "/prodigy-code-review --focus security"
    ```

=== "shell"
    Run shell commands:

    ```yaml
    commands:
      - shell: "cargo test"
      - shell: "npm run build"
    ```

=== "foreach"
    Iterate over items with parallel execution:

    ```yaml
    commands:
      - foreach: ["file1.rs", "file2.rs", "file3.rs"]
        parallel: 3
        do:
          - shell: "cargo check --lib ${item}"
    ```

=== "write_file"
    Create files with variable interpolation:

    ```yaml
    # Source: src/config/command.rs:279-317
    commands:
      - write_file:
          path: "output/${name}.json"
          content: '{"result": "${result}"}'
          format: json        # text, json, or yaml
          mode: "0644"        # file permissions
          create_dirs: true   # create parent directories
    ```

=== "validate"
    Check implementation completeness:

    ```yaml
    # Source: src/cook/workflow/validation.rs:11-49
    commands:
      - claude: "/prodigy-implement-spec ${SPEC_ID}"
      - validate:
          shell: "cargo test --lib"
          threshold: 100           # completion percentage
          timeout: 300             # seconds
          on_incomplete:
            claude: "/prodigy-fix-gaps ${SPEC_ID}"
            max_attempts: 3
            fail_workflow: true
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

Commands support multiple error handling strategies:

```yaml
commands:
  - name: prodigy-risky-operation
    metadata:
      continue_on_error: true
      retries: 3
      timeout: 600

  # Conditional execution on failure
  # Source: src/config/command.rs:367-368
  - shell: "cargo test"
    on_failure:
      claude: "/prodigy-debug-test-failure"
      max_attempts: 3
      fail_workflow: true
      commit_required: true

  # Conditional execution on success
  # Source: src/config/command.rs:371-372
  - shell: "cargo build --release"
    on_success:
      shell: "cargo install --path ."
```

#### Conditional Execution

Use `when` clauses to conditionally run commands:

```yaml
# Source: src/config/command.rs:384
commands:
  - shell: "cargo build"
    id: build
    capture_output: build_result

  - claude: "/prodigy-test"
    when: "${build_result.exit_code} == 0"

  - shell: "deploy.sh"
    when: "${env} == 'production'"
```

#### Output Capture

Capture command output for use in subsequent commands:

```yaml
# Source: src/config/command.rs:362-396
commands:
  - shell: "cargo metadata --format-version 1"
    capture_output: metadata        # variable name to store output
    capture_format: json            # json, text, or lines
    capture_streams: stdout         # stdout, stderr, or both

  - shell: "echo 'Version: ${metadata.version}'"

  # Redirect output to file
  - shell: "cargo test 2>&1"
    output_file: "test-results.log"
```

### Configuration Location

Workflows can be specified in multiple ways:

1. **Explicit path**: `prodigy run /path/to/workflow.yml`
2. **Project default**: `.prodigy/workflow.yml` in project directory
3. **Embedded in config**: Nested under `workflow:` in config files

### Environment Variables in Workflows

Workflows have a dedicated `env:` block for defining environment variables with advanced features like secrets, profiles, and step-level overrides:

```yaml title=".prodigy/workflow.yml"
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
- **Env Files**: Load variables from `.env` files via `env_files`

To select a profile, use the `--profile` flag:

```bash
prodigy run workflow.yml --profile prod
```

See [Environment Variables - Workflow Section](environment-variables.md#workflow-environment-variables) for complete documentation.

!!! note "Config Variables vs Workflow Env"
    Project config `variables` are separate from workflow `env` and serve different purposes:

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
