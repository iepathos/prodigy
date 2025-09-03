---
number: 57
title: Claude Agent Real-Time Observability
category: optimization
priority: high
status: draft
dependencies: [51, 55]
created: 2025-01-03
---

# Specification 57: Claude Agent Real-Time Observability

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [51 - Event Logging, 55 - OpenTelemetry Integration]

## Context

Prodigy currently executes Claude commands in non-interactive mode using `claude --print` with minimal visibility into agent execution. As organizations deploy multiple concurrent prodigy agents through MapReduce workflows and other parallel execution patterns, the lack of real-time observability becomes a critical limitation. Teams need to monitor agent behavior, track costs, debug failures, and optimize performance across distributed agent fleets.

The Claude CLI supports JSON and stream-JSON output formats that provide rich execution metadata including session IDs, tool usage, costs, and real-time event streams. Additionally, Claude maintains detailed JSONL logs in `~/.claude/projects/` that can be correlated with execution sessions. This specification defines a comprehensive observability system that captures, processes, and exposes this data for monitoring dashboards and operational intelligence.

## Objective

Implement a real-time observability system for Claude agents that provides streaming telemetry, structured logging, and session correlation capabilities. Enable monitoring dashboards to track multiple concurrent agents, visualize tool usage patterns, monitor costs, and debug issues in real-time while maintaining compatibility with existing workflow execution.

## Requirements

### Functional Requirements

#### Stream Processing
- Capture real-time events from Claude using `--output-format stream-json`
- Parse and process JSONL event streams with proper error handling
- Support both streaming and batch JSON output formats
- Handle connection interruptions and resume event streaming
- Buffer events for reliability and performance optimization

#### Event Types and Schema
- System initialization events with session metadata
- Assistant messages with content and tool usage
- Tool invocation events with parameters and timing
- Tool result events with outputs and status
- Session completion events with metrics and costs
- Error events with context and recovery information

#### Session Management
- Extract and track session IDs from Claude output
- Correlate sessions with workflow executions
- Associate agents with unique identifiers
- Maintain session state and lifecycle tracking
- Support session replay from stored events

#### Data Collection
- Capture stdout/stderr from Claude processes
- Parse JSON/JSONL output formats
- Extract structured metrics and metadata
- Store events for historical analysis
- Implement configurable retention policies

#### Integration Points
- Modify `ClaudeExecutor` to support streaming output
- Add event emission to workflow executors
- Integrate with existing tracing infrastructure
- Support both legacy and streaming modes
- Maintain backward compatibility

### Non-Functional Requirements

#### Performance
- Minimal overhead on agent execution (< 5% CPU)
- Event processing latency < 100ms
- Support 100+ concurrent agents
- Efficient memory usage with streaming
- Configurable buffering and batching

#### Reliability
- Handle partial JSON parsing failures
- Recover from stream interruptions
- Persist events for crash recovery
- Support graceful degradation
- Implement circuit breakers for downstream services

#### Scalability
- Horizontal scaling of event processors
- Partitioned event streams by agent
- Distributed event aggregation
- Configurable parallelism limits
- Resource-based auto-scaling

#### Security
- Sanitize sensitive data from logs
- Implement access control for events
- Encrypt event streams in transit
- Audit trail for event access
- Configurable PII redaction

## Acceptance Criteria

- [ ] Claude executor supports `--output-format stream-json` with `--verbose`
- [ ] Event parser handles all Claude stream event types
- [ ] Session IDs are extracted and tracked consistently
- [ ] Events are emitted with < 100ms latency
- [ ] Dashboard receives real-time agent events
- [ ] Tool usage is tracked with timing and parameters
- [ ] Costs are accumulated per agent and session
- [ ] Errors include actionable context and stack traces
- [ ] Historical events can be queried by session ID
- [ ] System handles 100 concurrent agents without degradation
- [ ] Backward compatibility maintained with text output mode
- [ ] Integration tests cover streaming and error scenarios
- [ ] Documentation includes dashboard setup guide
- [ ] Metrics exported to Prometheus/OpenTelemetry

