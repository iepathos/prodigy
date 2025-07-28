# Test Script for Worktree Logging

To test the worktree logging with args, you can run:

```bash
# Test with direct args
mmm improve -wn 1 -c examples/implement.yml --args "31"

# Test with map
mmm improve -wn 1 -c examples/implement.yml --map "specs/31*.md"
```

Both commands should now display:
1. The worktree creation message: `🌳 Created worktree: session-xxx at /path/to/worktree`
2. The command with args: `🤖 Running /mmm-implement-spec 31`

## What was fixed:

1. **Enhanced command logging** - Now shows the full command with resolved arguments
2. **Fixed worktree mode with args/map** - Previously, when using `--args` or `--map`, the code bypassed worktree creation
3. **Unified worktree handling** - Now both `run_standard` and `run_with_mapping` properly handle worktree creation

The key issue was that `run_with_mapping` didn't check for worktree mode, so it would run directly in the current directory instead of creating an isolated worktree.