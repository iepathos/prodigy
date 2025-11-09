## Complete Examples

This section provides end-to-end examples demonstrating multiple composition features working together.

### Example 1: Multi-Environment CI/CD Pipeline

This example uses templates, parameters, and inheritance for environment-specific deployments.

**base-ci-template.yml** (template in registry):
```yaml
name: ci-pipeline-template

parameters:
  definitions:
    environment:
      type: String
      description: "Deployment environment"
      validation: "matches('^(dev|staging|prod)$')"

    replicas:
      type: Number
      description: "Number of service replicas"
      default: 1

    run_tests:
      type: Boolean
      description: "Whether to run test suite"
      default: true

defaults:
  timeout: 600
  log_level: "info"

commands:
  - shell: "echo Deploying to ${environment} with ${replicas} replicas"
  - shell: "cargo build --release"
  - shell: |
      if [ "${run_tests}" = "true" ]; then
        cargo test --release
      fi
  - shell: "kubectl apply -f k8s/${environment}/deployment.yml"
  - shell: "kubectl scale deployment app --replicas=${replicas}"
```

**dev-deployment.yml**:
```yaml
name: dev-deployment

template:
  source:
    registry: "ci-pipeline-template"
  with:
    environment: "dev"
    replicas: 1
    run_tests: false  # Skip tests in dev for speed
```

**prod-deployment.yml**:
```yaml
name: prod-deployment

template:
  source:
    registry: "ci-pipeline-template"
  with:
    environment: "prod"
    replicas: 5
    run_tests: true  # Always test before prod

# Add production-specific safeguards
commands:
  - shell: "verify-release-notes.sh"
  - shell: "notify-team 'Production deployment starting'"
```

### Example 2: Modular Monorepo Testing

Uses sub-workflows and imports for testing multiple services in parallel.

**shared/common-setup.yml**:
```yaml
name: common-setup

commands:
  - shell: "git fetch origin"
  - shell: "npm install"
  - shell: "cargo build"
```

**monorepo-test.yml**:
```yaml
name: monorepo-test
mode: standard

imports:
  - path: "shared/common-setup.yml"

sub_workflows:
  # Test all services in parallel
  - name: "api-tests"
    source: "services/api/test.yml"
    working_dir: "./services/api"
    parallel: true
    outputs:
      - "coverage"
      - "test_count"

  - name: "worker-tests"
    source: "services/worker/test.yml"
    working_dir: "./services/worker"
    parallel: true
    outputs:
      - "coverage"
      - "test_count"

  - name: "frontend-tests"
    source: "apps/frontend/test.yml"
    working_dir: "./apps/frontend"
    parallel: true
    outputs:
      - "coverage"
      - "test_count"

commands:
  - shell: |
      echo "Test Results:"
      echo "  API: ${api-tests.test_count} tests, ${api-tests.coverage}% coverage"
      echo "  Worker: ${worker-tests.test_count} tests, ${worker-tests.coverage}% coverage"
      echo "  Frontend: ${frontend-tests.test_count} tests, ${frontend-tests.coverage}% coverage"
  - shell: "generate-combined-report.sh"
```

### Example 3: Layered Configuration with Extends

Uses inheritance to create environment-specific variations of a base workflow.

**base-config.yml**:
```yaml
name: base-config
mode: standard

defaults:
  log_level: "info"
  timeout: 300

env:
  APP_NAME: "my-service"
  DATABASE_POOL_SIZE: "10"

commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy"
  - shell: "cargo test"
  - shell: "cargo build"
```

**staging-config.yml**:
```yaml
name: staging-config
extends: "base-config.yml"

defaults:
  log_level: "debug"
  timeout: 600

env:
  DATABASE_POOL_SIZE: "20"
  ENABLE_DEBUG_ENDPOINTS: "true"

# Inherits all commands from base, adds staging-specific
commands:
  - shell: "run-integration-tests.sh"
  - shell: "deploy-to-staging.sh"
```