## Technical Details

### Implementation Approach

#### Phase 1: Streaming Infrastructure
```rust
// src/cook/execution/claude_streaming.rs
pub struct StreamingClaudeExecutor<R: CommandRunner> {
    runner: R,
    event_sender: mpsc::Sender<AgentEvent>,
    agent_id: String,
    session_id: Option<String>,
    buffer_size: usize,
}

impl<R: CommandRunner> StreamingClaudeExecutor<R> {
    pub async fn execute_with_streaming(
        &mut self,
        command: &str,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        let args = self.build_streaming_args(command);
        let mut child = self.spawn_claude_process(&args, &context).await?;
        
        // Process stream in parallel with execution
        let stream_handle = self.process_event_stream(child.stdout.take());
        let result = child.wait().await?;
        
        // Ensure all events are processed
        stream_handle.await?;
        
        Ok(self.build_execution_result(result))
    }
    
    async fn process_event_stream(&mut self, stdout: Stdio) -> JoinHandle<Result<()>> {
        let reader = BufReader::new(stdout);
        let sender = self.event_sender.clone();
        let agent_id = self.agent_id.clone();
        
        tokio::spawn(async move {
            for line in reader.lines() {
                if let Ok(json) = line {
                    Self::parse_and_emit_event(json, &sender, &agent_id).await?;
                }
            }
            Ok(())
        })
    }
}
```

#### Phase 2: Event Models
```rust
// src/observability/events.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeStreamEvent {
    System {
        subtype: SystemEventType,
        session_id: String,
        #[serde(flatten)]
        metadata: SystemMetadata,
    },
    Assistant {
        message: AssistantMessage,
        session_id: String,
        uuid: String,
    },
    ToolUse {
        tool_name: String,
        tool_id: String,
        input: serde_json::Value,
        session_id: String,
    },
    ToolResult {
        tool_id: String,
        output: String,
        success: bool,
        duration_ms: Option<u64>,
    },
    Result {
        success: bool,
        duration_ms: u64,
        cost_usd: f64,
        session_id: String,
        #[serde(flatten)]
        metrics: ExecutionMetrics,
    },
    Error {
        error_type: String,
        message: String,
        recoverable: bool,
        session_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetadata {
    pub model: String,
    pub tools: Vec<String>,
    pub cwd: PathBuf,
    pub permission_mode: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub api_calls: u32,
    pub tool_uses: u32,
}
```

#### Phase 3: Event Aggregation
```rust
// src/observability/aggregator.rs
pub struct EventAggregator {
    agents: DashMap<String, AgentState>,
    sessions: DashMap<String, SessionState>,
    event_store: Arc<dyn EventStore>,
    broadcast: broadcast::Sender<DashboardEvent>,
    metrics: Arc<MetricsCollector>,
}

impl EventAggregator {
    pub async fn handle_event(&self, event: AgentEvent) -> Result<()> {
        // Update agent state
        self.update_agent_state(&event).await?;
        
        // Update session state
        self.update_session_state(&event).await?;
        
        // Collect metrics
        self.metrics.record_event(&event);
        
        // Store for history
        self.event_store.persist(&event).await?;
        
        // Broadcast to dashboards
        self.broadcast_to_dashboards(event).await?;
        
        Ok(())
    }
    
    async fn update_agent_state(&self, event: &AgentEvent) -> Result<()> {
        self.agents
            .entry(event.agent_id().to_string())
            .and_modify(|state| state.apply_event(event))
            .or_insert_with(|| AgentState::from_event(event));
        Ok(())
    }
}
```

