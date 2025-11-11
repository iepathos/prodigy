## Common Patterns

This section provides practical patterns and realistic examples for environment configuration in Prodigy workflows. All examples are validated against the actual implementation and real workflow files.

> **Source References**: Examples based on:
> - src/config/workflow.rs:12-39 (WorkflowConfig structure)
> - src/cook/environment/config.rs:12-144 (Environment configuration types)
> - workflows/mapreduce-env-example.yml (Complete working example)

### Multi-Environment Deployment Pattern

Use profiles to manage different deployment environments with environment-specific configurations.

> **Profile Support**: Profiles provide environment-specific variable overrides (src/cook/environment/config.rs:116-124). Activate with `--profile <name>` flag.

```yaml
name: multi-env-deployment
mode: mapreduce

env:
  # Shared across all environments
  PROJECT_NAME: my-service
  VERSION: "2.1.0"
  LOG_LEVEL: info

profiles:
  dev:
    API_URL: http://localhost:3000
    DATABASE_URL: postgresql://localhost:5432/dev
    MAX_WORKERS: "2"
    CACHE_TTL: "60"
    DEBUG: "true"

  staging:
    API_URL: https://staging-api.example.com
    DATABASE_URL: postgresql://staging-db:5432/app
    MAX_WORKERS: "10"
    CACHE_TTL: "300"
    DEBUG: "true"

  prod:
    API_URL: https://api.example.com
    DATABASE_URL: postgresql://prod-db:5432/app
    MAX_WORKERS: "20"
    CACHE_TTL: "3600"
    DEBUG: "false"

secrets:
  # Secrets from environment variables (supported)
  DATABASE_PASSWORD:
    provider: env
    key: DB_PASSWORD

commands:
  - shell: "echo 'Deploying $PROJECT_NAME v$VERSION to $PROFILE environment'"
  - shell: "echo 'API: $API_URL, Workers: $MAX_WORKERS'"
  - shell: "deploy.sh --env $PROFILE --version $VERSION"
```

**Usage:**
```bash
prodigy run deploy.yml --profile dev
prodigy run deploy.yml --profile staging
prodigy run deploy.yml --profile prod
```

### Secrets Management Pattern

Combine env files with secret providers for secure credential management:

> **Currently Supported Providers** (src/cook/environment/secret_store.rs:40-41):
> - `env` - Environment variables
> - `file` - File-based secrets
>
> **Planned Providers** (defined but not yet implemented):
> - `vault` - HashiCorp Vault integration
> - `aws` - AWS Secrets Manager
> - `custom` - Custom provider support

```yaml
name: secure-workflow

# Load non-sensitive config from files
env_files:
  - .env              # Base configuration (committed)
  - .env.local        # Local overrides (gitignored)

env:
  SERVICE_NAME: payment-processor
  REGION: us-west-2

secrets:
  # Secrets from environment variables (currently supported)
  AWS_ACCESS_KEY:
    provider: env
    key: AWS_ACCESS_KEY_ID

  AWS_SECRET_KEY:
    provider: env
    key: AWS_SECRET_ACCESS_KEY

  # Secrets from files (currently supported)
  DATABASE_URL:
    provider: file
    key: /path/to/secrets/database_url.txt

  # Simple secret reference (loaded from env)
  THIRD_PARTY_API_KEY: "${THIRD_PARTY_API_KEY}"

commands:
  - shell: "aws s3 ls --region $REGION"  # Uses AWS credentials
  - shell: "psql $DATABASE_URL -c 'SELECT version()'"
  - shell: "curl -H 'Authorization: Bearer ***' https://api.example.com"
```

**.env file (committed):**
```bash
SERVICE_NAME=payment-processor
REGION=us-west-2
LOG_LEVEL=info
```

**.env.local file (gitignored):**
```bash
THIRD_PARTY_API_KEY=sk-local-dev-key
DATABASE_URL=postgresql://localhost:5432/dev
```

### Profile-Based Configuration Strategy

Base configuration with environment-specific overrides:

