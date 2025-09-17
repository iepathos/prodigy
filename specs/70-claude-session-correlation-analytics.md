---
number: 70
title: Claude Session Correlation & Analytics
category: optimization
priority: medium
status: draft
dependencies: [57]
created: 2025-01-16
---

# Specification 70: Claude Session Correlation & Analytics

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [57 - Claude Streaming Output Support]

## Context

Claude maintains detailed JSONL logs for every session in `~/.claude/projects/` that contain rich metadata about tool usage, token consumption, model interactions, and execution timelines. These logs provide a goldmine of operational intelligence that is currently untapped by Prodigy.

With spec 57 providing real-time streaming of Claude events, this specification extends that capability to correlate streaming events with historical session data, enabling advanced analytics, cost tracking, performance optimization, and debugging capabilities. Organizations running large-scale Claude deployments need these insights to optimize costs, identify performance bottlenecks, and improve agent effectiveness.

## Objective

Build a comprehensive analytics layer that correlates real-time Claude events with historical session logs, providing insights into tool usage patterns, token consumption, cost tracking, and performance metrics. Enable session replay, cross-session analysis, and intelligent optimization recommendations.

## Requirements

### Functional Requirements

#### Session Correlation
- Extract session IDs from Claude streaming output
- Locate corresponding JSONL files in `~/.claude/projects/{project-path}/`
- Parse and index historical session data
- Correlate real-time events with historical logs
- Support session lookup by ID, timestamp, or workflow

#### Analytics Engine
- Calculate token usage and costs per session
- Track tool invocation frequency and duration
- Identify performance bottlenecks and slow tools
- Generate usage patterns and trends
- Support custom metric definitions

#### Data Collection
- Monitor `~/.claude/projects/` for new session files
- Parse JSONL files incrementally as they grow
- Extract structured metrics from session data
- Store analytics data for long-term analysis
- Support data export for external analysis

#### Session Replay
- Reconstruct session timeline from JSONL logs
- Replay tool invocations in sequence
- Show intermediate results and outputs
- Support time-based navigation
- Export session transcripts

### Non-Functional Requirements

#### Performance
- Lazy loading of session data
- Indexed search across sessions
- Cached analytics computations
- Incremental processing of new data

#### Scalability
- Handle thousands of session files
- Support parallel analysis jobs
- Distribute analytics computation
- Archive old sessions efficiently

## Acceptance Criteria

- [ ] Session IDs extracted from Claude output and correlated with JSONL files
- [ ] Analytics dashboard shows tool usage statistics and trends
- [ ] Cost tracking accurate to Claude's billing metrics
- [ ] Session replay allows step-by-step execution review
- [ ] Performance bottlenecks identified automatically
- [ ] Cross-session analysis reveals usage patterns
- [ ] Data export available in JSON/CSV formats
- [ ] Historical data indexed and searchable

## Technical Details

### Implementation Approach

#### Session Watcher
```rust
// src/analytics/session_watcher.rs
pub struct SessionWatcher {
    claude_projects_path: PathBuf,
    event_logger: Arc<EventLogger>,
    index: Arc<SessionIndex>,
}

impl SessionWatcher {
    pub async fn watch(&self) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher = notify::Watcher::new(tx, Duration::from_secs(1))?;

        watcher.watch(&self.claude_projects_path, RecursiveMode::Recursive)?;

        while let Ok(event) = rx.recv() {
            if let DebouncedEvent::Create(path) | DebouncedEvent::Write(path) = event {
                if path.extension() == Some("jsonl") {
                    self.process_session_file(path).await?;
                }
            }
        }
        Ok(())
    }

    async fn process_session_file(&self, path: PathBuf) -> Result<()> {
        let session_id = Self::extract_session_id(&path)?;
        let events = self.parse_jsonl_incremental(&path).await?;

        for event in events {
            self.index.add_event(session_id, event).await?;
        }

        Ok(())
    }
}
```

