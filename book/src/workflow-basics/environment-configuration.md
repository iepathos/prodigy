## Environment Configuration

Environment variables can be configured at multiple levels in Prodigy workflows, providing flexible control over workflow execution across different environments and contexts.

**Source**: Configuration structures defined in [src/cook/environment/config.rs:11-144](../../../src/cook/environment/config.rs), workflow configuration in [src/config/workflow.rs:12-39](../../../src/config/workflow.rs).

### Overview

Prodigy supports comprehensive environment configuration including:
- Global environment variables with static, dynamic, and conditional values
- Secret management with automatic log masking
- Environment profiles for different deployment contexts
- Environment files (.env format)
- Step-level environment overrides
- Variable interpolation in commands

This subsection provides a quick introduction to environment configuration. For comprehensive coverage, see the [Environment chapter](../environment/index.md).

### Global Environment Variables

Define environment variables in the `env:` block at the workflow root. Variables can be static strings, dynamically computed from commands, or conditionally set based on expressions.

**Source**: [workflows/environment-example.yml:4-18](../../../workflows/environment-example.yml)

```yaml
env:
  # Static environment variables
  NODE_ENV: production
  API_URL: https://api.example.com

  # Dynamic environment variable (computed from command)
  WORKERS:
    command: "nproc 2>/dev/null || echo 4"
    cache: true

  # Conditional environment variable
  DEPLOY_ENV:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"
```

**Type Definitions** ([src/cook/environment/config.rs:38-81](../../../src/cook/environment/config.rs)):
- `EnvValue::Static(String)` - Static string value
- `EnvValue::Dynamic(DynamicEnv)` - Computed from command with optional caching
- `EnvValue::Conditional(ConditionalEnv)` - Based on condition expression

### Environment Files

Load environment variables from .env format files using the `env_files:` field.

**Source**: [workflows/environment-example.yml:25-27](../../../workflows/environment-example.yml), [src/cook/environment/config.rs:22-23](../../../src/cook/environment/config.rs)

```yaml
env_files:
  - .env.production
  - .env.local
```

Variables from env files are merged with global env variables, with explicit env: block taking precedence.

For detailed information about .env file format and loading behavior, see [Environment Files](../environment/environment-files.md).

### Secrets Management

Secret environment variables are automatically masked in all log output, preventing accidental credential leaks.

**Source**: [workflows/mapreduce-env-example.yml:23-26](../../../workflows/mapreduce-env-example.yml), [src/cook/environment/config.rs:86-112](../../../src/cook/environment/config.rs)

```yaml
secrets:
  # Simple secret reference
  API_KEY: "${env:SECRET_API_KEY}"

  # Provider-based secret
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"
```

**Secret Providers** ([src/cook/environment/config.rs:101-112](../../../src/cook/environment/config.rs)):
- `env` - Environment variable
- `file` - File-based secret
- `vault` - HashiCorp Vault
- `aws` - AWS Secrets Manager
- Custom providers

For comprehensive secrets management documentation, see [Secrets Management](../environment/secrets-management.md).

### Environment Profiles

Profiles enable different environment configurations for various deployment contexts (development, staging, production).

**Source**: [workflows/environment-example.yml:30-39](../../../workflows/environment-example.yml), [src/cook/environment/config.rs:116-124](../../../src/cook/environment/config.rs)

```yaml
profiles:
  development:
    description: "Development environment with debug enabled"
    NODE_ENV: development
    API_URL: http://localhost:3000
    DEBUG: "true"

  production:
    description: "Production environment"
    NODE_ENV: production
    API_URL: https://api.example.com
    DEBUG: "false"
```

**Activate a profile**:
```bash
# Via command line flag
prodigy run workflow.yml --profile development

# Via environment variable
export PRODIGY_PROFILE=production
prodigy run workflow.yml
```

For detailed profile configuration and best practices, see [Environment Profiles](../environment/environment-profiles.md).

### Step-Level Environment Overrides

Commands can define their own environment variables that override global and profile settings.

**Source**: [workflows/environment-example.yml:47-60](../../../workflows/environment-example.yml), [src/cook/environment/config.rs:126-144](../../../src/cook/environment/config.rs)

