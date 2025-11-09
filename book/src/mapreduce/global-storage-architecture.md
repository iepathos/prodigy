## Global Storage Architecture

MapReduce workflows use a global storage architecture located in `~/.prodigy/` (not `.prodigy/` in your project). This enables:

- **Cross-worktree event aggregation**: Multiple worktrees working on the same job share event logs
- **Persistent state management**: Job checkpoints survive worktree cleanup
- **Centralized monitoring**: All job data accessible from a single location
- **Efficient storage**: Deduplication across worktrees