**production-config.yml**:
```yaml
name: production-config
extends: "base-config.yml"

defaults:
  log_level: "warn"
  timeout: 900

env:
  DATABASE_POOL_SIZE: "50"
  ENABLE_MONITORING: "true"
  RATE_LIMIT_ENABLED: "true"

commands:
  - shell: "verify-security-scan.sh"
  - shell: "cargo build --release"
  - shell: "run-smoke-tests.sh"
  - shell: "deploy-to-production.sh"
  - shell: "notify-deployment-complete.sh"
```

### Example 4: Complex Composition with Multiple Features

Combines imports, extends, template, parameters, and sub-workflows.

**templates/microservice-ci.yml**:
```yaml
name: microservice-ci-template

parameters:
  definitions:
    service_name:
      type: String
      description: "Name of the microservice"

    language:
      type: String
      description: "Programming language"
      validation: "matches('^(rust|typescript|python)$')"

    test_timeout:
      type: Number
      description: "Test timeout in seconds"
      default: 300

defaults:
  coverage_threshold: "80"

sub_workflows:
  - name: "lint"
    source: "workflows/${language}/lint.yml"
    working_dir: "./services/${service_name}"

  - name: "test"
    source: "workflows/${language}/test.yml"
    working_dir: "./services/${service_name}"
    timeout: "${test_timeout}"
    outputs:
      - "coverage"

commands:
  - shell: "echo Testing ${service_name} (${language})"
  - shell: |
      if [ "${test.coverage}" -lt "${coverage_threshold}" ]; then
        echo "Coverage ${test.coverage}% below threshold ${coverage_threshold}%"
        exit 1
      fi
```

**service-api-ci.yml** (uses the template):
```yaml
name: api-service-ci

imports:
  - path: "shared/docker-utils.yml"

template:
  source:
    file: "templates/microservice-ci.yml"
  with:
    service_name: "api"
    language: "rust"
    test_timeout: 600

commands:
  - shell: "docker build -t api:latest ./services/api"
  - shell: "docker push api:latest"
```

### Example 5: Workflow Registry Pattern

Demonstrates using a template registry for standardized workflows across teams.

**Setup Registry** (one-time):
```bash
mkdir -p ~/.prodigy/templates
cp standard-ci.yml ~/.prodigy/templates/
cp security-scan.yml ~/.prodigy/templates/
cp deployment.yml ~/.prodigy/templates/
```

**team-workflow.yml**:
```yaml
name: team-workflow
mode: standard

# Use multiple registry templates
template:
  source:
    registry: "standard-ci"
  with:
    project_type: "rust"

# Import additional registry workflows
imports:
  - path: ~/.prodigy/templates/security-scan.yml

# Extend with team-specific configuration
commands:
  - shell: "run-team-specific-tests.sh"
```

### Example 6: Progressive Composition

Builds complexity through layers of composition.

**Layer 1 - Base** (minimal.yml):
```yaml
name: minimal
defaults:
  timeout: 300
commands:
  - shell: "cargo build"
```

**Layer 2 - Add Testing** (with-tests.yml):
```yaml
name: with-tests
extends: "minimal.yml"
commands:
  - shell: "cargo test"
```

**Layer 3 - Add Linting** (with-quality.yml):
```yaml
name: with-quality
extends: "with-tests.yml"
commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy"
```

**Layer 4 - Full CI** (full-ci.yml):
```yaml
name: full-ci
extends: "with-quality.yml"

parameters:
  definitions:
    deploy_env:
      type: String

commands:
  - shell: "cargo build --release"
  - shell: "deploy-to-${deploy_env}.sh"
```

### Running the Examples

```bash
# Example 1: Template-based deployment
prodigy run dev-deployment.yml
prodigy run prod-deployment.yml --param replicas=10

# Example 2: Monorepo testing
prodigy run monorepo-test.yml

# Example 3: Environment-specific configs
prodigy run staging-config.yml
prodigy run production-config.yml

# Example 4: Complex composition
prodigy run service-api-ci.yml

# Example 5: Registry templates
prodigy run team-workflow.yml

# Example 6: Progressive composition
prodigy run full-ci.yml --param deploy_env=staging
```

