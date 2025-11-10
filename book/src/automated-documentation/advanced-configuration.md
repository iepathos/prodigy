## Advanced Configuration

This subsection covers advanced configuration topics for optimizing and customizing your automated documentation workflows. These configurations enable fine-tuning of performance, security, and behavior for documentation generation at scale.

### Configuration Files and Locations

Prodigy supports configuration at multiple levels with a clear precedence chain:

**Configuration File Locations** (Source: src/config/mod.rs:39-86):

1. **Global Configuration**: `~/.prodigy/config.toml`
   - Applies across all projects
   - Contains defaults for editor, log level, API keys, and global settings

2. **Project Configuration**: `.prodigy/config.toml`
   - Project-specific overrides
   - Contains project name, description, spec directory, and custom variables

3. **Workflow Environment**: `env:` block in workflow YAML files
   - Workflow-specific configuration
   - Defines variables, secrets, and profiles for the workflow

**Configuration Precedence Chain**:
```
Step env > Workflow profile > Workflow env > Project config > Global config > System env
```

Higher-priority configurations override lower-priority ones. For example, a step-level environment variable will override the same variable defined in the workflow env block.

### Environment Variables

Environment variables parameterize workflows and can be defined in the `env:` block at the workflow root (Source: src/cook/environment/config.rs:12-36).

**Environment Configuration Structure** (Source: src/cook/environment/config.rs:12-36):

```yaml
env:
  # Plain variables
  PROJECT_NAME: "Prodigy"
  VERSION: "1.0.0"
  BOOK_DIR: "book"

  # Secret variables (masked in logs)
  API_KEY:
    secret: true
    value: "sk-abc123"

  # Profile-specific variables
  DATABASE_URL:
    default: "postgres://localhost/dev"
    prod: "postgres://prod-server/db"
```

**Variable Interpolation Syntax**:
- `$VAR` - Simple variable reference (shell-style)
- `${VAR}` - Bracketed reference for clarity

**Secret Masking** (Source: src/cook/environment/mod.rs:45-61):

Variables marked with `secret: true` are automatically masked in command output logs, error messages, event logs, and checkpoint files. The masking utility replaces secret values with `***MASKED***`.

**Profile Support**:

Activate different environment profiles using the `--profile` flag:
```bash
prodigy run workflow.yml --profile prod
```

**Real-World Example** (Source: workflows/book-docs-drift.yml:8-21):

```yaml
env:
  # Project configuration
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/book-config.json"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"

  # Book-specific settings
  BOOK_DIR: "book"
  ANALYSIS_DIR: ".prodigy/book-analysis"
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"

  # Workflow settings
  MAX_PARALLEL: "3"
```

These variables are referenced throughout the workflow using `$VARIABLE_NAME` or `${VARIABLE_NAME}` syntax.

### MapReduce Performance Tuning

For documentation workflows using MapReduce, several configuration options control parallelism and resource usage (Source: src/config/mapreduce.rs:238-241, 276-278).

**max_parallel Configuration** (Source: src/config/mapreduce.rs:238-241):

Controls the number of concurrent documentation agents processing chapters/subsections in parallel:

```yaml
map:
  input: "${ANALYSIS_DIR}/flattened-items.json"
  json_path: "$[*]"

  agent_template:
    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --json '${item}'"

  max_parallel: ${MAX_PARALLEL}  # Default: 10
```

**Performance Trade-offs**:
- **Higher parallelism** (10+): Faster completion, higher resource usage (CPU, memory, disk I/O)
- **Lower parallelism** (3-5): More conservative resource usage, longer total execution time
- **Balanced approach** (5-7): Good for most documentation workflows

The `book-docs-drift.yml` workflow uses `MAX_PARALLEL: 3` for balanced performance and resource management.

**Timeout Configuration**:

While not explicitly shown in the MapReduce configuration, agent timeouts can be configured for long-running documentation tasks:
- `agent_timeout_secs`: Maximum time allowed for each map agent
- `setup_timeout`: Maximum time for feature analysis phase
- `reduce_timeout`: Maximum time for book build phase

### Book Configuration

