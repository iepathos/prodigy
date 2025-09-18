# MapReduce Job Resume Guide

This guide explains how to resume interrupted MapReduce jobs in Prodigy, including job recovery, retry configuration, and best practices for handling large-scale parallel processing.

## Table of Contents

1. [Overview](#overview)
2. [Basic Resume Operations](#basic-resume-operations)
3. [Advanced Resume Options](#advanced-resume-options)
4. [Checkpoint Management](#checkpoint-management)
5. [DLQ (Dead Letter Queue) Integration](#dlq-integration)
6. [Performance Considerations](#performance-considerations)
7. [Troubleshooting](#troubleshooting)
8. [Examples](#examples)

## Overview

MapReduce jobs in Prodigy automatically create checkpoints during execution, allowing you to resume interrupted jobs without losing progress. The resume functionality includes:

- **Automatic checkpointing**: State saved after each agent completion
- **DLQ preservation**: Failed items tracked for later reprocessing
- **Duplicate prevention**: Already-processed items won't be reprocessed
- **Configuration overrides**: Adjust parallelism and retry settings on resume

## Basic Resume Operations

### Resuming an Interrupted Job

When a MapReduce job is interrupted (e.g., system crash, manual cancellation), you can resume it:

```bash
# Resume using the original workflow file
prodigy resume

# Resume a specific job by ID
prodigy resume-job mapreduce-20240101-123456
```

The resume command will:
1. Load the latest checkpoint
2. Restore completed agent results
3. Identify remaining work items
4. Continue processing from where it left off

### Checking Job Status

Before resuming, check the current state:

```bash
# View job checkpoints
prodigy checkpoints list mapreduce-20240101-123456

# View job events
prodigy events show mapreduce-20240101-123456

# Check DLQ status
prodigy dlq status mapreduce-20240101-123456
```

## Advanced Resume Options

### Force Resume

Force a resume even if the job appears complete or has no retriable items:

```bash
prodigy resume-job mapreduce-20240101-123456 --force
```

This is useful when:
- You want to retry all failed items regardless of retry limits
- The job state is corrupted but you want to attempt recovery
- You need to reprocess items due to external changes

### Adjusting Parallelism

Change the number of parallel agents during resume:

```bash
# Increase parallelism for faster processing
prodigy resume-job mapreduce-20240101-123456 --max-parallel 20

# Decrease parallelism to reduce system load
prodigy resume-job mapreduce-20240101-123456 --max-parallel 5
```

### Adding Extra Retries

Grant additional retry attempts to failed items:

```bash
# Add 2 extra retry attempts for failed items
prodigy resume-job mapreduce-20240101-123456 --max-additional-retries 2
```

This is helpful when:
- Temporary issues (network, API limits) caused failures
- You've fixed the underlying problem and want to retry
- You need more attempts than originally configured

### Skipping Validation

Skip pre-resume validation checks:

```bash
prodigy resume-job mapreduce-20240101-123456 --skip-validation
```

Use with caution - this bypasses safety checks that prevent:
- Resuming completed jobs
- Processing with incompatible configurations
- Checkpoint corruption issues

### Resume from Specific Checkpoint

Resume from an earlier checkpoint version:

```bash
# List available checkpoints
prodigy checkpoints list mapreduce-20240101-123456

# Resume from checkpoint version 5
prodigy resume-job mapreduce-20240101-123456 --from-checkpoint 5
```

## Checkpoint Management

### Understanding Checkpoints

Checkpoints contain:
- Complete job configuration
- All work items and their status
- Agent results (success/failure)
- DLQ state for failed items
- Reduce phase progress

### Checkpoint Storage

Checkpoints are stored in:
```
~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
├── checkpoint-v{N}.json      # Checkpoint files
├── metadata.json             # Job metadata
└── latest.json              # Symlink to latest checkpoint
```

### Checkpoint Retention

- Maximum 3 checkpoints retained per job
- Older checkpoints automatically cleaned up
- Manual cleanup: `prodigy checkpoints clean mapreduce-20240101-123456`

## DLQ Integration

### Understanding DLQ on Resume

When resuming a job:
1. Failed items from previous runs are restored
2. Retry counts are preserved
3. Items exceeding retry limits remain in DLQ
4. New failures are added to existing DLQ

### Reprocessing DLQ Items

After resume, process DLQ items separately:

```bash
# View DLQ items
prodigy dlq list mapreduce-20240101-123456

# Reprocess all DLQ items
prodigy dlq reprocess mapreduce-20240101-123456

# Reprocess with custom parallelism
prodigy dlq reprocess mapreduce-20240101-123456 --max-parallel 10
```

## Performance Considerations

### Checkpoint Overhead

- Checkpoints add <1s overhead per save
- Automatic after each agent completion
- Asynchronous to minimize impact
- Use atomic writes to prevent corruption

### Memory Usage

For large jobs (>10,000 items):
- State loaded incrementally
- Only active items kept in memory
- Consider `--max-items` to limit batch size

### Network Considerations

When resuming with many agents:
- Start with lower `--max-parallel` value
- Gradually increase if system handles load
- Monitor API rate limits
- Use exponential backoff for retries

## Troubleshooting

### Common Issues

#### "No retriable items found"
- All items either succeeded or exceeded retry limit
- Use `--force` to retry anyway
- Check DLQ for permanently failed items

#### "Checkpoint not found"
- Job may have been cleaned up
- Check if job ID is correct
- Verify checkpoint directory exists

#### "Workflow file not found"
- Original workflow file moved/deleted
- Recreate workflow file with same structure
- Use `prodigy resume` from original directory

### Debug Commands

```bash
# Verbose logging
prodigy resume-job mapreduce-20240101-123456 --verbose

# Check job state details
prodigy sessions show

# Inspect checkpoint file directly
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/latest.json | jq .
```

## Examples

### Example 1: Simple Resume After Interruption

```bash
# Original job interrupted after processing 50/100 items
$ prodigy cook analyze-codebase.yaml
[... processes 50 items, then interrupted with Ctrl+C ...]

# Resume from where it left off
$ prodigy resume
Resuming job mapreduce-20240101-123456
Loaded checkpoint v50: 50 completed, 50 remaining
Continuing with 50 pending items...
[... processes remaining 50 items ...]
Job completed successfully!
```

### Example 2: Resume with Retry for Failed Items

```bash
# Check job status
$ prodigy checkpoints list mapreduce-20240101-123456
Job Status: Incomplete
- Successful: 80
- Failed: 20 (10 in DLQ, 10 retriable)
- Pending: 0

# Resume with extra retries
$ prodigy resume-job mapreduce-20240101-123456 --max-additional-retries 3
Resuming with 10 retriable items
Granting 3 additional retry attempts
[... retries failed items ...]

# Process DLQ items separately
$ prodigy dlq reprocess mapreduce-20240101-123456
Processing 10 DLQ items...
```

### Example 3: Resume with Configuration Changes

```yaml
# Original workflow: analyze-codebase.yaml
name: analyze-codebase
mode: mapreduce

map:
  input: "files.json"
  json_path: "$.files[*]"
  max_parallel: 5  # Original setting

  agent_template:
    - claude: "/analyze '${item.path}'"
```

```bash
# Resume with increased parallelism for faster processing
$ prodigy resume-job mapreduce-20240101-123456 --max-parallel 20
Resuming job with max_parallel override: 5 -> 20
Processing 30 remaining items with 20 parallel agents...
```

### Example 4: Recovery from Checkpoint

```bash
# List available checkpoints
$ prodigy checkpoints list mapreduce-20240101-123456
Available checkpoints:
- v95 (latest): 95/100 complete
- v90: 90/100 complete
- v85: 85/100 complete

# Something went wrong after v90, resume from there
$ prodigy resume-job mapreduce-20240101-123456 --from-checkpoint 90
Loading checkpoint v90...
Rolling back to: 90 completed, 10 remaining
Continuing from checkpoint v90...
```

## Best Practices

1. **Monitor Progress**: Use `prodigy events watch` during execution
2. **Set Appropriate Retry Limits**: Balance between resilience and efficiency
3. **Use DLQ for Permanent Failures**: Don't endlessly retry doomed items
4. **Test Resume Logic**: Interrupt test jobs to verify resume works
5. **Clean Up Old Jobs**: Run `prodigy checkpoints clean --all --older-than 7d`
6. **Document Retry Strategies**: Comment workflow files with retry reasoning

## Summary

MapReduce resume functionality ensures no work is lost when jobs are interrupted. With automatic checkpointing, flexible retry options, and DLQ integration, you can confidently run large-scale parallel processing jobs with the ability to recover from any failure scenario.

For more information:
- [MapReduce Workflows](./PRODIGY_WHITEPAPER.md#mapreduce-workflows)
- [DLQ Management](./dlq-management.md)
- [Performance Tuning](./performance-tuning.md)