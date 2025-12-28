# Storage Architecture

Prodigy uses a global storage architecture for persistent state, events, and failure tracking across all workflows and sessions.

## Overview

Global storage features:
- **Centralized storage**: All data in `~/.prodigy/`
- **Repository organization**: Data grouped by repository name
- **Cross-worktree sharing**: Multiple worktrees access shared state
- **Persistent state**: Job checkpoints survive worktree cleanup
- **Efficient deduplication**: Minimize storage overhead

## Documentation

This section covers the complete storage architecture:

- [**Storage Structure**](structure.md) - Directory layout, event storage, and checkpoint types
- [**Session & DLQ Storage**](session-dlq.md) - Session tracking and dead letter queue management
- [**Worktree Storage**](worktree-storage.md) - Git worktree organization and session-job mapping
- [**Maintenance**](maintenance.md) - Performance characteristics, cleanup, and migration

## Quick Reference

```
~/.prodigy/
├── events/                     # Event logs
├── dlq/                        # Dead Letter Queue
├── state/                      # State and checkpoints
├── sessions/                   # Session tracking
├── worktrees/                  # Git worktrees
└── orphaned_worktrees/         # Cleanup failure tracking
```