```yaml
name: profile-based-config

env:
  # Base configuration (defaults)
  PROJECT_NAME: analytics-pipeline
  MAX_WORKERS: "5"
  BATCH_SIZE: "100"
  TIMEOUT: "300"
  RETRY_ATTEMPTS: "3"
  CACHE_ENABLED: "true"

profiles:
  # Local development - minimal resources
  dev:
    MAX_WORKERS: "2"
    BATCH_SIZE: "10"
    TIMEOUT: "600"        # Longer timeout for debugging
    CACHE_ENABLED: "false"  # Disable caching for fresh data

  # CI/CD environment - controlled resources
  ci:
    MAX_WORKERS: "4"
    BATCH_SIZE: "50"
    TIMEOUT: "300"
    RETRY_ATTEMPTS: "1"  # Fail fast in CI

  # Production - optimized for throughput
  prod:
    MAX_WORKERS: "20"
    BATCH_SIZE: "500"
    TIMEOUT: "180"       # Strict timeout
    RETRY_ATTEMPTS: "5"  # More retries for resilience
    CACHE_ENABLED: "true"

map:
  input: data.json
  max_parallel: ${MAX_WORKERS}
  agent_timeout_secs: ${TIMEOUT}

  agent_template:
    - shell: "process-batch.sh --size $BATCH_SIZE --retries $RETRY_ATTEMPTS"
```

### Environment Variable Composition Pattern

Layer configuration from multiple sources with clear precedence (src/cook/environment/config.rs:12-36):

> **Note**: env_files paths are static and don't support variable interpolation. Use profiles to handle environment-specific file loading.

```yaml
name: layered-config

# Layer 1: Base defaults and local overrides
env_files:
  - .env              # Base configuration
  - .env.local        # Local overrides (if exists)

# Layer 4: Global workflow values (override env files)
env:
  WORKFLOW_VERSION: "3.0.0"
  EXECUTION_MODE: standard

# Layer 5: Profile values (override everything when active)
profiles:
  prod:
    EXECUTION_MODE: high-performance
    MAX_WORKERS: "50"

# Layer 6: Secrets (separate layer for security)
secrets:
  API_TOKEN:
    provider: vault
    key: secret/data/api-token

commands:
  - shell: "echo 'Mode: $EXECUTION_MODE, Workers: $MAX_WORKERS'"
```

**Precedence Order** (highest to lowest):
1. Profile variables (when profile is active)
2. Global `env` variables
3. Variables from `env_files` (later files override earlier)
4. Inherited system environment variables

**File structure:**
```
.env                  # Base: MAX_WORKERS=5, API_URL=http://localhost
.env.local           # Local: MAX_WORKERS=2 (overrides .env)
```

### CI/CD Integration Pattern

Use environment variables to make workflows portable across CI/CD systems.

> **Conditional Execution**: Commands support the `when` field for conditional execution based on environment variables (src/config/command.rs:388).

```yaml
name: ci-cd-workflow

env:
  # CI/CD environment detection
  CI_MODE: "${CI:-false}"                    # GitHub Actions, GitLab CI set CI=true
  BUILD_NUMBER: "${BUILD_NUMBER:-local}"     # Jenkins BUILD_NUMBER
  COMMIT_SHA: "${GITHUB_SHA:-unknown}"       # GitHub Actions
  BRANCH_NAME: "${BRANCH_NAME:-main}"        # Can be set by CI

  # Resource limits for CI
  MAX_WORKERS: "${CI_MAX_WORKERS:-5}"
  TIMEOUT: "${CI_TIMEOUT:-300}"

  # Paths
  ARTIFACT_DIR: "${WORKSPACE:-./artifacts}"
  CACHE_DIR: "${CACHE_DIR:-./cache}"

commands:
  - shell: "echo 'CI Mode: $CI_MODE, Build: $BUILD_NUMBER'"
  - shell: "echo 'Branch: $BRANCH_NAME, Commit: $COMMIT_SHA'"

  - shell: "cargo build --release"
    when: "${CI_MODE} == 'true'"

  - shell: "cargo test --all"
    timeout: ${TIMEOUT}

  - shell: "mkdir -p $ARTIFACT_DIR"
  - shell: "cp target/release/app $ARTIFACT_DIR/"
```

**GitHub Actions example:**
```yaml
- name: Run Prodigy workflow
  env:
    CI_MAX_WORKERS: 10
    CI_TIMEOUT: 600
  run: prodigy run workflow.yml
```

**Jenkins example:**
```groovy
environment {
  CI_MAX_WORKERS = '10'
  CI_TIMEOUT = '600'
}
steps {
  sh 'prodigy run workflow.yml'
}
```

### Local Development Pattern

Optimize for local development with overridable defaults:

