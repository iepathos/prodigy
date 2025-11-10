## Best Practices

This section covers best practices for creating and maintaining automated documentation workflows with Prodigy.

### Workflow Design Best Practices

#### Keep Workflows Simple and Focused

Each workflow should have a single, clear purpose. Break complex workflows into smaller, composable pieces.

**Good Example:**
```yaml
# Simple, focused workflow for drift detection
name: prodigy-book-docs-drift-detection
mode: mapreduce

setup:
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME"
  - claude: "/prodigy-detect-documentation-gaps --project $PROJECT_NAME"
```

**Source**: workflows/book-docs-drift.yml:1-34

**Anti-Pattern:**
```yaml
# Overly complex workflow doing too many things
- claude: "/analyze-features"
- claude: "/detect-gaps"
- claude: "/fix-everything"
- claude: "/deploy-to-production"
- claude: "/send-notifications"
```

#### Use Environment Variables for Parameterization

Define reusable values as environment variables instead of hardcoding them throughout your workflow.

```yaml
env:
  PROJECT_NAME: "Prodigy"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"
  BOOK_DIR: "book"
  MAX_PARALLEL: "3"

setup:
  - claude: "/prodigy-analyze-features --project $PROJECT_NAME --features $FEATURES_PATH"
```

**Source**: workflows/book-docs-drift.yml:8-21

**Benefits:**
- Easy configuration changes without touching workflow logic
- Consistent naming across all commands
- Support for different environments via profiles

See [Environment Variables](../configuration/environment-variables.md) for comprehensive documentation.

#### Include Validation Steps

Always validate your outputs to ensure quality. Use `goal_seek` for iterative improvement.

```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/prodigy-coverage --improve"
    validate: "cargo tarpaulin --print-summary 2>/dev/null | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
```

**Source**: workflows/goal-seeking-examples.yml:6-13

#### Capture and Use Command Output

Use `capture_output: true` to make command results available to subsequent steps.

```yaml
- shell: "prodigy validate-spec --spec specs/auth.md --json"
  capture_output: true

- claude: "/implement-missing-features ${shell.output}"
```

#### Add Commit Requirements

Specify `commit_required: true` for steps that should produce changes. This ensures your workflow makes actual progress.

```yaml
- claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME"
  commit_required: true
```

**Source**: workflows/book-docs-drift.yml:48

### MapReduce Best Practices

#### Choose Appropriate Parallelism

Set `max_parallel` based on:
- Resource constraints (CPU, memory)
- Network/API rate limits
- Complexity of each work item

```yaml
map:
  max_parallel: 4  # Run up to 4 agents in parallel
```

**Source**: workflows/documentation-drift-mapreduce.yml:74

**Guidelines:**
- **Simple operations** (file analysis, linting): 8-10 parallel agents
- **Medium operations** (code fixes, refactoring): 4-6 parallel agents
- **Complex operations** (feature implementation): 2-3 parallel agents
- **Claude-heavy workflows**: 3-5 parallel agents (respects rate limits)

#### Use Setup Phase for Work Item Generation

The setup phase is ideal for generating work items dynamically based on codebase analysis.

```yaml
setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME"
  - claude: "/prodigy-detect-documentation-gaps --project $PROJECT_NAME"
```

**Source**: workflows/book-docs-drift.yml:24-34

This ensures work items are always current and reflect the latest state of your codebase.

#### Filter and Sort Work Items

Process high-priority items first and skip low-value work.

```yaml
map:
  filter: "File.score >= 10 OR Function.unified_score.final_score >= 10"
  sort_by: "File.score DESC NULLS LAST, Function.unified_score.final_score DESC NULLS LAST"
```

**Source**: workflows/debtmap-reduce.yml:79-80

**Common patterns:**
- `filter: "severity == 'high' || severity == 'critical'"` - Process critical items only
- `sort_by: "priority DESC"` - High priority first
- `sort_by: "complexity ASC"` - Simple items first (builds momentum)

#### Configure Error Handling Policies

Use `on_item_failure: dlq` to continue processing when individual items fail.

```yaml
error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate
```

**Source**: workflows/book-docs-drift.yml:86-90

**Error handling strategies:**
- `on_item_failure: dlq` - Continue processing, collect failures in Dead Letter Queue
- `continue_on_failure: true` - Keep going when items fail
- `max_failures: 2` - Fail workflow if too many items fail (prevents cascading failures)
- `error_collection: aggregate` - Collect all errors for later analysis

#### Monitor Events and DLQ

Use Prodigy's event system and DLQ commands to track progress and handle failures:

```bash
# View events for a job
prodigy events <job_id>

# Check failed items in DLQ
prodigy dlq show <job_id>

# Retry failed items after fixing issues
prodigy dlq retry <job_id> --max-parallel 5
```

#### Use Reduce Phase for Aggregation

