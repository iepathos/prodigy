# Error Handling

Prodigy provides comprehensive error handling at both the workflow level (for MapReduce jobs) and the command level (for individual workflow steps). This chapter covers the practical features available for handling failures gracefully.

---

## Overview

Error handling in Prodigy operates at two levels:

1. **[Command-Level Error Handling](command-level.md)** - Handle failures for individual workflow steps with `on_failure` handlers, retry logic, and recovery strategies.

2. **[Workflow-Level Error Policy](workflow-level.md)** - Configure job-wide error policies for MapReduce workflows, including circuit breakers, retry configuration with backoff, and failure thresholds.

3. **[Dead Letter Queue (DLQ)](dlq.md)** - Store failed work items from MapReduce jobs for later retry or analysis.

4. **[Best Practices](best-practices.md)** - Guidelines for choosing the right error handling approach and common patterns.

---

## Quick Reference

### Command-Level (All Workflows)

```yaml
# Simple: ignore errors
- shell: "optional-cleanup.sh"
  on_failure: true

# Recovery command
- shell: "npm install"
  on_failure: "npm cache clean --force"

# Advanced with retry
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings ${shell.output}"
    max_attempts: 3
    fail_workflow: false
```

### Workflow-Level (MapReduce Only)

```yaml
mode: mapreduce
error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 10
  failure_threshold: 0.2
```

### Dead Letter Queue

```bash
# Retry failed items
prodigy dlq retry <job_id>

# View failed items
prodigy dlq show <job_id>
```
