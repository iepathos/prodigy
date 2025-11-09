## Event Tracking

All MapReduce execution events are logged to `~/.prodigy/events/{repo_name}/{job_id}/` for debugging and monitoring:

**Events Tracked:**
- Agent lifecycle events (started, completed, failed)
- Work item processing status
- Checkpoint saves for resumption
- Error details with correlation IDs
- Cross-worktree event aggregation for parallel jobs

**Event Log Format:**
Events are stored in JSONL (JSON Lines) format, with each line representing a single event:

```json
{"timestamp":"2024-01-01T12:00:00Z","event_type":"agent_started","agent_id":"agent-1","item_id":"item-001"}
{"timestamp":"2024-01-01T12:05:00Z","event_type":"agent_completed","agent_id":"agent-1","item_id":"item-001","status":"success"}
```

**Viewing Events:**
```bash
# View all events for a job
prodigy events <job_id>

# Stream events in real-time
prodigy events <job_id> --follow
```

