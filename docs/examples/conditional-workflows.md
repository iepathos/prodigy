# Conditional Workflows

This section covers workflows with conditional logic, validation, and environment-aware configuration.

## Example 4: Conditional Deployment

```yaml
- shell: "cargo test --quiet && echo true || echo false"
  id: "test"
  capture_output: "test_result"  # Canonical field name (alias: 'capture')
  capture_format: "boolean"  # Supported formats explained below
  timeout: 300  # Timeout in seconds (5 minutes)

- shell: "cargo build --release"
  when: "${test_result} == true"

- shell: "docker build -t myapp ."
  when: "${test_result} == true"
  on_success:
    shell: "docker push myapp:latest"
```

**Note:** `capture_format` options:
- `string` - Raw text output (default)
- `json` - Parse output as JSON object
- `lines` - Split output into array of lines
- `number` - Parse output as numeric value
- `boolean` - Parse as true/false based on exit code or output text

**Advanced capture options:**
```yaml
# Capture specific streams (stdout, stderr, exit_code, success, duration)
- shell: "cargo build 2>&1"
  capture_output: "build_output"
  capture_streams: "stdout,stderr,exit_code"  # Capture multiple streams

# Access captured values
- shell: "echo 'Exit code was ${build_output.exit_code}'"
```

---

## Example 5: Multi-Step Validation

!!! note "Iterative Validation"
    Use `validate` for completion checks with targeted gap filling. The `threshold` setting and `on_incomplete` handlers provide iterative refinement.

```yaml
# Source: Validation pattern from features.json
- claude: "/implement-feature auth"
  commit_required: true
  validate:
    commands:
      - shell: "cargo test auth"
      - shell: "cargo clippy -- -D warnings"
      - claude: "/validate-implementation --output validation.json"
    result_file: "validation.json"
    threshold: 90
    on_incomplete:
      claude: "/complete-gaps ${validation.gaps}"
      commit_required: true
      max_attempts: 2
```

**Validation Lifecycle Explanation:**

The validation system follows this flow:
1. **Execute validation commands** - Run tests, linting, and custom validation scripts
2. **Parse result file** - Read `validation.json` to extract score and gaps
3. **Check threshold** - Compare score against threshold (90 in this example)
4. **Populate `validation.gaps`** - If score < threshold, extract gaps from result file
5. **Execute `on_incomplete`** - Pass gaps to Claude for targeted fixes

**Result File Format:**

The validation result file (`validation.json`) should contain:
```json
{
  "score": 75,
  "gaps": [
    "Missing tests for login endpoint",
    "No error handling for invalid tokens",
    "Documentation incomplete for auth module"
  ]
}
```

The `${validation.gaps}` variable is populated from the `gaps` array in the result file. If the result file doesn't contain a `gaps` field, validation will fail with an error.

**Alternative: Shell Script Validation**

You can also use shell scripts that output structured data:
```yaml
validate:
  commands:
    - shell: |
        # Run tests and extract missing coverage
        cargo tarpaulin --output-format json > coverage.json
        # Parse coverage and create validation result
        jq '{score: .coverage, gaps: .uncovered_files}' coverage.json > validation.json
  result_file: "validation.json"
  threshold: 80
```

**Note:** Validation provides iterative completion checking with gap filling. Use it when you want to verify completeness and have Claude fill specific gaps.

---

## Example 6: Environment-Aware Workflow

```yaml
# Global environment variables (including secrets with masking)
env:
  # Regular variables
  NODE_ENV: production
  API_URL: https://api.production.com

  # Secrets (automatically masked in logs)
  # Use secret: true and value fields for sensitive data
  API_KEY:
    secret: true
    value: "${SECRET_API_KEY}"

  # Secret with external provider
  DB_PASSWORD:
    secret: true
    value: "${DB_PASSWORD}"
    # provider: "vault"  # Optional: external secret store (not yet implemented)

# Environment profiles for different contexts
profiles:
  production:
    env:
      API_URL: https://api.production.com
      LOG_LEVEL: error
    description: "Production environment with error-level logging"

  staging:
    env:
      API_URL: https://api.staging.com
      LOG_LEVEL: warn
    description: "Staging environment with warning-level logging"

# Load additional variables from .env files
# Note: Paths are relative to workflow file location
env_files:
  - .env
  - .env.production

# Workflow steps
- shell: "cargo build --release"

# Use environment variables in commands
- shell: "echo 'Deploying to ${NODE_ENV} at ${API_URL}'"

# Override environment for specific step using env field
- shell: "./deploy.sh"
  env:
    LOG_LEVEL: debug
```

**Source**: Environment configuration from src/cook/environment/config.rs:12-36, secret masking from src/cook/environment/config.rs:84-96

**Note:** Profiles are activated using the `--profile <name>` CLI flag when running workflows. For example:
```bash
# Use production profile
prodigy run workflow.yml --profile production

# Use staging profile
prodigy run workflow.yml --profile staging
```

**Secrets Masking**: Variables with `secret: true` are automatically masked in:
- Command output logs
- Error messages
- Event logs
- Checkpoint files

Example masked output:
```
$ echo 'API key is ***'
```

**Alternative Secrets Syntax (Legacy)**:

Both modern and legacy secret syntaxes are supported:

```yaml
# Modern approach (recommended)
env:
  API_KEY:
    secret: true
    value: "${SECRET_KEY}"

# Legacy approach (still supported)
secrets:
  API_KEY:
    provider: env
    key: "SECRET_KEY"
```

The modern `env`-based approach is recommended for consistency, but legacy workflows using the top-level `secrets:` field continue to work.

**Source**: Environment configuration from src/cook/environment/config.rs:12-36, secret support from src/cook/environment/config.rs:84-96, example workflow from workflows/mapreduce-env-example.yml:7-25, profile structure from tests/environment_workflow_test.rs:68-88