#### Analytics Engine
```rust
// src/analytics/engine.rs
pub struct AnalyticsEngine {
    index: Arc<SessionIndex>,
    metrics: Arc<MetricsCollector>,
}

impl AnalyticsEngine {
    pub async fn calculate_session_cost(&self, session_id: &str) -> Result<Cost> {
        let session = self.index.get_session(session_id).await?;

        Ok(Cost {
            input_tokens: session.total_input_tokens(),
            output_tokens: session.total_output_tokens(),
            cache_tokens: session.total_cache_tokens(),
            estimated_cost_usd: self.calculate_cost(session),
        })
    }

    pub async fn analyze_tool_usage(&self, time_range: TimeRange) -> Result<ToolStats> {
        let sessions = self.index.query_sessions(time_range).await?;

        let mut tool_stats = HashMap::new();
        for session in sessions {
            for tool_use in session.tool_invocations() {
                tool_stats.entry(tool_use.name)
                    .and_modify(|s| s.increment(tool_use))
                    .or_insert_with(|| ToolStat::from(tool_use));
            }
        }

        Ok(ToolStats { stats: tool_stats })
    }
}
```

#### Session Replay
```rust
// src/analytics/replay.rs
pub struct SessionReplay {
    session: Session,
    current_position: usize,
}

impl SessionReplay {
    pub fn step_forward(&mut self) -> Option<ReplayEvent> {
        if self.current_position < self.session.events.len() {
            let event = &self.session.events[self.current_position];
            self.current_position += 1;
            Some(self.format_replay_event(event))
        } else {
            None
        }
    }

    pub fn jump_to_timestamp(&mut self, timestamp: DateTime<Utc>) {
        self.current_position = self.session.events
            .iter()
            .position(|e| e.timestamp >= timestamp)
            .unwrap_or(self.session.events.len());
    }
}
```

### Data Structures

#### Session Index Schema
```sql
CREATE TABLE claude_sessions (
    session_id VARCHAR(128) PRIMARY KEY,
    project_path TEXT NOT NULL,
    jsonl_path TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    model VARCHAR(64),
    total_tokens INTEGER,
    estimated_cost DECIMAL(10,4),
    tool_count INTEGER,
    INDEX idx_started_at (started_at),
    INDEX idx_project (project_path)
);

CREATE TABLE claude_tool_uses (
    id SERIAL PRIMARY KEY,
    session_id VARCHAR(128) REFERENCES claude_sessions,
    tool_name VARCHAR(128),
    invoked_at TIMESTAMPTZ,
    duration_ms INTEGER,
    parameters JSONB,
    result_size INTEGER,
    INDEX idx_session (session_id),
    INDEX idx_tool (tool_name),
    INDEX idx_time (invoked_at)
);
```

### APIs and Interfaces

#### Analytics API
```yaml
/api/v1/analytics:
  /sessions:
    GET: List sessions with filtering and pagination
  /sessions/{id}:
    GET: Get detailed session information
  /sessions/{id}/replay:
    GET: Get session replay data
    POST: Control replay position
  /tools/usage:
    GET: Tool usage statistics
  /costs:
    GET: Cost analysis and projections
  /patterns:
    GET: Usage patterns and recommendations
```

## Dependencies

- **Prerequisites**: Specification 57 (Claude Streaming Output)
- **Affected Components**:
  - Event system for session correlation
  - Dashboard for analytics visualization
- **External Dependencies**:
  - `notify`: File system watching
  - `sqlx`: Database for session index

## Testing Strategy

### Unit Tests
- Session ID extraction from file paths
- JSONL parsing and incremental processing
- Cost calculation accuracy
- Tool usage statistics

### Integration Tests
- End-to-end session correlation
- Analytics calculation with real data
- Session replay functionality
- File watching and indexing

## Documentation Requirements

### User Documentation
- Analytics dashboard user guide
- Cost optimization best practices
- Session replay tutorial
- API reference for analytics endpoints

### Architecture Documentation
- Session correlation architecture
- Analytics data flow
- Index schema and design decisions

## Implementation Notes

### JSONL File Structure
Claude session files follow this pattern:
- Location: `~/.claude/projects/{project-name}/{session-id}.jsonl`
- Format: One JSON object per line
- Events include: system, assistant, tool_use, tool_result, error

### Cost Calculation
Based on Claude's pricing model:
- Input tokens: $X per 1M tokens
- Output tokens: $Y per 1M tokens
- Cache tokens: $Z per 1M tokens
- Model-specific multipliers

### Performance Optimization
- Index sessions by timestamp for efficient queries
- Cache frequently accessed analytics
- Use database views for complex aggregations
- Implement data retention policies

## Migration and Compatibility

### Data Migration
- Index existing session files on first run
- Support incremental indexing for new sessions
- Handle missing or corrupted JSONL files gracefully

### Future Extensions
- Machine learning for pattern detection
- Automated optimization recommendations
- Cross-project analytics
- Team usage dashboards