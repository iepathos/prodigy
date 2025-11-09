## Parallel Iteration with Foreach

Process multiple items in parallel using the `foreach` command.

### Basic Foreach

Iterate over a list of items:

```yaml
- foreach:
    foreach: ["a", "b", "c"]
    do:
      - shell: "process ${item}"
```

### Dynamic Item Lists

Generate items from a command:

```yaml
- foreach:
    foreach: "find . -name '*.rs'"
    do:
      - shell: "rustfmt ${item}"
```

### Parallel Execution

Control parallelism with the `parallel` field. It accepts both boolean and numeric values:

**Boolean - Automatic Parallelism:**

```yaml
- foreach:
    foreach: "ls *.txt"
    parallel: true  # Use all available cores
    do:
      - shell: "analyze ${item}"
```

**Number - Explicit Concurrency Limit:**

```yaml
- foreach:
    foreach: "ls *.txt"
    parallel: 5  # Process 5 items concurrently
    do:
      - shell: "analyze ${item}"
```

Use `true` for automatic parallelism based on available resources, or specify a number to limit concurrent execution.

### Error Handling

Continue processing remaining items on failure:

```yaml
- foreach:
    foreach: ["test1", "test2", "test3"]
    continue_on_error: true
    do:
      - shell: "run-test ${item}"
```

### Limiting Items

Process only a subset of items:

```yaml
- foreach:
    foreach: "find . -name '*.log'"
    max_items: 10  # Process first 10 items only
    do:
      - shell: "compress ${item}"
```

### Nested Commands

Each item can execute multiple commands:

```yaml
- foreach:
    foreach: "cargo metadata --format-version 1 | jq -r '.packages[].name'"
    do:
      - shell: "cargo build -p ${item}"
      - shell: "cargo test -p ${item}"
      - shell: "cargo doc -p ${item}"
```

### Practical Use Cases

**Process Multiple Directories:**
```yaml
- foreach:
    foreach: ["frontend", "backend", "shared"]
    parallel: 3
    do:
      - shell: "cd ${item} && npm install"
      - shell: "cd ${item} && npm test"
```

**Batch File Processing:**
```yaml
- foreach:
    foreach: "find src -name '*.rs'"
    parallel: 10
    continue_on_error: true
    do:
      - shell: "rustfmt ${item}"
      - shell: "cargo clippy --manifest-path=${item}"
```
