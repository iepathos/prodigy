# Prodigy Workflow Examples

This directory contains example workflows for the Prodigy tool, demonstrating both sequential and MapReduce execution modes.

## Sequential Workflows

Sequential workflows process steps one at a time in order:

- **debtmap.yml** - Analyzes and fixes technical debt sequentially
- **implement.yml** - Implements new features with test-driven development
- **simple.yml** - Basic example workflow

## MapReduce Workflows

MapReduce workflows enable parallel processing across multiple isolated worktrees:

### Production Workflows

- **debtmap-mapreduce.yml** - Parallel technical debt elimination
  - Analyzes codebase and generates debt items
  - Processes high-impact items in parallel (up to 5 agents)
  - Merges all fixes and generates summary report
  - Example: `prodigy cook workflows/debtmap-mapreduce.yml --worktree`

- **fix-files-mapreduce.yml** - Fix issues in multiple files concurrently
  - Uses regex pattern matching to filter Rust files
  - Processes files by complexity (highest first)
  - Validates all changes with build and test checks
  - Demonstrates the new regex matching feature

### Test Workflows

- **test-mapreduce.yml** - Simple test of MapReduce functionality
  - Creates test data with different severity levels
  - Filters and processes high/critical items only
  - Useful for verifying MapReduce setup

- **mapreduce-example.yml** - Original example showing all MapReduce features
  - Complete example with setup, map, and reduce phases
  - Shows variable interpolation syntax
  - Includes filtering, sorting, and error handling

## MapReduce Features

### Variable Interpolation
- `${item.field}` - Access work item fields
- `${item.nested.field}` - Nested property access
- `${array[0]}` - Array indexing
- `${field:-default}` - Default values
- `${shell.output}` - Command output capture
- `${map.successful}`, `${map.failed}` - Aggregate statistics

### Filtering (Data Pipeline)
- Comparison: `priority > 5`, `severity == 'high'`
- Logical: `severity == 'high' && priority > 5`
- Regex: `path matches '\.rs$'` (NEW - regex pattern matching)
- Contains: `description contains 'memory'`
- In: `status in ['pending', 'active']`

### Configuration Options
- `max_parallel`: Number of concurrent agents (default: 10)
- `timeout_per_agent`: Maximum time per agent (default: 600s)
- `retry_on_failure`: Retry attempts for failed agents (default: 2)
- `max_items`: Limit number of items to process
- `offset`: Skip first N items
- `sort_by`: Sort items before processing
- `filter`: Filter expression to select items

## Usage Examples

### Sequential Execution
```bash
# Run sequential debtmap workflow
prodigy cook workflows/debtmap.yml

# Run in a worktree for isolation
prodigy cook workflows/debtmap.yml --worktree
```

### MapReduce Execution
```bash
# Run parallel debt elimination
prodigy cook workflows/debtmap-mapreduce.yml --worktree

# Test MapReduce with simple workflow
prodigy cook workflows/test-mapreduce.yml

# Fix Rust files in parallel with regex filtering
prodigy cook workflows/fix-files-mapreduce.yml --worktree

# Auto-merge results to main branch
prodigy cook workflows/debtmap-mapreduce.yml --worktree -y
```

## Workflow Modes

- **sequential** (default): Execute steps one after another
- **mapreduce**: Execute map phase in parallel, then reduce phase

## Best Practices

1. **Use MapReduce for**:
   - Processing multiple independent items
   - Parallelizable tasks (file fixes, test runs, analysis)
   - Large-scale refactoring across many files

2. **Use Sequential for**:
   - Dependent steps that must run in order
   - Single-threaded operations
   - Simple workflows with few steps

3. **Always use --worktree** for:
   - MapReduce workflows (required for isolation)
   - Experimental or risky changes
   - Testing new workflows

4. **Performance Tips**:
   - Set appropriate `max_parallel` based on system resources
   - Use filtering to process only relevant items
   - Sort by priority/impact to fix important items first
   - Set reasonable timeouts to prevent hanging agents