#### Phase 4: Dashboard Integration
```rust
// src/observability/dashboard.rs
pub struct DashboardService {
    aggregator: Arc<EventAggregator>,
    websocket_server: WebSocketServer,
    metrics_endpoint: MetricsEndpoint,
}

impl DashboardService {
    pub async fn start(&self) -> Result<()> {
        // Start WebSocket server for real-time events
        let ws_handle = self.start_websocket_server();
        
        // Start metrics endpoint for Prometheus
        let metrics_handle = self.start_metrics_endpoint();
        
        // Start event subscription
        let event_handle = self.subscribe_to_events();
        
        tokio::try_join!(ws_handle, metrics_handle, event_handle)?;
        Ok(())
    }
    
    async fn handle_dashboard_connection(&self, ws: WebSocket) -> Result<()> {
        let mut rx = self.aggregator.subscribe();
        
        while let Ok(event) = rx.recv().await {
            let msg = self.format_dashboard_event(&event)?;
            ws.send(msg).await?;
        }
        
        Ok(())
    }
}
```

### Architecture Changes

#### Component Interactions
```
┌─────────────────────────────────────────────────────┐
│                  Dashboard UI                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │ Agent    │ │ Metrics  │ │ Session  │            │
│  │ Monitor  │ │ Graphs   │ │ Replay   │            │
│  └──────────┘ └──────────┘ └──────────┘            │
└─────────────────────────────────────────────────────┘
           ▲            ▲            ▲
           │ WebSocket  │ HTTP      │ gRPC
┌──────────┴────────────┴────────────┴────────────────┐
│              Dashboard Service                       │
│  ┌──────────────────────────────────────────┐       │
│  │         Event Aggregator                  │       │
│  │  ┌────────┐ ┌────────┐ ┌────────┐       │       │
│  │  │ Agent  │ │Session │ │Metrics │       │       │
│  │  │ State  │ │ State  │ │Collector│      │       │
│  │  └────────┘ └────────┘ └────────┘       │       │
│  └──────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────┘
           ▲            ▲            ▲
           │ Events     │            │
┌──────────┴────────────┴────────────┴────────────────┐
│            Streaming Claude Executors                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │ Agent 1  │ │ Agent 2  │ │ Agent N  │            │
│  │ (stream) │ │ (stream) │ │ (stream) │            │
│  └──────────┘ └──────────┘ └──────────┘            │
└──────────────────────────────────────────────────────┘
```

#### Data Flow
1. Claude executor spawns with `--output-format stream-json`
2. JSONL events parsed and converted to internal format
3. Events sent to aggregator via async channel
4. Aggregator updates state and broadcasts to subscribers
5. Dashboard receives events via WebSocket
6. Metrics exported to Prometheus/OpenTelemetry

### Data Structures

#### Event Storage Schema
```sql
CREATE TABLE agent_events (
    id UUID PRIMARY KEY,
    agent_id VARCHAR(128) NOT NULL,
    session_id VARCHAR(128),
    event_type VARCHAR(64) NOT NULL,
    event_data JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    INDEX idx_agent_id (agent_id),
    INDEX idx_session_id (session_id),
    INDEX idx_timestamp (timestamp)
);

CREATE TABLE agent_sessions (
    session_id VARCHAR(128) PRIMARY KEY,
    agent_id VARCHAR(128) NOT NULL,
    workflow_id VARCHAR(128),
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    status VARCHAR(32),
    metrics JSONB,
    INDEX idx_agent_id (agent_id),
    INDEX idx_workflow_id (workflow_id)
);
```

### APIs and Interfaces

#### Event Stream API
```protobuf
service ObservabilityService {
    rpc StreamEvents(StreamEventsRequest) returns (stream AgentEvent);
    rpc GetSession(GetSessionRequest) returns (SessionDetails);
    rpc ListAgents(ListAgentsRequest) returns (AgentList);
    rpc GetMetrics(GetMetricsRequest) returns (MetricsSnapshot);
}

message StreamEventsRequest {
    repeated string agent_ids = 1;
    repeated string session_ids = 2;
    repeated EventType event_types = 3;
    google.protobuf.Timestamp since = 4;
}
```

