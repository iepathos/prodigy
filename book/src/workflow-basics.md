# Workflow Basics

Prodigy workflows define automated sequences of commands that execute in an isolated git worktree. This chapter covers the fundamentals of standard workflows: structure, command types, execution model, and basic configuration.

## Why Workflows?

Workflows provide:
- **Automation**: Execute complex multi-step processes consistently
- **Isolation**: Changes happen in a dedicated worktree, keeping your main repository clean
- **Auditability**: Every command creates a git commit with full history
- **Reproducibility**: Define once, run anywhere with the same results

## Workflow Structure

Prodigy supports three workflow configuration formats, from simplest to most comprehensive.

### Simple Array Format

The quickest way to create a workflow - just list your commands:

```yaml
# simple-workflow.yml
- shell: echo "Step 1: Analyze code"
- shell: cargo check
- shell: cargo test
- shell: echo "All checks passed!"
```

This format is perfect for:
- Quick automation tasks
- Learning Prodigy basics
- Scripts without environment configuration

### Full Configuration Format

For production workflows with environment variables, secrets, and custom merge logic:

```yaml
# production-workflow.yml
name: code-quality-check
mode: standard

env:
  PROJECT_NAME: myproject
  LOG_LEVEL: info

secrets:
  # Simple string format - value is retrieved from environment variable
  API_KEY: "${env:SECRET_API_KEY}"

commands:
  - shell: echo "Running $PROJECT_NAME quality checks"
  - shell: cargo clippy -- -D warnings
  - shell: cargo test --all
  - shell: cargo fmt --check

merge:
  commands:
    - shell: cargo build --release
    - shell: echo "Merging to ${merge.target_branch}"
  timeout: 300
```

This format provides:
- Named workflows for clarity
- Global environment variables
- Secret masking in logs
- Custom merge validation
- Profile support for different environments

### Legacy Commands Field Format

Prodigy maintains backward compatibility with the original format:

```yaml
# legacy-workflow.yml
commands:
  - shell: cargo check
  - shell: cargo test
```

**Note**: This format is deprecated. Use the simple array or full configuration format instead.

## Command Types

Prodigy supports several command types, each designed for specific use cases.

### Command Type Reference

| Command Type | Purpose | Key Fields |
|--------------|---------|------------|
| `shell:` | Execute shell commands | Command string |
| `claude:` | Execute Claude Code commands | Command string (starts with `/`) |
| `write_file:` | Create/update files with formatting | `path`, `content`, `format` |
| `goal_seek:` | Iterative refinement to threshold | `goal`, `validate`, `threshold`, `max_attempts` |
| `foreach:` | Iterate over items | `foreach`, `do`, `parallel`, `continue_on_error` |

**Source**: src/config/command.rs:319-380

