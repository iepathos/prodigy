## Best Practices

Guidelines for effective workflow organization, MapReduce design, and environment management in Prodigy.

> **Implementation Status Note**: Some composition features (imports, extends, templates) are implemented in the codebase but not yet integrated with the CLI. This document focuses on features available in current releases.

### When to Use Each Feature

**MapReduce Workflows** - Use when:
- Processing multiple items in parallel (lint checks, test files, code analysis)
- Distributing work across isolated git worktrees
- Need automatic retry and DLQ for failed items
- Want checkpoint-based resume for long-running operations
- See: [MapReduce chapter](../mapreduce/index.md)

**Sub-Workflows** (workflows field) - Use when:
- Breaking complex workflows into smaller, reusable pieces
- Running independent validation/test workflows
- Isolating different workflow stages
- Sharing workflow logic between projects
- Source: src/cook/workflow/composition/sub_workflow.rs:14-46

**Environment Variables** - Use when:
- Parameterizing workflows for different environments (dev/staging/prod)
- Managing secrets and sensitive configuration
- Configuring per-command overrides
- Using profiles for different deployment contexts
- See: [Environment chapter](../environment/index.md)

**Goal-Seeking Operations** - Use when:
- Iteratively refining code until tests pass
- Fixing lint/clippy errors automatically
- Improving code quality metrics
- Need validation-driven refinement
- See: [Goal-Seeking chapter](../advanced/goal-seeking-operations.md)

### Workflow Organization

**Real-World Directory Structure** (from Prodigy itself):
```
workflows/
├── book-docs-drift.yml           # MapReduce: documentation maintenance
├── debtmap-reduce.yml            # MapReduce: technical debt analysis
├── documentation-maintain.yml    # MapReduce: multi-chapter updates
├── fix-files-mapreduce.yml       # MapReduce: batch file processing
├── implement-with-tests.yml      # Standard: TDD workflow
├── coverage.yml                  # Standard: test coverage check
├── environment-example.yml       # Example: env var configuration
├── mapreduce-env-example.yml     # Example: MapReduce with env vars
├── data/                         # Work item JSON files
│   ├── chapters.json
│   ├── test-files.json
│   └── components.json
└── tests/                        # Test workflows
    └── minimal-mapreduce.yml
```

**Organization Patterns:**

1. **MapReduce Workflows** - Named with clear purpose, store work items in `data/`
2. **Standard Workflows** - Single-file workflows for linear processes
3. **Environment Examples** - Demonstrate environment variable usage
4. **Test Workflows** - Minimal examples for testing features

### Environment Variable Naming Conventions

**Good names** (from workflows/mapreduce-env-example.yml:7-20):
```yaml
env:
  # Configuration
  PROJECT_NAME: "example-project"
  PROJECT_CONFIG: "config.yml"
  FEATURES_PATH: "features"

  # Output settings
  OUTPUT_DIR: "output"
  REPORT_FORMAT: "json"

  # Workflow behavior
  MAX_RETRIES: "3"
  TIMEOUT_SECONDS: "300"
  DEBUG_MODE: "false"
```

**Avoid:**
```yaml
env:
  ENV: "prod"          # Too brief, conflicts with common shell var
  COUNT: "3"           # Unclear what it counts
  FLAG: "true"         # Doesn't indicate purpose
  PATH: "/usr/bin"     # Overwrites critical shell variable
```

**Best Practices:**
- Use UPPER_CASE for environment variables
- Prefix project-specific vars (e.g., `MYAPP_*`)
- Be descriptive: `DEPLOYMENT_ENVIRONMENT` not `ENV`
- Avoid common shell variable names (PATH, HOME, USER)
- See: [Environment Variables chapter](../environment/index.md)

### MapReduce Work Item Design

**Granularity Best Practices:**

Good granularity:
```json
{
  "items": [
    {"file": "src/main.rs", "task": "lint"},
    {"file": "src/lib.rs", "task": "lint"},
    {"file": "tests/integration.rs", "task": "lint"}
  ]
}
```

Too coarse (loses parallelism):
```json
{
  "items": [
    {"files": ["src/**/*.rs"], "task": "lint-all"}
  ]
}
```

Too fine (overhead dominates):
```json
{
  "items": [
    {"line": 1, "file": "src/main.rs", "task": "lint-line"},
    {"line": 2, "file": "src/main.rs", "task": "lint-line"}
  ]
}
```

**Idempotency:**
- Design work items so they can be retried safely
- Avoid operations that fail on re-execution (file creation, append operations)
- Use `git add` not `git commit` in agents (reduce phase commits)
- See: [MapReduce chapter](../mapreduce/index.md)

**Performance Considerations:**

**Parallel Execution:**
- Set `max_parallel` based on available resources (default: 5)
- For I/O-bound tasks: Higher parallelism (10-20)
- For CPU-bound tasks: Match CPU count
- For API calls: Respect rate limits
- Source: workflows/book-docs-drift.yml:59 uses `max_parallel: 3`

