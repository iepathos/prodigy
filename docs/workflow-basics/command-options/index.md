## Command-Level Options

All command types (`claude:`, `shell:`, `foreach:`) support additional fields for advanced control and orchestration. These options enable timeout management, output capture, error handling, conditional execution, and more.

## Documentation Pages

This section is organized into the following pages:

### [Core Options](core-options.md)

Essential command configuration options:

- **[timeout](core-options.md#timeout)** - Maximum execution time for commands
- **[id](core-options.md#id)** - Command identifier for output referencing
- **[commit_required](core-options.md#commit_required)** - Git commit validation and tracking

### [Output Capture Options](output-capture.md)

Capturing and processing command output:

- **[capture_output](output-capture.md#capture_output)** - Store command output in workflow variables
- **[capture_format](output-capture.md#capture_format)** - Parse output as string, JSON, lines, number, or boolean
- **[capture_streams](output-capture.md#capture_streams)** - Control which streams to capture (stdout, stderr, metadata)
- **[output_file](output-capture.md#output_file)** - Redirect output to a file

### [Error Handling and Conditional Execution](error-conditional.md)

Control flow and error recovery:

- **[on_failure](error-conditional.md#on_failure)** - Commands to execute when main command fails
- **[on_success](error-conditional.md#on_success)** - Commands to execute when main command succeeds
- **[when](error-conditional.md#when)** - Conditional execution based on boolean expressions

### [Advanced Options](advanced-options.md)

Additional configuration and patterns:

- **[validate](advanced-options.md#validate)** - Implementation completeness validation
- **[analysis](advanced-options.md#analysis)** - Code quality and coverage analysis
- **[env](advanced-options.md#env)** - Command-specific environment variables
- **[Option Combinations](advanced-options.md#option-combinations)** - Common patterns and best practices
- **[Troubleshooting](advanced-options.md#troubleshooting)** - Solutions for common issues

## Quick Reference

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `timeout` | `u64` | None | Max execution time in seconds |
| `id` | `String` | None | Command identifier |
| `commit_required` | `bool` | `false` | Require git commit |
| `capture_output` | `bool`/`String` | None | Capture output to variable |
| `capture_format` | `String` | `"string"` | Output parsing format |
| `capture_streams` | Object | stdout only | Streams to capture |
| `output_file` | `String` | None | Redirect to file |
| `on_failure` | Object | None | Failure handler |
| `on_success` | Object | None | Success handler |
| `when` | `String` | None | Conditional expression |
| `validate` | Object | None | Validation config |
| `analysis` | Object | None | Analysis config |
| `env` | Object | None | Environment variables |
