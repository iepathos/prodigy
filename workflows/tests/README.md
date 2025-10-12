# Test Workflows

This directory contains minimal test workflows for verifying Prodigy functionality.

## Available Tests

### minimal-mapreduce.yml

Tests the MapReduce workflow merge path, specifically the fix for the bug where MapReduce worktree changes weren't merging back to the parent worktree.

**What it tests:**
- Agent worktrees → MapReduce worktree merge
- **MapReduce worktree → parent worktree merge** (the critical fix)
- Parallel processing with max_parallel
- Setup, map, and reduce phases
- Git commits and merge operations

**How to run:**
```bash
prodigy run workflows/tests/minimal-mapreduce.yml
```

**Expected outcome:**
1. Setup phase creates `test-items.json` with 3 items
2. Map phase spawns 2 parallel agents to process the 3 items
3. Each agent creates an output file and commits it
4. Agents merge their changes to the MapReduce worktree
5. MapReduce worktree merges to parent worktree using `/prodigy-merge-worktree`
6. Reduce phase verifies all 3 output files exist
7. All changes are visible in the parent worktree after completion

**How to verify the fix:**
After running, check that:
```bash
# The parent worktree should contain all output files
prodigy worktree ls  # Find your session worktree
cd ~/.prodigy/worktrees/prodigy/<session-name>
ls output-*.txt  # Should show 3 files

# Git log should show all commits
git log --oneline
# Should include:
# - Process item-one (id=1)
# - Process item-two (id=2)
# - Process item-three (id=3)
```

**What would happen with the old bug:**
Without the fix, the output files would be trapped in the MapReduce worktree and never reach the parent worktree. The merge would fail with:
```
fatal: not a git repository (or any of the parent directories): .git
```

## Test Workflow Best Practices

1. **Keep them minimal**: Test one thing at a time
2. **Make them fast**: Use small datasets and simple operations
3. **Make them verifiable**: Include checks that prove the test passed
4. **Document expectations**: Explain what should happen
5. **Document the bug**: Explain what the old behavior was

## Adding New Test Workflows

When adding new test workflows:

1. Name them descriptively: `test-<feature>-<scenario>.yml`
2. Add documentation at the top explaining what it tests
3. Include verification steps in the workflow itself
4. Add an entry to this README
5. Keep them in this `workflows/tests/` directory
