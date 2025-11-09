## Setup Phase (Advanced)

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

  # Timeout for entire setup phase (seconds, can use environment variables)
  timeout: 300  # or timeout: "$SETUP_TIMEOUT"

  # Capture outputs from setup commands
  capture_outputs:
    # Simple format (shorthand - captures stdout with defaults)
    file_count: 0  # Capture stdout from command at index 0

    # Detailed CaptureConfig format
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
- `timeout` - Timeout for entire setup phase in seconds
- `capture_outputs` - Map of variable names to command outputs (Simple or Detailed format)

