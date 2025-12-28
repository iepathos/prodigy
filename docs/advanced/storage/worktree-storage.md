# Worktree Storage

This page covers Git worktree organization, orphaned worktree tracking, and session-job mapping.

## Worktree Storage

Git worktrees are created per session:

```
~/.prodigy/worktrees/{repo_name}/
├── session-abc123/             # Workflow session
└── session-mapreduce-xyz/      # MapReduce parent worktree
    ├── agent-1/                # MapReduce agent worktree
    └── agent-2/                # MapReduce agent worktree
```

### Worktree Lifecycle

1. **Creation**: Worktree created when workflow starts
2. **Execution**: All commands run in worktree context
3. **Persistence**: Worktree remains until merge or cleanup
4. **Cleanup**: Removed after successful merge

## Orphaned Worktree Tracking

When cleanup fails, worktree paths are registered:

```
~/.prodigy/orphaned_worktrees/{repo_name}/{job_id}.json
```

### Registry Format

```json
{
  "job_id": "mapreduce-123",
  "orphaned_worktrees": [
    {
      "agent_id": "agent-1",
      "item_id": "item-1",
      "worktree_path": "/Users/user/.prodigy/worktrees/prodigy/agent-1",
      "timestamp": "2025-01-11T12:00:00Z",
      "error": "Permission denied"
    }
  ]
}
```

## Session-Job Mapping

Bidirectional mapping enables resume with session or job IDs:

```
~/.prodigy/state/{repo_name}/mappings/
├── session-to-job.json
└── job-to-session.json
```

### Mapping Format

**session-to-job.json**:
```json
{
  "session-mapreduce-xyz": "mapreduce-123"
}
```

**job-to-session.json**:
```json
{
  "mapreduce-123": "session-mapreduce-xyz"
}
```