**Note**: The `validate` field is used within `goal_seek` commands, not as a standalone command type. See [Validation in Goal-Seek](#validation-in-goal-seek) for details.

### Shell Commands

Execute shell commands with full access to the worktree environment:

```yaml
- shell: echo "Hello from Prodigy"
- shell: cargo build --release
- shell: |
    if [ -f README.md ]; then
      echo "Documentation exists"
    else
      echo "Missing documentation"
    fi
```

Shell commands support:
- Single-line commands
- Multi-line scripts (using `|` for literal blocks)
- Variable interpolation
- Full bash syntax and features

### Claude Commands

Execute Claude Code commands via the CLI:

```yaml
- claude: /analyze-code
- claude: "/fix-issue '${item.path}'"
- claude: /generate-docs --format markdown
```

Claude commands:
- Must start with `/` (slash command syntax)
- Can reference workflow variables
- Stream output in verbose mode
- Create detailed JSON logs for debugging

See [Command Types](./workflow-basics/command-types.md) for detailed information on all available command types, including Claude commands.

### Write File Commands

Create or update files with automatic formatting and validation:

```yaml
- write_file:
    path: ".prodigy/map-results.json"
    content: "${map.results}"
    format: json
    create_dirs: true
```

**Source**: src/config/command.rs:280-298

Write file commands support:
- **path**: File path with variable interpolation
- **content**: File content with variable interpolation
- **format**: Output format - `text` (default), `json` (with validation), or `yaml` (with formatting)
- **mode**: File permissions in octal (default: `"0644"`)
- **create_dirs**: Create parent directories if needed (default: `false`)

**Real-world example** from workflows/debtmap-reduce.yml:105:
```yaml
reduce:
  - write_file:
      path: ".prodigy/map-results.json"
      content: "${map.results}"
      format: json
      create_dirs: true
```

Use cases:
- Generate configuration files from templates
- Export workflow results to JSON/YAML
- Create reports with proper formatting
- Write data with validation

### Validation in Goal-Seek

The `validate` field is used within `goal_seek` commands to verify implementation completeness. It's not a standalone command type, but rather a configuration field that defines how to validate progress toward a goal.

**Source**: src/cook/workflow/validation.rs:12-27

**Real-world example** from workflows/implement.yml:8:
```yaml
- goal_seek:
    goal: "Implement specification requirements"
    claude: "/implement-spec $ARG"
    validate:
      claude: "/prodigy-validate-spec $ARG --output .prodigy/validation-result.json"
      result_file: ".prodigy/validation-result.json"
      threshold: 100
    max_attempts: 5
```

The `validate` field supports:
- **shell**: Shell command for validation
- **claude**: Claude command for validation
- **commands**: Multi-step validation sequence
- **result_file**: JSON file with validation results containing a score
- **threshold**: Minimum score to pass (0-100)

**Note**: The `validate` field is specifically designed for use within `goal_seek` commands, not as a standalone command type. See [Goal-Seek Commands](#goal-seek-commands) below for more details.

Use cases:
- Specification coverage checking
- Documentation quality validation
- Feature completeness verification
- Test coverage improvement tracking

### Deprecated and Internal Commands

**Test Command (Deprecated)**

The `test:` command type is deprecated and will show a warning when used. Use `shell:` with `on_failure:` instead:

```yaml
# Old (deprecated):
- test: cargo test --all
  on_failure:
    debug_command: cargo test --verbose

# New (preferred):
- shell: cargo test --all
  on_failure:
    - shell: cargo test --verbose
```

**Source**: src/config/command.rs:447-462

**Analyze Command (Internal)**

The `analyze:` command type is used internally by Prodigy and is not intended for direct use in workflow files.

**Source**: src/config/command.rs:330-332

### Goal-Seek Commands

Iteratively refine code until validation score meets threshold:

```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/improve-test-coverage"
    validate: "cargo tarpaulin --print-summary 2>/dev/null | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
```

**Source**: src/cook/goal_seek/mod.rs:15-41

Goal-seek commands support:
- **goal**: Human-readable description of the objective
- **claude**: Claude command for refinement (optional, use this OR shell)
- **shell**: Shell command for refinement (optional, use this OR claude)
- **validate**: Command that outputs `score: N` where N is 0-100
- **threshold**: Minimum score to succeed (0-100)
- **max_attempts**: Maximum refinement iterations
- **timeout_seconds**: Optional timeout for entire operation
- **fail_on_incomplete**: Whether to fail workflow if threshold not met

**Real-world example** from workflows/goal-seeking-examples.yml:9:
```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/improve-test-coverage"
    validate: "cargo tarpaulin --print-summary 2>/dev/null | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
```

**How it works:**
1. Run validation command to get current score
2. If score < threshold, run refinement command (claude or shell)
3. Re-run validation to check if score improved
4. Repeat until threshold met or max_attempts reached

Goal-seek enables:
- Test coverage improvement
- Performance optimization
- Code quality enforcement
- Iterative refinement workflows

### Foreach Commands

Iterate over items with nested command sequences:

```yaml
- foreach: ["src/main.rs", "src/lib.rs", "src/utils.rs"]
  do:
    - shell: echo "Processing ${item}"
    - shell: rustfmt ${item}
    - shell: cargo check
  parallel: false
  continue_on_error: false
  max_items: 10
```

**Source**: src/config/command.rs:191-211

Foreach commands support:
- **foreach**: Input source - either a list `["item1", "item2"]` or a command that outputs items
- **do**: Commands to execute for each item (item available as `${item}`)
- **parallel**: Execute items in parallel - `false`, `true`, or number of parallel jobs (default: `false`)
- **continue_on_error**: Continue processing remaining items if one fails (default: `false`)
- **max_items**: Limit number of items to process (optional)

**Implementation Note**: While you use `do:` in YAML, the internal representation uses `do_block` to avoid Rust keyword conflicts. This is transparent to workflow authors - always use `do:` in your YAML files.

**Example with command input:**
```yaml
- foreach: "ls src/*.rs"
  do:
    - shell: rustfmt ${item}
    - shell: cargo check
  parallel: 4
  continue_on_error: true
```

**Example with parallel execution:**
```yaml
- foreach: ["test1.txt", "test2.txt", "test3.txt"]
  do:
    - shell: process ${item}
  parallel: 3  # Process 3 items concurrently
```

Foreach enables:
- Batch file processing
- Parallel execution of similar tasks
- Dynamic workflow generation
- Multi-target operations

## Command Syntax Formats

Commands can be specified in three formats, progressing from simple to complex.

### Simple String Format

The most concise syntax for basic commands:

```yaml
- shell: echo "Hello World"
- claude: /analyze-code
```

Prodigy parses these strings into structured commands internally.

### Structured Format (WorkflowStep)

Add configuration like timeouts, output capture, and error handlers:

```yaml
- shell: cargo test
  timeout: 300
  capture_output: test_results
  on_failure:
    - shell: cargo test --verbose
```

This format provides:
- Per-step timeout control
- Output capture and variable binding
- Error handling configuration
- Conditional execution control

### SimpleObject Format

For commands requiring explicit argument parsing:

```yaml
- name: analyze-file
  args:
    - "${file.path}"
    - "--verbose"
  commit_required: true
```

## Sequential Execution Model

Prodigy executes commands sequentially, one at a time, with these guarantees:

**Order Preservation**: Commands execute in the exact order defined in your workflow file.

**Output Capture**: Each command's stdout, stderr, and exit code are captured and available to subsequent commands.

**State Flow**: Variables and environment configuration flow from one command to the next:

```yaml
- shell: echo "build-123"
  capture: build_id

- shell: echo "Deploying build ${build_id}"
  # Outputs: "Deploying build build-123"
```

**Commit Tracking**: By default, successful commands create git commits with descriptive messages:

```
commit abc123
Author: Prodigy Workflow
Date: 2025-01-11

Step 2: cargo test

Command: cargo test
Exit code: 0
Duration: 3.2s
```

**Failure Handling**: When a command fails:
1. Execution stops (unless `continue_on_error: true` or `on_failure` handler configured)
2. Error details captured in logs
3. Worktree preserved for debugging
4. No merge to main repository occurs

## Variable Interpolation

Prodigy supports two variable syntax formats for referencing values in commands:

**Braced Format** (`${VAR}`): Explicit and recommended for clarity:

```yaml
- shell: echo "Processing ${PROJECT_NAME} version ${VERSION}"
- shell: test -f ${item.path}
```

**Unbraced Format** (`$VAR`): Shell-style, shorter but less explicit:

```yaml
- shell: echo "Project: $PROJECT_NAME"
```

### Variable Scopes

**Workflow-level variables**: Defined in `env` block, available to all commands:

```yaml
env:
  PROJECT_NAME: prodigy
  VERSION: "1.0.0"

commands:
  - shell: echo "$PROJECT_NAME v$VERSION"
```

**Captured variables**: Outputs from previous commands:

```yaml
- shell: git rev-parse HEAD
  capture: commit_hash

- shell: echo "Current commit: ${commit_hash}"
```

**Built-in variables**: Provided by Prodigy for special contexts:
- MapReduce: `${item.*}`, `${map.results}`, `${map.total}`
- Merge: `${merge.source_branch}`, `${merge.target_branch}`, `${merge.worktree}`

See [Variables](./variables/index.md) for complete documentation on all available variables and interpolation syntax.

## Environment Configuration

Configure environment variables at the workflow level for consistent execution:

### Plain Environment Variables

```yaml
env:
  PROJECT_NAME: prodigy
  RUST_BACKTRACE: "1"
  LOG_LEVEL: debug

commands:
  - shell: cargo test
```

All commands inherit these variables automatically.

### Secret Variables

Mask sensitive values in logs and output:

```yaml
secrets:
  # Simple string format - retrieves from environment variable
  DATABASE_URL: "${env:DATABASE_URL}"
  # Provider-based format with explicit provider
  API_KEY:
    provider: env
    key: "SECRET_API_KEY"

commands:
  - shell: curl -H "Authorization: Bearer ${API_KEY}" https://api.example.com
```

Secret values are replaced with `***` in all logs, errors, and output.

### Environment Files

Load variables from `.env` files:

```yaml
env_files:
  - .env
  - .env.local

commands:
  - shell: echo "Using database $DATABASE_URL"
```

### Environment Profiles

Define different configurations for dev, staging, and production:

```yaml
profiles:
  dev:
    API_URL: http://localhost:3000
    DEBUG: "true"
  staging:
    API_URL: https://staging.api.example.com
    DEBUG: "false"
  prod:
    API_URL: https://api.example.com
    DEBUG: "false"

commands:
  - shell: curl $API_URL/health
```

Activate a profile:
```bash
prodigy run workflow.yml --profile prod
```

See [Environment Variables](./environment/index.md) for comprehensive coverage of environment configuration, secrets, profiles, and precedence rules.

## Step-Level Configuration

Individual commands can be configured with additional options:

### Timeout

Set maximum execution time per command (in seconds):

```yaml
- shell: cargo build --release
  timeout: 600  # 10 minutes
```

**Source**: src/config/command.rs:382-384

### Output Capture

Capture command output to variables for use in subsequent steps:

**Simple capture** - Store stdout in a variable:
```yaml
- shell: git rev-parse HEAD
  capture: commit_hash

- shell: echo "Current commit: ${commit_hash}"
```

**Advanced capture** - Configure format and streams:
```yaml
- shell: cargo test --all
  capture_output: test_results
  capture_format: json  # Parse output as JSON
  capture_streams: both  # Capture stdout and stderr (options: stdout, stderr, both)
```

**Note**: The `capture` field is shorthand for `capture_output` with default settings (plain text format, stdout only). Use `capture_output` when you need advanced configuration with `capture_format` or `capture_streams`.

**Source**: src/config/command.rs:366-396

### Output Redirection

Redirect command output to a file:

```yaml
- shell: cargo test --all
  output_file: test-results.txt
```

**Source**: src/config/command.rs:398-400

### Commit Required

Control whether the step must create a git commit:

```yaml
- shell: cargo fmt
  commit_required: false  # Don't require changes
```

**Source**: src/config/command.rs:354-356

### Conditional Execution

Execute step only if condition evaluates to true:

```yaml
- shell: cargo build --release
  when: "${tests_passed}"  # Only run if tests_passed is true
```

**Source**: src/config/command.rs:386-388

## Custom Merge Workflows

By default, Prodigy prompts to merge worktree changes back to your original branch. You can customize this process with a merge workflow:

```yaml
name: main-workflow
commands:
  - shell: cargo test
  - shell: cargo clippy

merge:
  commands:
    - shell: git fetch origin
    - shell: git merge origin/main
    - shell: cargo test  # Verify after merge
    - shell: cargo build --release
  timeout: 600
```

### Merge Variables

Special variables available in merge workflows:

- `${merge.worktree}`: Name of the worktree (e.g., `session-abc123`)
- `${merge.source_branch}`: Branch in the worktree
- `${merge.target_branch}`: Your original branch (where you were when workflow started)
- `${merge.session_id}`: Session ID for correlation

Example usage:

```yaml
merge:
  commands:
    - shell: echo "Merging ${merge.worktree} to ${merge.target_branch}"
    - shell: git diff ${merge.target_branch}..${merge.source_branch}
    - shell: cargo test --all
```

**Important**: Always merge to `${merge.target_branch}`, not a hardcoded branch name. This ensures changes merge back to wherever you started (master, feature branch, etc.).

See [Merge Workflows](./workflow-basics/merge-workflows.md) for advanced patterns and conflict resolution strategies.

## Running Workflows

### Basic Execution

Run a workflow file:

```bash
prodigy run workflow.yml
```

Prodigy will:
1. Create an isolated git worktree
2. Execute commands sequentially
3. Commit each successful step
4. Prompt to merge changes back to your branch

### With Arguments

Pass arguments to your workflow:

```bash
prodigy run workflow.yml --arg FILE=src/main.rs
```

Reference in workflow:
```yaml
- shell: rustfmt $FILE
```

### With Profiles

Activate an environment profile:

```bash
prodigy run workflow.yml --profile prod
```

### Verbose Mode

See detailed execution including Claude streaming output:

```bash
prodigy run workflow.yml -v
```

### Resume Interrupted Workflows

If execution is interrupted, resume from checkpoint:

```bash
prodigy resume
```

See [Checkpoint and Resume](./mapreduce/checkpoint-and-resume.md) for details on resuming both standard and MapReduce workflows.

## Worktree Isolation

All workflows execute in isolated git worktrees located in `~/.prodigy/worktrees/{repo-name}/`:

**Benefits:**
- Main repository remains clean during execution
- Safe experimentation without affecting your working tree
- Multiple workflows can run in parallel (different sessions)
- Full git history preserved for debugging

**Lifecycle:**
1. Workflow starts → Worktree created from current branch
2. Commands execute → Changes committed in worktree
3. Workflow completes → Prompt to merge to original branch
4. After merge → Worktree cleaned up

**Verification:**

After running a workflow, your main repository is unchanged:

```bash
git status
# nothing to commit, working tree clean
```

Changes are in the worktree:

```bash
cd ~/.prodigy/worktrees/{repo}/session-{id}/
git log  # See workflow commits
```

## Troubleshooting Common Issues

### Command Not Found

**Symptom**: `command not found: my-tool`

**Solution**: Ensure the command is available in your `PATH` or use absolute paths:

```yaml
- shell: /usr/local/bin/my-tool
```

### Variable Not Substituted

**Symptom**: Output shows `${VAR}` instead of the value

**Solution**: Ensure variable is defined before use:

```yaml
env:
  VAR: value

commands:
  - shell: echo "${VAR}"  # Correct
```

### Workflow Won't Resume

**Symptom**: `prodigy resume` reports no sessions

**Solution**: Check session state:

```bash
prodigy sessions list
```

If no sessions, the workflow completed or checkpoint wasn't saved.

### Permission Denied in Worktree

**Symptom**: Cannot write files in worktree

**Solution**: Check worktree permissions:

```bash
ls -la ~/.prodigy/worktrees/{repo}/session-{id}/
```

### Merge Conflicts

**Symptom**: Merge to main branch fails with conflicts

**Solution**: Use custom merge workflow to handle conflicts:

```yaml
merge:
  commands:
    - shell: git fetch origin
    - shell: git merge origin/main || true
    - claude: /resolve-conflicts
    - shell: git add -A
    - shell: git commit -m "Resolve merge conflicts"
```

## Best Practices

### Keep Workflows Simple

Break complex automation into multiple small workflows rather than one large workflow:

**Good:**
```yaml
# lint.yml
- shell: cargo clippy
- shell: cargo fmt --check

# test.yml
- shell: cargo test --all

# deploy.yml
- shell: cargo build --release
```

**Avoid:**
```yaml
# monolithic-workflow.yml with 50+ steps
```

### Use Descriptive Names

Name workflows and capture variables clearly:

```yaml
name: code-quality-check  # Clear purpose

commands:
  - shell: git rev-parse HEAD
    capture: current_commit  # Descriptive name
```

### Test Workflows Locally

Always test workflows locally before running in CI/CD:

```bash
prodigy run workflow.yml -v  # Verbose output for debugging
```

### Leverage Variable Capture

Capture important values for reuse and debugging:

```yaml
- shell: cargo test 2>&1 | grep -E 'test result:' | grep -oE '[0-9]+ passed'
  capture: tests_passed

- shell: echo "Passed ${tests_passed} tests"
```

### Use Timeouts

Prevent workflows from hanging indefinitely:

```yaml
- shell: cargo build --release
  timeout: 600  # 10 minutes max
```

### Clean Up Resources

Include cleanup steps at the end of workflows:

```yaml
- shell: rm -rf target/debug  # Clean build artifacts
- shell: docker-compose down  # Stop test services
```

## Next Steps

Now that you understand workflow basics, explore:

- **[MapReduce Workflows](./mapreduce/index.md)**: Parallel processing for large-scale tasks with parallel agent execution
- **[Error Handling](./error-handling.md)**: Advanced failure recovery and retry strategies
- **[Variable Capture](./variables/custom-variable-capture.md)**: Advanced techniques for working with command outputs and variables
- **[Environment Variables](./environment/index.md)**: Comprehensive environment configuration, secrets, and profiles
- **[Command Types](./workflow-basics/command-types.md)**: Detailed reference for all available command types