The reduce phase runs after all map agents complete. Use it to:
- Validate combined results
- Build summary reports
- Perform final cleanup

```yaml
reduce:
  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
      commit_required: true

  - shell: "rm -rf ${ANALYSIS_DIR}"
```

**Source**: workflows/book-docs-drift.yml:62-81

### Testing Best Practices

#### Include Test Validation Steps

Always verify changes don't break existing functionality:

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/debug-test-failures"
```

**Source**: workflows/mapreduce-example.yml:18-20

#### Test Workflows Incrementally

Start with a minimal test workflow before scaling up:

```yaml
# Minimal MapReduce workflow to test merge-to-parent functionality
name: minimal-mapreduce-test
mode: mapreduce

setup:
  - shell: |
      echo '[
        {"id": 1, "name": "item-one"},
        {"id": 2, "name": "item-two"}
      ]' > test-items.json

map:
  input: test-items.json
  json_path: $[*]
  max_parallel: 2

  agent_template:
    - shell: echo "Processed by agent: ${item.name}" > output-${item.name}.txt
    - shell: git add output-${item.name}.txt
    - shell: git commit -m "Process ${item.name}"
```

**Source**: workflows/tests/minimal-mapreduce.yml:1-50

**Testing strategy:**
1. Start with 2-3 work items
2. Use `max_parallel: 2` for easier debugging
3. Verify results before processing full dataset
4. Scale up parallelism gradually

#### Use Goal-Seeking for Automated Debugging

Let Prodigy iteratively fix test failures:

```yaml
- shell: "cargo test"
  on_failure:
    goal_seek:
      goal: "Fix all failing tests"
      claude: "/debug-test-failures"
      validate: |
        cargo test 2>&1 | grep -q "test result: ok" && echo "score: 100" || {
          passed=$(cargo test 2>&1 | grep -oP '\d+(?= passed)' | head -1)
          failed=$(cargo test 2>&1 | grep -oP '\d+(?= failed)' | head -1)
          total=$((passed + failed))
          score=$((passed * 100 / total))
          echo "score: $score"
        }
      threshold: 100
      max_attempts: 3
```

**Source**: workflows/goal-seeking-examples.yml:76-95

#### Verify Checkpoint/Resume Functionality

Test that your workflow can resume from interruption:

```bash
# Start workflow
prodigy run workflow.yml

# Interrupt it (Ctrl+C)

# Resume from checkpoint
prodigy resume <session-id>
```

All map phase progress should be preserved. See [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) for details.

### Error Handling Best Practices

#### Use on_failure for Recovery

Define recovery strategies for predictable failures:

```yaml
- shell: "cargo build --release"
  on_failure:
    claude: "/fix-build-errors"
    commit_required: true
```

#### Provide Contextual Error Information

Pass error output to Claude for better debugging:

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/debug-test ${shell.output}"
```

**Source**: workflows/mapreduce-example.yml:18-20

#### Set Reasonable Max Attempts

For iterative fixes with validation, limit attempts to prevent infinite loops:

```yaml
validate:
  claude: "/prodigy-validate-doc-fix --project $PROJECT_NAME"
  threshold: 100
  on_incomplete:
    claude: "/prodigy-complete-doc-fix --project $PROJECT_NAME"
    max_attempts: 3
    fail_workflow: false
```

**Source**: workflows/book-docs-drift.yml:49-56

**Guidelines:**
- Simple fixes: `max_attempts: 2-3`
- Complex fixes: `max_attempts: 5`
- Set `fail_workflow: false` if partial progress is acceptable

### Environment Management Best Practices

#### Use Profiles for Different Environments

Define environment-specific configurations:

```yaml
env:
  API_URL:
    default: http://localhost:3000
    staging: https://staging.api.com
    prod: https://api.com

profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"

  testing:
    NODE_ENV: test
    COVERAGE: "true"
```

**Source**: workflows/environment-example.yml:4-39

Activate with: `prodigy run workflow.yml --profile prod`

#### Mark Secrets Appropriately

Use the `secrets` block to mask sensitive values in logs:

```yaml
secrets:
  API_KEY: "${env:SECRET_API_KEY}"
```

**Source**: workflows/environment-example.yml:21-23

Secrets are automatically masked in command output, error messages, and event logs.

#### Load Environment Files

Use `env_files` to load variables from external files:

```yaml
env_files:
  - .env.production
```

**Source**: workflows/environment-example.yml:25-27

This keeps sensitive values out of version control.

#### Document Required Variables

Add comments explaining what each environment variable does:

```yaml
env:
  # Project configuration
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/book-config.json"

  # Book-specific settings
  BOOK_DIR: "book"
  MAX_PARALLEL: "3"
```

**Source**: workflows/book-docs-drift.yml:10-21

### Documentation-Specific Best Practices

#### Structure Feature Analysis First

Always run feature analysis before gap detection:

