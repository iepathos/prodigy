# Workflow Basics

This chapter covers the fundamentals of creating Prodigy workflows. You'll learn about workflow structure, basic commands, and configuration options.

## Overview

Prodigy workflows are YAML files that define a sequence of commands to execute. They can be as simple as a list of shell commands or as complex as parallel MapReduce jobs.

**Two Main Workflow Types:**
- **Standard Workflows**: Sequential command execution (covered here)
- **MapReduce Workflows**: Parallel processing with map/reduce phases (see [MapReduce chapter](../mapreduce/index.md))

## Simple Workflows

The simplest workflow is just an array of commands:

```yaml
# Simple array format - just list your commands
- shell: "echo 'Starting workflow...'"
- claude: "/prodigy-analyze"
- shell: "cargo test"
```

This executes each command sequentially. No additional configuration needed.


## Additional Topics

See also:
- [Full Workflow Structure](full-workflow-structure.md)
- [Available Fields](available-fields.md)
- [Command Types](command-types.md)
- [Command-Level Options](command-level-options.md)
- [Environment Configuration](environment-configuration.md)
- [Merge Workflows](merge-workflows.md)
- [Complete Example](complete-example.md)
- [Next Steps](next-steps.md)