**Checkpoint Frequency:**
- Checkpoints save after configurable number of items
- More frequent = faster resume but more I/O overhead
- Less frequent = slower resume but better performance
- See: [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md)

### Testing and Validation

**Validate Workflow Syntax:**
```bash
# Validate YAML format and detect issues
prodigy validate workflow.yml

# Dry-run without executing
prodigy run workflow.yml --dry-run
```

**Test with Different Environments:**
```bash
# Test with development profile
prodigy run workflow.yml --profile development

# Test with production profile
prodigy run workflow.yml --profile production
```

**Test MapReduce with Minimal Items:**
```bash
# Create test work items file
echo '{"items": [{"id": "test1"}, {"id": "test2"}]}' > test-items.json

# Run MapReduce with minimal parallelism
prodigy run workflow.yml  # Uses max_parallel from workflow
```

### Debugging Workflows

**Verbose Output Levels:**
```bash
# Standard output (clean, progress only)
prodigy run workflow.yml

# Verbose: Show Claude streaming output
prodigy run workflow.yml -v

# Debug: Show debug logs
prodigy run workflow.yml -vv

# Trace: Show trace-level logs
prodigy run workflow.yml -vvv
```

**View Execution Logs:**
```bash
# View Claude JSON logs for debugging
prodigy logs --latest

# View with summary
prodigy logs --latest --summary

# Follow log in real-time
prodigy logs --latest --tail
```

**Inspect MapReduce Job State:**
```bash
# View job progress
prodigy progress <job_id>

# View job events
prodigy events <job_id>

# Check failed items
prodigy dlq list <job_id>

# Analyze failure patterns
prodigy dlq analyze <job_id>
```

### Security Best Practices

**Secrets Management:**

**DO: Use environment variables and secret masking** (from workflows/mapreduce-env-example.yml:22-26):
```yaml
env:
  PROJECT_NAME: "example-project"

secrets:
  API_TOKEN:
    provider: env
    key: "GITHUB_TOKEN"  # Reads from environment, masked in logs
```

**DON'T: Store secrets in workflow files:**
```yaml
env:
  API_KEY: "sk-abc123"  # Visible in version control!
```

**Pass secrets via environment:**
```bash
# Set in shell
export GITHUB_TOKEN="ghp_abc123"

# Run workflow (secret is masked in logs)
prodigy run workflow.yml
```

**Environment Profiles for Security:**
```yaml
profiles:
  development:
    API_URL: "http://localhost:3000"  # Safe for dev

  production:
    API_URL: "https://api.prod.com"   # Protected endpoint
```

See: [Secrets Management](../environment/secrets-management.md)

### DLQ and Error Recovery

**When MapReduce Items Fail:**

1. **Check the DLQ:**
```bash
# List failed items
prodigy dlq list <job_id>

# Inspect specific failure
prodigy dlq inspect <job_id> <item_id>

# View failure patterns
prodigy dlq analyze <job_id>
```

2. **Fix the Root Cause:**
- Update workflow to handle edge cases
- Fix Claude command logic
- Adjust work item format

3. **Retry Failed Items:**
```bash
# Retry all failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to preview
prodigy dlq retry <job_id> --dry-run
```

See: [Dead Letter Queue chapter](../mapreduce/dead-letter-queue-dlq.md)

### Workflow Documentation

**Document Workflow Purpose:**
```yaml
# Book Documentation Drift Detection
# Purpose: Analyzes codebase features and updates documentation automatically
# Runs: On-demand or via CI when features change
# Duration: ~30 minutes for full book
# Output: Updated markdown files in book/src/

name: prodigy-book-docs-drift-detection
mode: mapreduce

env:
  PROJECT_NAME: "Prodigy"
  BOOK_DIR: "book"
```

**Include Usage Examples:**
```yaml
# Run with default settings:
#   prodigy run book-docs-drift.yml
#
# Run with custom parallel limit:
#   prodigy run book-docs-drift.yml
#   (Edit MAX_PARALLEL in env section)
#
# Resume interrupted run:
#   prodigy resume <session_id>
```

**Document Work Item Format:**
```yaml
# Work items structure (for future contributors):
# {
#   "items": [
#     {
#       "id": "chapter-id",
#       "file": "book/src/chapter.md",
#       "features": ["feature1", "feature2"],
#       "validation": "Check completeness"
#     }
#   ]
# }
```

### Workflow Maintenance

**Version Control:**
- Commit workflows to git
- Use descriptive commit messages for workflow changes
- Tag workflow versions for major changes
- Review workflow changes in PRs

**Regular Updates:**
- Keep work item JSON files up to date
- Update environment variable defaults
- Review and update max_parallel settings
- Clean up obsolete workflows

**Monitoring:**
- Check DLQ regularly for patterns
- Monitor workflow execution times
- Review checkpoint sizes
- Track resource usage (parallelism impact)