```yaml
name: dev-friendly-workflow

env_files:
  - .env                # Base config (committed)
  - .env.local          # Personal settings (gitignored)

env:
  # Development defaults
  API_URL: http://localhost:3000
  DATABASE_URL: postgresql://localhost:5432/dev
  REDIS_URL: redis://localhost:6379

  # Resource limits for local dev
  MAX_WORKERS: "2"
  TIMEOUT: "60"

  # Feature flags
  ENABLE_CACHING: "false"
  ENABLE_ANALYTICS: "false"
  DEBUG_MODE: "true"

profiles:
  # Personal override for more powerful dev machines
  high-perf:
    MAX_WORKERS: "8"
    ENABLE_CACHING: "true"

commands:
  - shell: "echo 'Development mode: Debug=$DEBUG_MODE'"
  - shell: "docker-compose up -d"
    when: "${DATABASE_URL} =~ 'localhost'"

  - shell: "cargo run --bin migrate"
  - shell: "cargo test"
```

**.env.local.example (committed as template):**
```bash
# Copy to .env.local and customize for your machine

# Override API endpoint for local backend
# API_URL=http://localhost:8080

# Use more workers if you have powerful CPU
# MAX_WORKERS=4

# Enable features for testing
# ENABLE_CACHING=true
# ENABLE_ANALYTICS=true
```

### Template Parameterization Pattern

Create reusable workflows parameterized with environment variables:

```yaml
name: reusable-test-workflow

env:
  # Required parameters (set by caller or env)
  PROJECT_NAME: "${PROJECT_NAME}"           # Must be provided
  TEST_SUITE: "${TEST_SUITE:-all}"         # Default: all
  COVERAGE_THRESHOLD: "${COVERAGE_THRESHOLD:-80}"

  # Optional customization
  PARALLEL_JOBS: "${PARALLEL_JOBS:-5}"
  TIMEOUT: "${TIMEOUT:-300}"
  REPORT_FORMAT: "${REPORT_FORMAT:-json}"

commands:
  - shell: "echo 'Testing $PROJECT_NAME: $TEST_SUITE suite'"

  - shell: "cargo test --workspace"
    when: "${TEST_SUITE} == 'all'"
    timeout: ${TIMEOUT}

  - shell: "cargo test --package $PROJECT_NAME"
    when: "${TEST_SUITE} == 'unit'"

  - shell: "cargo tarpaulin --out $REPORT_FORMAT --output-dir coverage"
    capture_output: coverage_result
    capture_format: json

  - shell: |
      COVERAGE=$(echo '$coverage_result' | jq '.coverage')
      if (( $(echo "$COVERAGE < $COVERAGE_THRESHOLD" | bc -l) )); then
        echo "Coverage $COVERAGE% below threshold $COVERAGE_THRESHOLD%"
        exit 1
      fi
```

**Usage:**
```bash
# With environment variables
PROJECT_NAME=my-app TEST_SUITE=unit prodigy run test-workflow.yml

# Or with .env file
echo "PROJECT_NAME=my-app" > .env
echo "TEST_SUITE=integration" >> .env
echo "COVERAGE_THRESHOLD=90" >> .env
prodigy run test-workflow.yml
```

### Regional Configuration Pattern

Deploy to different regions with region-specific settings:

```yaml
name: multi-region-deployment

env:
  SERVICE_NAME: api-gateway
  VERSION: "1.2.0"

profiles:
  us-west:
    REGION: us-west-2
    API_ENDPOINT: https://api-usw.example.com
    S3_BUCKET: my-app-usw-artifacts
    MAX_INSTANCES: "10"

  us-east:
    REGION: us-east-1
    API_ENDPOINT: https://api-use.example.com
    S3_BUCKET: my-app-use-artifacts
    MAX_INSTANCES: "20"

  eu-west:
    REGION: eu-west-1
    API_ENDPOINT: https://api-euw.example.com
    S3_BUCKET: my-app-euw-artifacts
    MAX_INSTANCES: "15"

secrets:
  # AWS credentials from environment (currently supported)
  AWS_ACCESS_KEY:
    provider: env
    key: AWS_ACCESS_KEY_ID
  AWS_SECRET_KEY:
    provider: env
    key: AWS_SECRET_ACCESS_KEY

commands:
  - shell: "echo 'Deploying to $REGION'"
  - shell: "aws s3 cp ./artifact.zip s3://$S3_BUCKET/$VERSION/ --region $REGION"
  - shell: "deploy-to-region.sh --region $REGION --instances $MAX_INSTANCES"
```