The `.prodigy/book-config.json` file defines book-specific analysis and generation settings (Source: .prodigy/book-config.json:1-220).

**Book Configuration Structure** (Source: .prodigy/book-config.json):

```json
{
  "project_name": "Prodigy",
  "project_type": "cli_tool",
  "book_dir": "book",
  "book_src": "book/src",
  "book_build_dir": "book/book",
  "analysis_targets": [
    {
      "area": "configuration",
      "source_files": [
        "src/config/mod.rs",
        "src/config/settings.rs"
      ],
      "feature_categories": [
        "file_locations",
        "precedence",
        "claude_settings",
        "storage_settings"
      ]
    }
  ],
  "chapter_file": "workflows/data/prodigy-chapters.json",
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true
  }
}
```

**Key Fields**:
- `analysis_targets`: Defines codebase areas to analyze for feature extraction
- `source_files`: Source code files to scan for each area
- `feature_categories`: Categories of features to document for each area
- `custom_analysis`: Options for including examples, best practices, and troubleshooting sections

**Adapting for Different Project Types**:
- Rust: Use `src/**/*.rs` patterns
- Python: Use `src/**/*.py` or package structure
- JavaScript: Use `src/**/*.js`, `src/**/*.ts`

### Claude-Specific Configuration

Control Claude's behavior during documentation generation with environment variables and verbosity flags.

**Claude Streaming Configuration**:

- `PRODIGY_CLAUDE_STREAMING=false`: Disable JSON streaming output (useful in CI/CD)
- `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true`: Force streaming output regardless of verbosity
- `-v` flag: Enable verbose mode to see Claude streaming output for debugging

**Claude Log Locations**:

Claude creates detailed JSON log files for each command execution at:
```
~/.local/state/claude/logs/session-{session_id}.json
```

**Analyzing Claude Logs for Debugging**:

```bash
# View complete Claude interaction
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages'

# Check tool invocations
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages[].content[] | select(.type == "tool_use")'

# Analyze token usage
cat ~/.local/state/claude/logs/session-abc123.json | jq '.usage'
```

Use `-v` flag during workflow execution to see real-time streaming output from Claude for troubleshooting failed documentation agents.

### Error Handling Configuration

Configure how documentation workflows handle failures and errors (Source: workflows/book-docs-drift.yml:85-90).

**Error Policy Configuration** (Source: workflows/book-docs-drift.yml:85-90):

```yaml
error_policy:
  on_item_failure: dlq            # Send failed items to Dead Letter Queue
  continue_on_failure: true       # Continue processing other items
  max_failures: 2                 # Stop workflow after 2 failures
  error_collection: aggregate     # Aggregate errors for reporting
```

**Error Policy Options**:
- `on_item_failure`: `dlq` (Dead Letter Queue), `fail` (stop immediately), `skip` (continue)
- `continue_on_failure`: Whether to continue processing remaining items after a failure
- `max_failures`: Maximum number of failures before stopping the entire workflow
- `error_collection`: How to collect and report errors (`aggregate`, `individual`)

**Dead Letter Queue (DLQ) Usage**:

Failed documentation items are stored in `~/.prodigy/dlq/{repo_name}/{job_id}/` for review and retry:

```bash
# View failed items
prodigy dlq show <job_id>

# Retry all failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 5
```

**Retry Strategies**:

While not shown in the example workflow, retry configuration can be added to commands:
- Backoff strategies: exponential, linear, fibonacci
- Max retry attempts
- Retry budget limits

### Storage and Worktree Configuration

Prodigy uses global storage for centralized state management and git worktrees for isolation.

**Global Storage Locations**:
- Events: `~/.prodigy/events/{repo_name}/{job_id}/`
- DLQ: `~/.prodigy/dlq/{repo_name}/{job_id}/`
- State: `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`
- Worktrees: `~/.prodigy/worktrees/{repo_name}/`

**Repository Grouping**:

All storage is grouped by repository name, enabling:
- Cross-worktree event aggregation
- Persistent state across worktree cleanup
- Centralized monitoring of all jobs for a repository