```yaml
setup:
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME"
  - claude: "/prodigy-detect-documentation-gaps --project $PROJECT_NAME"
```

This ensures gap detection knows what features exist in your codebase.

#### Use Subsection-Aware Commands

When working with multi-subsection chapters, use subsection-aware commands:

```yaml
map:
  agent_template:
    - claude: "/prodigy-analyze-subsection-drift --project $PROJECT_NAME --json '${item}'"
    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --json '${item}'"
```

**Source**: workflows/book-docs-drift.yml:42-47

These commands understand subsection scope and preserve cross-references.

#### Configure Validation Thresholds

Set appropriate quality thresholds for documentation:

```yaml
validate:
  threshold: 100  # Documentation must meet 100% quality standards
  on_incomplete:
    max_attempts: 3
    fail_workflow: false
    commit_required: true
```

**Source**: workflows/book-docs-drift.yml:52-57

**Recommended thresholds:**
- **100%**: Complete, production-ready documentation
- **95%**: Minor improvements needed
- **90%**: Good enough for initial review

#### Limit Parallel Processing

Documentation workflows are Claude-intensive. Use moderate parallelism:

```yaml
map:
  max_parallel: 3  # 3-4 agents optimal for documentation
```

**Source**: workflows/book-docs-drift.yml:59

Too many parallel agents can hit API rate limits.

#### Handle DLQ Items Appropriately

Some documentation items may fail due to missing features or unclear scope. Review DLQ items manually:

```bash
prodigy dlq show <job_id>

# Fix issues (add features, clarify scope, etc.)

prodigy dlq retry <job_id>
```

#### Verify mdBook Build

Always build the book in the reduce phase to catch broken links:

```yaml
reduce:
  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
```

**Source**: workflows/book-docs-drift.yml:63-67

### Quick Wins Checklist

Use this checklist when creating automated documentation workflows:

**Setup Phase:**
- [ ] Feature analysis command runs first
- [ ] Gap detection generates work items dynamically
- [ ] Environment variables defined for all paths and settings
- [ ] Working directory is correct

**Map Phase:**
- [ ] `max_parallel` set to 3-4 for Claude-heavy workflows
- [ ] Agent template uses subsection-aware commands
- [ ] `commit_required: true` for all fix commands
- [ ] Validation configured with appropriate thresholds

**Reduce Phase:**
- [ ] mdBook build step included
- [ ] Build errors have recovery command
- [ ] Temporary analysis files cleaned up

**Error Handling:**
- [ ] `on_item_failure: dlq` configured
- [ ] `continue_on_failure: true` set
- [ ] Reasonable `max_failures` threshold

**Testing:**
- [ ] Workflow tested with minimal dataset first
- [ ] DLQ retry tested
- [ ] Resume functionality verified

### Common Anti-Patterns to Avoid

#### Over-Parallelization

**Problem:**
```yaml
map:
  max_parallel: 50  # Way too high!
```

**Impact:** API rate limits, resource exhaustion, difficult debugging

**Solution:** Use 3-5 for Claude-heavy workflows, 4-10 for simple operations

#### Missing Error Handlers

**Problem:**
```yaml
- shell: "cargo build"
# No on_failure handler - workflow fails on first error
```

**Impact:** Workflow stops at first failure, no recovery

**Solution:**
```yaml
- shell: "cargo build"
  on_failure:
    claude: "/fix-build-errors"
```

#### Incomplete Validation

**Problem:**
```yaml
- claude: "/fix-drift"
# No validation step - no way to know if it worked
```

**Impact:** Can't verify quality, may need manual review

**Solution:**
```yaml
- claude: "/fix-drift"
  validate:
    claude: "/validate-fix"
    threshold: 95
```

#### Hardcoded Paths

**Problem:**
```yaml
- shell: "cd /Users/alice/projects/myapp && make"
```

**Impact:** Workflow breaks on other machines

**Solution:**
```yaml
env:
  PROJECT_DIR: "."

commands:
  - shell: "make"
    working_dir: "${PROJECT_DIR}"
```

#### Ignoring DLQ Items

**Problem:** Running workflow, seeing failures, never checking DLQ

**Impact:** Work items silently fail, issues accumulate

**Solution:** Regularly check and retry DLQ:
```bash
prodigy dlq show <job_id>
prodigy dlq retry <job_id>
```

### Related Documentation

- [Understanding the Workflow](understanding-the-workflow.md) - How the automated documentation system works
- [Troubleshooting](troubleshooting.md) - Common issues and solutions
- [Advanced Configuration](advanced-configuration.md) - Fine-tuning workflow behavior
- [GitHub Actions Integration](github-actions-integration.md) - Running workflows in CI/CD
- [Environment Variables](../configuration/environment-variables.md) - Complete environment configuration guide
- [Goal-Seeking Operations](../advanced/goal-seeking-operations.md) - Iterative improvement workflows
- [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) - Recovery from interruptions