#### REST API Endpoints
```yaml
/api/v1/observability:
  /agents:
    GET: List active agents with status
  /agents/{id}/events:
    GET: Stream events for specific agent
  /sessions/{id}:
    GET: Get session details and events
  /metrics:
    GET: Prometheus-format metrics
  /dashboard/ws:
    WS: WebSocket endpoint for real-time events
```

## Dependencies

- **Prerequisites**: 
  - Specification 51: Event Logging infrastructure
  - Specification 55: OpenTelemetry integration
- **Affected Components**:
  - `ClaudeExecutor`: Add streaming support
  - `WorkflowExecutor`: Emit lifecycle events
  - `CommandRunner`: Support process streaming
- **External Dependencies**:
  - `tokio`: Async runtime for streaming
  - `serde_json`: JSON/JSONL parsing
  - `tungstenite`: WebSocket server
  - `prometheus`: Metrics export

## Testing Strategy

### Unit Tests
- Event parser handles all Claude event types
- Session ID extraction from various formats
- Event aggregation state management
- Metrics calculation accuracy
- Buffer overflow handling

### Integration Tests
- End-to-end streaming with real Claude CLI
- Multiple concurrent agent execution
- WebSocket event delivery
- Session reconstruction from events
- Error recovery and reconnection

### Performance Tests
- Measure event processing latency
- Load test with 100+ concurrent agents
- Memory usage under sustained load
- Network bandwidth optimization
- Database write throughput

### User Acceptance
- Dashboard displays real-time agent status
- Tool usage visualization is accurate
- Cost tracking matches Claude billing
- Session replay shows correct timeline
- Error messages provide actionable context

## Documentation Requirements

### Code Documentation
- Event type definitions and schemas
- Streaming protocol specification
- Session correlation logic
- Dashboard API reference
- Configuration parameters

### User Documentation
- Dashboard setup and configuration guide
- Monitoring best practices
- Troubleshooting event streaming
- Cost optimization strategies
- Security and privacy considerations

### Architecture Updates
- Update ARCHITECTURE.md with observability layer
- Document event flow and processing pipeline
- Add dashboard component diagram
- Include deployment topology options

## Implementation Notes

### Streaming Considerations
- Use `--verbose` flag required for stream-json format
- Handle partial JSON lines in stream
- Implement backpressure for slow consumers
- Consider event deduplication for retries
- Buffer size tuning for performance

### Session Correlation
- Session IDs in JSON output: `session_id` field
- JSONL logs in `~/.claude/projects/{project-path}/`
- Filename is session UUID with `.jsonl` extension
- Correlate by matching session_id and timestamp
- Support both real-time and post-hoc analysis

### Error Handling
- Graceful degradation to text output on error
- Retry streaming connection on interruption
- Queue events during network outages
- Log parsing errors without failing execution
- Provide fallback metrics from text output

### Performance Optimization
- Use zero-copy parsing where possible
- Implement event batching for network efficiency
- Consider compression for event storage
- Lazy load historical events
- Cache frequently accessed sessions

## Migration and Compatibility

### Backward Compatibility
- Support existing text-based output mode
- Feature flag for enabling streaming mode
- Gradual rollout to production workflows
- Maintain existing ExecutionResult interface
- Preserve current logging behavior

### Migration Path
1. Deploy streaming executor alongside existing
2. Enable for development workflows first
3. Gradually migrate production workflows
4. Monitor performance and reliability
5. Deprecate text mode after validation

### Configuration Migration
```yaml
# Legacy configuration
claude:
  output_format: text
  
# New configuration with observability
claude:
  output_format: stream-json
  observability:
    enabled: true
    buffer_size: 1000
    event_retention_days: 30
    dashboard_port: 8080
```

### Breaking Changes
- None for existing workflows
- New dependencies for dashboard features
- Additional CPU/memory for event processing
- Network bandwidth for streaming events
- Storage requirements for event history