**Cleanup Policies**:
- **Automatic cleanup on success**: Worktrees are removed after successful agent completion
- **Orphan registry on failure**: Failed worktrees are registered in `~/.prodigy/orphaned_worktrees/{repo_name}/{job_id}.json`

**Cleaning Orphaned Worktrees**:

```bash
# List orphaned worktrees
prodigy worktree clean-orphaned <job_id>

# Clean with confirmation
prodigy worktree clean-orphaned <job_id> --force
```

### Validation Configuration

Configure quality gates and validation for documentation generation (Source: workflows/book-docs-drift.yml:49-57).

**Validation Configuration** (Source: workflows/book-docs-drift.yml:49-57):

```yaml
validate:
  claude: "/prodigy-validate-doc-fix --project $PROJECT_NAME --json '${item}' --output .prodigy/validation-result.json"
  result_file: ".prodigy/validation-result.json"
  threshold: 100  # Documentation must meet 100% quality standards
  on_incomplete:
    claude: "/prodigy-complete-doc-fix --project $PROJECT_NAME --json '${item}' --gaps ${validation.gaps}"
    max_attempts: 3
    fail_workflow: false  # Continue even if we can't reach 100%
    commit_required: true
```

**Validation Options**:
- `threshold`: Completion percentage required to pass (0-100)
- `result_file`: File where validation results are written
- `on_incomplete`: Handler to execute when validation threshold is not met
- `max_attempts`: Maximum attempts to complete validation
- `fail_workflow`: Whether to fail the entire workflow if validation never passes

**Quality Gates**:

The validation system ensures:
- All critical drift issues are addressed
- Documentation meets minimum content requirements
- Examples are grounded in actual codebase
- Links are valid and point to existing files

### Configuration Checklist for Optimizing Documentation Workflows

**Performance Optimization**:
- [ ] Set `MAX_PARALLEL` based on available CPU cores (recommend: cores / 2)
- [ ] Configure agent timeouts appropriate for documentation complexity
- [ ] Use global storage for centralized state management

**Security**:
- [ ] Mark API keys and sensitive data as secrets (`secret: true`)
- [ ] Use profiles to separate development and production credentials
- [ ] Enable secret masking for logs and error output

**Quality Control**:
- [ ] Set validation threshold to 100% for production documentation
- [ ] Configure `on_incomplete` handlers to automatically fix validation failures
- [ ] Enable `error_policy.on_item_failure: dlq` for failed item recovery

**Resource Management**:
- [ ] Configure cleanup policies for worktrees
- [ ] Set `max_failures` to prevent runaway workflows
- [ ] Use `continue_on_failure: true` to maximize successful documentation coverage

**Debugging**:
- [ ] Enable Claude streaming in development (`-v` flag or `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true`)
- [ ] Configure verbose logging for troubleshooting
- [ ] Preserve Claude JSON logs for post-mortem analysis

### Troubleshooting Common Configuration Issues

**Issue: Documentation workflow is too slow**
- Solution: Increase `MAX_PARALLEL` value, but monitor resource usage
- Check: CPU and memory utilization during workflow execution

**Issue: Out of memory errors during MapReduce**
- Solution: Decrease `max_parallel` to reduce concurrent agent count
- Check: Each agent may load large amounts of documentation into context

**Issue: Secrets appearing in logs**
- Solution: Ensure secrets are marked with `secret: true` in environment config
- Check: Review event logs and Claude logs for masked values

**Issue: Validation always failing at 100% threshold**
- Solution: Review validation command output to identify quality gaps
- Check: Use `on_incomplete` handler with `max_attempts` to iteratively improve

**Issue: Orphaned worktrees consuming disk space**
- Solution: Run `prodigy worktree clean-orphaned <job_id>` regularly
- Check: Monitor `~/.prodigy/worktrees/` directory size

### See Also

- [Understanding the Workflow](understanding-the-workflow.md) - Overview of documentation workflow phases
- [Troubleshooting](troubleshooting.md) - Common issues and solutions
- [Quick Start](quick-start.md) - Getting started with automated documentation
- [Environment Variables](../configuration/environment-variables.md) - Detailed environment variable reference
- [Configuration Precedence Rules](../configuration/configuration-precedence-rules.md) - How configuration values are resolved
