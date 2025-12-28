# Shell-Based Filtering and Formatting

Since git context variables are provided as space-separated strings, all filtering and formatting must be done using shell commands. This section shows practical patterns for common tasks.

## Default Format (Space-Separated)

Git context variables are always formatted as space-separated strings:

```yaml
- shell: "echo ${step.files_changed}"
# Output: src/main.rs src/lib.rs tests/test.rs
```

This format works well with most shell commands:

```yaml
# Pass directly to commands
- shell: "cargo fmt ${step.files_changed}"
- shell: "git add ${workflow.files_modified}"

# Use in loops
- shell: |
    for file in ${step.files_added}; do
      echo "Processing $file"
    done
```

## Filtering by File Extension

Use `grep` to filter files by extension or pattern:

```yaml
# Only Rust files
- shell: |
    rust_files=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$')
    echo "$rust_files"
# Output:
# src/main.rs
# src/lib.rs

# Only files in src/ directory
- shell: |
    src_files=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '^src/')
    echo "$src_files"

# Multiple extensions (Rust or TOML)
- shell: |
    filtered=$(echo "${step.files_modified}" | tr ' ' '\n' | grep -E '\.(rs|toml)$')
    echo "$filtered"

# Pass filtered files to a command
- shell: |
    rust_files=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$' | tr '\n' ' ')
    if [ -n "$rust_files" ]; then
      cargo fmt $rust_files
    fi
```

## Converting to JSON Format

Use `jq` to convert space-separated files to JSON arrays:

```yaml
# Convert to JSON array
- shell: "echo ${step.files_added} | tr ' ' '\n' | jq -R | jq -s"
# Output: ["src/main.rs","src/lib.rs","tests/test.rs"]

# Filter AND convert to JSON
- shell: |
    echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$' | jq -R | jq -s
# Output: ["src/main.rs","src/lib.rs"]

# Pretty-print JSON
- shell: |
    echo "${workflow.files_modified}" | tr ' ' '\n' | jq -R | jq -s '.'
```

## Converting to Newline-Separated Format

Use `tr` to convert space-separated to newline-separated:

```yaml
# One file per line
- shell: "echo ${step.files_changed} | tr ' ' '\n'"
# Output:
# src/main.rs
# src/lib.rs
# tests/test.rs

# Useful with xargs for parallel processing
- shell: |
    echo "${workflow.files_modified}" | tr ' ' '\n' | xargs -I {} cp {} backup/

# Count files
- shell: "echo ${step.files_added} | tr ' ' '\n' | wc -l"
```

## Converting to CSV Format

Use `tr` to convert to comma-separated values:

```yaml
# Comma-separated
- shell: "echo ${step.files_added} | tr ' ' ','"
# Output: src/main.rs,src/lib.rs,tests/test.rs

# CSV with filtering
- shell: |
    echo "${step.files_changed}" | tr ' ' '\n' | grep '\.md$' | tr '\n' ',' | sed 's/,$//'
# Output: README.md,CHANGELOG.md
```

## Combining Filtering and Formatting

Practical examples combining multiple operations:

```yaml
# Get Rust files as JSON
- shell: |
    echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$' | jq -R | jq -s

# Get source files as comma-separated list
- shell: |
    echo "${workflow.files_modified}" | tr ' ' '\n' | grep '^src/' | tr '\n' ',' | sed 's/,$//'

# Count files by extension
- shell: |
    echo "${workflow.files_changed}" | tr ' ' '\n' | sed 's/.*\.//' | sort | uniq -c
# Output:
#    5 md
#    12 rs
#    3 toml
```