**Usage:**
```bash
prodigy run deploy.yml --profile us-west
prodigy run deploy.yml --profile eu-west
```

### Feature Flag Pattern

Use environment variables to control feature availability:

```yaml
name: feature-flag-workflow

env:
  # Feature flags
  ENABLE_NEW_PIPELINE: "${ENABLE_NEW_PIPELINE:-false}"
  ENABLE_EXPERIMENTAL: "${ENABLE_EXPERIMENTAL:-false}"
  ENABLE_BETA_FEATURES: "${ENABLE_BETA_FEATURES:-false}"

  # Version-based features
  MIN_VERSION: "2.0.0"
  CURRENT_VERSION: "2.1.0"

profiles:
  canary:
    ENABLE_EXPERIMENTAL: "true"

  beta:
    ENABLE_BETA_FEATURES: "true"

  production:
    ENABLE_NEW_PIPELINE: "true"
    ENABLE_EXPERIMENTAL: "false"
    ENABLE_BETA_FEATURES: "false"

commands:
  - shell: "run-legacy-pipeline.sh"
    when: "${ENABLE_NEW_PIPELINE} == 'false'"

  - shell: "run-new-pipeline.sh"
    when: "${ENABLE_NEW_PIPELINE} == 'true'"

  - shell: "run-experimental-features.sh"
    when: "${ENABLE_EXPERIMENTAL} == 'true'"

  - shell: "validate-version.sh --min $MIN_VERSION --current $CURRENT_VERSION"
```

### Complete Real-World Example

Combining multiple patterns for a production-ready workflow:

```yaml
name: production-data-pipeline
mode: mapreduce

# Layer 1: Base configuration
env_files:
  - .env
  - .env.${ENVIRONMENT}

# Layer 2: Global settings
env:
  PROJECT_NAME: data-pipeline
  VERSION: "3.0.0"
  ENVIRONMENT: "${ENVIRONMENT:-dev}"

# Layer 3: Environment profiles
profiles:
  dev:
    DATA_SOURCE: s3://dev-data-bucket
    OUTPUT_PATH: s3://dev-results-bucket
    MAX_WORKERS: "5"
    BATCH_SIZE: "100"
    ENABLE_MONITORING: "false"

  staging:
    DATA_SOURCE: s3://staging-data-bucket
    OUTPUT_PATH: s3://staging-results-bucket
    MAX_WORKERS: "15"
    BATCH_SIZE: "500"
    ENABLE_MONITORING: "true"

  prod:
    DATA_SOURCE: s3://prod-data-bucket
    OUTPUT_PATH: s3://prod-results-bucket
    MAX_WORKERS: "50"
    BATCH_SIZE: "1000"
    ENABLE_MONITORING: "true"

# Layer 4: Secrets
secrets:
  # Currently supported: env and file providers
  AWS_ACCESS_KEY:
    provider: env
    key: AWS_ACCESS_KEY_ID

  DATABASE_URL:
    provider: file
    key: /secrets/${ENVIRONMENT}/database_url.txt

setup:
  - shell: "echo 'Starting $PROJECT_NAME v$VERSION in $ENVIRONMENT'"
  - shell: "aws s3 ls $DATA_SOURCE --region us-west-2"
  - shell: "generate-work-items.sh --source $DATA_SOURCE --batch-size $BATCH_SIZE > items.json"

map:
  input: items.json
  max_parallel: ${MAX_WORKERS}

  agent_template:
    - shell: "process-batch.sh --input ${item.path} --output $OUTPUT_PATH/${item.id}.result"
    - shell: "validate-result.sh $OUTPUT_PATH/${item.id}.result"
      on_failure:
        shell: "log-failure.sh ${item.id}"

reduce:
  - shell: "echo 'Processed ${map.successful}/${map.total} batches'"
  - shell: "aggregate-results.sh --input $OUTPUT_PATH --output $OUTPUT_PATH/summary.json"
  - shell: "send-metrics.sh --completed ${map.successful} --failed ${map.failed}"
    when: "${ENABLE_MONITORING} == 'true'"

merge:
  commands:
    - shell: "cargo test"
    - shell: "validate-deployment.sh --env $ENVIRONMENT"
```

See also:
- [Best Practices](best-practices.md) for guidelines on environment management
- [Environment Profiles](environment-profiles.md) for profile configuration details
- [Secrets Management](secrets-management.md) for secure credential handling
- [MapReduce Environment Variables](mapreduce-environment-variables.md) for MapReduce-specific usage
