# Advanced Features

## Conditional Execution

```yaml
# Based on expression
- shell: "cargo build --release"
  when: "${tests_passed}"

# On success
- shell: "cargo test"
  on_success:
    shell: "cargo bench"

# On failure
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings"
    max_attempts: 3
    fail_workflow: false

# On exit code
- shell: "cargo check"
  on_exit_code:
    0:
      shell: "echo 'Success!'"
    101:
      claude: "/fix-compilation-errors"
```

---

## Output Capture Formats

```yaml
# String (default)
- shell: "git rev-parse HEAD"
  capture: "commit_hash"
  capture_format: string

# Number
- shell: "wc -l < file.txt"
  capture: "line_count"
  capture_format: number

# JSON
- shell: "cargo metadata --format-version 1"
  capture: "metadata"
  capture_format: json

# Lines (array)
- shell: "find . -name '*.rs'"
  capture: "rust_files"
  capture_format: lines

# Boolean
- shell: "test -f README.md && echo true || echo false"
  capture: "has_readme"
  capture_format: boolean
```

---

## Nested Conditionals

```yaml
- shell: "cargo check"
  on_success:
    shell: "cargo build --release"
    on_success:
      shell: "cargo test --release"
      on_failure:
        claude: "/debug-failures '${shell.output}'"
```

---

## Timeout Configuration

```yaml
# Command-level timeout
- shell: "cargo bench"
  timeout: 600  # 10 minutes

# MapReduce agent timeout
map:
  agent_timeout_secs: 300
  timeout_config:
    default: 300
    per_command:
      "cargo test": 600
      "cargo bench": 1800
```

---

## Enhanced Retry Configuration

```yaml
# Retry with exponential backoff
- shell: "flaky-api-call.sh"
  retry:
    max_attempts: 5
    backoff:
      type: exponential
      initial: 1000  # 1 second
      multiplier: 2
      max_delay: 30000  # 30 seconds
      jitter: true
```

---

## Working Directory

```yaml
# Run command in specific directory
- shell: "npm install"
  working_dir: "/path/to/project"

- shell: "pwd"  # Will show /path/to/project
```

---

## Auto-Commit

```yaml
# Automatically commit changes if detected
- claude: "/refactor-code"
  auto_commit: true
```

---

## Output File Redirection

```yaml
# Redirect output to file
- shell: "cargo test"
  output_file: "test-results.txt"
```

---

## Modular Handlers

```yaml
# Use custom handler
- handler:
    name: "custom-validator"
    attributes:
      path: "src/"
      threshold: 80
```

---

## Step Validation

```yaml
# Validate step success after execution
- shell: "deploy.sh"
  step_validate:
    shell: "curl -f https://app.com/health"
    timeout: 30
    on_failure:
      shell: "rollback.sh"
```

---

## Advanced Exit Code Handling

```yaml
# Map exit codes to full workflow steps
- shell: "cargo check"
  on_exit_code:
    0:
      shell: "echo 'Build successful'"
    101:
      claude: "/fix-compilation-errors"
      commit_required: true
    other:
      shell: "echo 'Unexpected error'"
```