```yaml
commands:
  - name: "Build frontend"
    shell: "echo 'Building with NODE_ENV='$NODE_ENV"
    env:
      BUILD_TARGET: production
      OPTIMIZE: "true"
    working_dir: ./frontend

  - name: "Run tests"
    shell: "pytest"
    env:
      PYTHONPATH: "./src:./tests"
      TEST_ENV: "true"
    working_dir: ./backend
    temporary: true  # Environment restored after this step
```

**Step Environment Fields** ([src/cook/environment/config.rs:128-144](../../../src/cook/environment/config.rs)):
- `env: HashMap<String, String>` - Step-specific variables
- `working_dir: Option<PathBuf>` - Working directory for this step
- `clear_env: bool` - Clear parent environment before applying step env
- `temporary: bool` - Restore environment after step completes

For detailed step-level configuration, see [Per-Command Environment Overrides](../environment/per-command-environment-overrides.md).

### Variable Interpolation

Environment variables can be referenced in commands using two syntaxes: `$VAR` or `${VAR}`.

**Source**: [workflows/mapreduce-env-example.yml:44-80](../../../workflows/mapreduce-env-example.yml)

```yaml
env:
  PROJECT_NAME: "my-project"
  OUTPUT_DIR: "output"

commands:
  # Both syntaxes work
  - shell: "echo Starting $PROJECT_NAME"
  - shell: "echo Output directory: ${OUTPUT_DIR}"

  # Use ${} for complex expressions
  - shell: "cp summary.json ${OUTPUT_DIR}/${PROJECT_NAME}-report.json"
```

Variable interpolation is available in:
- Shell commands
- Claude commands
- File paths
- MapReduce configuration (max_parallel, timeout, etc.)

For comprehensive variable interpolation documentation, see [Variable Interpolation](../variables/available-variables.md).

### Environment Precedence

When the same variable is defined at multiple levels, precedence is:

1. **Step-level env** (highest priority)
2. **Profile env**
3. **Global env**
4. **System environment** (lowest priority)

**Source**: Precedence logic in [src/cook/environment/builder.rs:48-53](../../../src/cook/environment/builder.rs), tests in [tests/environment_workflow_test.rs:62-132](../../../tests/environment_workflow_test.rs)

For detailed precedence rules and examples, see [Environment Precedence](../environment/environment-precedence.md).

### MapReduce Environment Variables

Environment variables are available across all MapReduce workflow phases: setup, map, reduce, and merge.

**Source**: [workflows/mapreduce-env-example.yml:1-95](../../../workflows/mapreduce-env-example.yml), [src/config/mapreduce.rs:24-38](../../../src/config/mapreduce.rs)

```yaml
name: mapreduce-workflow
mode: mapreduce

env:
  PROJECT_NAME: "my-project"
  MAX_RETRIES: "3"

setup:
  - shell: "echo Starting $PROJECT_NAME"

map:
  agent_template:
    - claude: "/process --project $PROJECT_NAME --retries $MAX_RETRIES"

reduce:
  - shell: "echo Completed $PROJECT_NAME workflow"

merge:
  commands:
    - shell: "echo Merging $PROJECT_NAME changes"
```

For comprehensive MapReduce environment documentation, see [MapReduce Environment Variables](../environment/mapreduce-environment-variables.md).

### Complete Example

This example demonstrates multiple environment features working together.

**Source**: Complete workflow in [workflows/environment-example.yml:1-70](../../../workflows/environment-example.yml)

```yaml
# Global environment with static, dynamic, and conditional values
env:
  NODE_ENV: production
  API_URL: https://api.example.com

  WORKERS:
    command: "nproc 2>/dev/null || echo 4"
    cache: true

  DEPLOY_ENV:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"

# Secrets (masked in logs)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files
env_files:
  - .env.production

# Profiles
profiles:
  development:
    NODE_ENV: development
    API_URL: http://localhost:3000
    DEBUG: "true"

# Commands with step-level overrides
commands:
  - name: "Build"
    shell: "npm run build"
    env:
      BUILD_TARGET: production
    working_dir: ./frontend

  - name: "Deploy"
    shell: "echo Deploying to $DEPLOY_ENV"
```
