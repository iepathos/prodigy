## Sub-Workflows

Execute child workflows as part of a parent workflow. Sub-workflows can run in parallel and have their own parameters and outputs, enabling modular workflow design and reusable validation/test pipelines.

### Basic Sub-Workflow Syntax

```yaml
name: deployment-pipeline
mode: standard

sub_workflows:
  - name: "lint-and-test"
    source: "workflows/quality-checks.yml"

  - name: "build"
    source: "workflows/build.yml"
    parameters:
      environment: "production"

  - name: "deploy"
    source: "workflows/deploy.yml"
    inputs:
      build_artifact: "${build.artifact_path}"
```

### Sub-Workflow Configuration

Each sub-workflow supports these fields:

```yaml
sub_workflows:
  - name: "validation"
    source: "path/to/workflow.yml"  # Required: workflow file path

    parameters:                       # Optional: parameter values
      env: "staging"
      timeout: 600

    inputs:                           # Optional: input from parent context
      commit_sha: "${git.commit}"
      branch: "${git.branch}"

    outputs:                          # Optional: extract values from sub-workflow
      - "test_coverage"
      - "artifact_url"

    parallel: false                   # Optional: run in parallel (default: false)

    continue_on_error: false          # Optional: continue if sub-workflow fails

    timeout: 1800                     # Optional: sub-workflow timeout (seconds)

    working_dir: "./sub-project"      # Optional: working directory for sub-workflow
```

### Parent-Child Context Isolation

Sub-workflows execute in isolated contexts:

- **Separate variable scope**: Sub-workflow variables don't leak to parent
- **Explicit input passing**: Use `inputs` to pass parent values to child
- **Output extraction**: Use `outputs` to capture child results
- **Independent git state**: Sub-workflows can operate in different directories

### Output Variable Extraction

Capture values from sub-workflow execution:

```yaml
# parent-workflow.yml
sub_workflows:
  - name: "build"
    source: "workflows/build.yml"
    outputs:
      - "docker_image_tag"
      - "artifact_sha256"

commands:
  # Use sub-workflow outputs
  - shell: "echo Deploying ${build.docker_image_tag}"
  - shell: "verify-checksum ${build.artifact_sha256}"
```

### Parallel Execution

Run multiple sub-workflows concurrently:

```yaml
sub_workflows:
  # These run in parallel
  - name: "unit-tests"
    source: "workflows/unit-tests.yml"
    parallel: true

  - name: "integration-tests"
    source: "workflows/integration-tests.yml"
    parallel: true

  - name: "e2e-tests"
    source: "workflows/e2e-tests.yml"
    parallel: true

# Parent waits for all parallel sub-workflows before continuing
commands:
  - shell: "echo All tests completed"
```

### Error Handling

Control behavior when sub-workflows fail:

```yaml
sub_workflows:
  # Critical step - fail parent if this fails
  - name: "security-scan"
    source: "workflows/security-scan.yml"
    continue_on_error: false  # Default behavior

  # Optional step - parent continues even if this fails
  - name: "performance-test"
    source: "workflows/perf-test.yml"
    continue_on_error: true
```

### Modular Pipeline Example

**parent-pipeline.yml:**
```yaml
name: ci-cd-pipeline
mode: standard

sub_workflows:
  # Step 1: Validation (sequential)
  - name: "validate"
    source: "workflows/validation.yml"
    outputs:
      - "validation_passed"

  # Step 2: Tests (parallel)
  - name: "unit-tests"
    source: "workflows/unit-tests.yml"
    parallel: true

  - name: "integration-tests"
    source: "workflows/integration-tests.yml"
    parallel: true

  # Step 3: Build (sequential, after tests)
  - name: "build"
    source: "workflows/build.yml"
    parameters:
      optimization_level: "3"
    outputs:
      - "artifact_path"

  # Step 4: Deploy (sequential, uses build output)
  - name: "deploy"
    source: "workflows/deploy.yml"
    inputs:
      artifact: "${build.artifact_path}"
      environment: "production"
```

