## Setup Phase (Advanced)

The setup phase runs once before the map phase begins, executing in the parent worktree. It's used to initialize the environment, generate work items, download data, or prepare configuration.

### Execution Context

The setup phase:
- **Runs once** before map phase begins
- **Executes in parent worktree**, providing isolation from main repository
- **Creates checkpoint** after successful completion, preserving outputs and artifacts for workflow resume
- **Outputs available** to map and reduce phases via captured variables

This isolation ensures the main repository remains untouched while setup operations prepare the environment for parallel processing.

### Common Use Cases

The setup phase is typically used for:

- **Generate work items** - Create JSON arrays of items to process in parallel
- **Initialize environment** - Install dependencies, configure tools, set up databases
- **Download data** - Fetch datasets, clone repositories, pull artifacts
- **Prepare configuration** - Generate configs, resolve templates, validate settings

### Configuration Formats

The setup phase supports two formats: simple array OR full configuration object.

```yaml
# Simple array format
setup:
  - shell: "prepare-data.sh"
  - shell: "analyze-codebase.sh"

# Full configuration format with timeout and capture
setup:
  commands:
    - shell: "prepare-data.sh"
    - shell: "analyze-codebase.sh"

  # Timeout for entire setup phase
  # Accepts numeric seconds (300) or string with env var ("${SETUP_TIMEOUT}")
  timeout: 300  # or timeout: "${SETUP_TIMEOUT}"

  # Capture outputs from setup commands
  capture_outputs:
    # Simple format (shorthand - captures stdout with defaults)
    # Use for basic stdout capture without JSON extraction
    file_count: 0  # Capture stdout from command at index 0

    # Detailed CaptureConfig format
    # Use for JSON extraction, size limits, or custom sources
    analysis_result:
      command_index: 1
      source: stdout           # stdout, stderr, both, combined
      json_path: "$.result"    # Extract JSON field
      max_size: 1048576        # Max bytes (1MB)
      default: "{}"            # Fallback if extraction fails
      multiline: preserve      # preserve, join, first_line, last_line, array
```

**Setup Phase Fields:**
- `commands` - Array of commands to execute (or use simple array format at top level)
- `timeout` - Timeout for entire setup phase in seconds (numeric or environment variable)
- `capture_outputs` - Map of variable names to command outputs (Simple or Detailed format)

### Best Practices

**Idempotent Operations:**
- Design setup commands to be safe to run multiple times
- Use conditional checks before creating resources
- Clean up stale artifacts before generating new ones

**Timeout Sizing:**
- Set generous timeouts for network operations (downloads, API calls)
- Use environment variables (`${SETUP_TIMEOUT}`) for flexibility across environments
- Consider total time for all setup commands, not individual commands

**Output Capture Patterns:**
- Use simple format (`command_index: number`) for basic text capture
- Use detailed `CaptureConfig` when extracting JSON fields or limiting size
- Always provide `default` value for robust error handling

### See Also

- [Worktree Isolation](./worktree-isolation.md) - Understanding parent worktree execution context
- [Environment Variables](./environment-variables.md) - Using env vars in timeout and commands
- [Checkpoint and Resume](./checkpoint-and-resume.md) - How setup checkpoints enable resume
- [Map Phase Configuration](./map-phase-configuration.md) - Using captured outputs in map phase