**validation.yml** (reusable sub-workflow):
```yaml
name: validation
mode: standard

commands:
  - shell: "cargo fmt --check"
  - shell: "cargo clippy -- -D warnings"
  - shell: "echo validation_passed=true >> $PRODIGY_OUTPUT"
```

### Working Directory Isolation

Sub-workflows can operate in different directories:

```yaml
sub_workflows:
  # Backend tests in backend/
  - name: "backend-tests"
    source: "workflows/rust-tests.yml"
    working_dir: "./backend"

  # Frontend tests in frontend/
  - name: "frontend-tests"
    source: "workflows/js-tests.yml"
    working_dir: "./frontend"
```

### Timeout Configuration

Set execution time limits:

```yaml
sub_workflows:
  - name: "quick-tests"
    source: "workflows/smoke-tests.yml"
    timeout: 120  # 2 minutes

  - name: "comprehensive-tests"
    source: "workflows/full-suite.yml"
    timeout: 3600  # 1 hour
```

### Use Cases

**Modular Testing:**
- Separate unit, integration, and e2e tests into sub-workflows
- Run test suites in parallel for faster feedback
- Reuse test workflows across multiple projects

**Multi-Language Projects:**
- Separate workflows for each language/component
- Independent validation for microservices
- Coordinated deployment of multiple services

**Reusable Validation:**
- Shared linting/formatting workflows
- Common security scanning workflows
- Standardized compliance checks

**Environment-Specific Pipelines:**
```yaml
sub_workflows:
  # Different deployment sub-workflows per environment
  - name: "deploy-staging"
    source: "workflows/deploy.yml"
    parameters:
      environment: "staging"
      replicas: "2"

  - name: "deploy-production"
    source: "workflows/deploy.yml"
    parameters:
      environment: "production"
      replicas: "5"
```

### Complete Example

```yaml
name: monorepo-ci
mode: standard

sub_workflows:
  # Validate everything first
  - name: "validate"
    source: "shared/validate.yml"

  # Test all services in parallel
  - name: "api-tests"
    source: "services/api/test.yml"
    working_dir: "./services/api"
    parallel: true
    outputs:
      - "coverage"

  - name: "worker-tests"
    source: "services/worker/test.yml"
    working_dir: "./services/worker"
    parallel: true
    outputs:
      - "coverage"

  - name: "frontend-tests"
    source: "apps/frontend/test.yml"
    working_dir: "./apps/frontend"
    parallel: true
    outputs:
      - "coverage"

# After all sub-workflows complete
commands:
  - shell: "echo API coverage: ${api-tests.coverage}%"
  - shell: "echo Worker coverage: ${worker-tests.coverage}%"
  - shell: "echo Frontend coverage: ${frontend-tests.coverage}%"
  - shell: "generate-combined-coverage-report.sh"
```

### Sub-Workflow Result

Each sub-workflow execution produces a `SubWorkflowResult`:

```rust
SubWorkflowResult {
    name: String,           // Sub-workflow name
    success: bool,          // Execution success
    outputs: HashMap<>,     // Extracted output variables
    duration: Duration,     // Execution time
    error: Option<String>,  // Error message if failed
}
```

### Implementation Status

- ✅ Sub-workflow configuration parsing
- ✅ Sub-workflow validation (`validate_sub_workflows`)
- ✅ Parameter and input definitions
- ✅ Output extraction structure
- ✅ Parallel execution configuration
- ✅ Error handling options (continue_on_error)
- ✅ Timeout and working directory settings
- ✅ SubWorkflowExecutor structure
- ⏳ Executor integration with main workflow runtime (in progress)

*Note: Sub-workflow definitions are fully validated and composed, but execution integration with the main workflow orchestrator is currently in development.*

### Related Topics

- [Workflow Imports](index.md#workflow-imports) - Import shared configurations
- [Template System](template-system.md) - Parameterized workflows
- [Parameter Definitions](parameter-definitions.md) - Define sub-workflow parameters

