---
number: 55
title: MapReduce OpenTelemetry Integration
category: optimization
priority: medium
status: draft
dependencies: [51, 53]
created: 2025-01-29
---

# Specification 55: MapReduce OpenTelemetry Integration

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [51 - Event Logging, 53 - Structured Errors]

## Context

The current MapReduce implementation uses basic logging via the `tracing` crate but lacks distributed tracing capabilities. This makes it difficult to track request flow across parallel agents, correlate events across the system, and integrate with modern observability platforms. OpenTelemetry provides a vendor-neutral standard for distributed tracing, metrics, and logs.

## Objective

Integrate OpenTelemetry into the MapReduce system to provide comprehensive distributed tracing, standardized metrics collection, and seamless integration with observability platforms like Jaeger, Zipkin, Prometheus, and cloud-native monitoring solutions.

## Requirements

### Functional Requirements
- Instrument all MapReduce operations with spans
- Propagate trace context across agents
- Collect standardized metrics
- Export traces to multiple backends
- Support sampling strategies
- Include baggage propagation
- Correlate logs with traces
- Support custom span attributes

### Non-Functional Requirements
- Tracing overhead < 2% of execution time
- Support 100,000+ spans per job
- Configurable sampling rates
- Zero data loss for sampled traces
- Support multiple export formats

## Acceptance Criteria

- [ ] OpenTelemetry SDK integrated
- [ ] All major operations create spans
- [ ] Trace context propagated to agents
- [ ] Metrics exported to Prometheus format
- [ ] Traces viewable in Jaeger UI
- [ ] Sampling configuration implemented
- [ ] Performance overhead measured < 2%
- [ ] Custom attributes on all spans
- [ ] Error spans properly marked
- [ ] Documentation for observability setup

## Technical Details

### Implementation Approach

1. **OpenTelemetry Setup**
```rust
use opentelemetry::{
    global,
    sdk::{propagation::TraceContextPropagator, trace, Resource},
    trace::{Span, SpanKind, Status, TraceContextExt, Tracer},
    KeyValue,
};
use opentelemetry_otlp::{OtlpExporterBuilder, Protocol};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, Registry};

pub struct TelemetryConfig {
    pub service_name: String,
    pub otlp_endpoint: String,
    pub sampling_rate: f64,
    pub max_spans_per_second: u32,
    pub export_timeout: Duration,
}

impl TelemetryConfig {
    pub fn init(&self) -> Result<()> {
        global::set_text_map_propagator(TraceContextPropagator::new());
        
        let resource = Resource::new(vec![
            KeyValue::new("service.name", self.service_name.clone()),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ]);
        
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                OtlpExporterBuilder::default()
                    .with_endpoint(&self.otlp_endpoint)
                    .with_protocol(Protocol::Grpc)
                    .with_timeout(self.export_timeout)
            )
            .with_trace_config(
                trace::config()
                    .with_sampler(trace::Sampler::TraceIdRatioBased(self.sampling_rate))
                    .with_resource(resource)
                    .with_max_events_per_span(64)
                    .with_max_attributes_per_span(32)
            )
            .install_batch(opentelemetry::runtime::Tokio)?;
        
        let telemetry_layer = OpenTelemetryLayer::new(tracer);
        
        let subscriber = Registry::default()
            .with(telemetry_layer)
            .with(tracing_subscriber::fmt::layer());
        
        tracing::subscriber::set_global_default(subscriber)?;
        
        Ok(())
    }
}
```

2. **Instrumented MapReduce Executor**
```rust
use tracing::{instrument, Instrument};

impl MapReduceExecutor {
    #[instrument(
        name = "mapreduce.job.execute",
        skip(self, env),
        fields(
            job.id = %job_id,
            job.total_items = work_items.len(),
            job.max_parallel = map_phase.config.max_parallel,
        )
    )]
    pub async fn execute_instrumented(
        &self,
        map_phase: &MapPhase,
        reduce_phase: Option<&ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        let span = Span::current();
        span.set_attribute(KeyValue::new("job.config", serde_json::to_string(&map_phase.config)?));
        
        // Create child span for map phase
        let map_span = global::tracer("mmm.mapreduce")
            .span_builder("mapreduce.phase.map")
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                KeyValue::new("phase.type", "map"),
                KeyValue::new("phase.parallelism", map_phase.config.max_parallel as i64),
            ])
            .start(&global::tracer("mmm.mapreduce"));
        
        let map_results = self.execute_map_phase_traced(map_phase, work_items, env)
            .instrument(map_span.clone())
            .await?;
        
        map_span.set_attribute(KeyValue::new("phase.results.success", 
            map_results.iter().filter(|r| matches!(r.status, AgentStatus::Success)).count() as i64));
        map_span.end();
        
        // Create child span for reduce phase
        if let Some(reduce) = reduce_phase {
            let reduce_span = global::tracer("mmm.mapreduce")
                .span_builder("mapreduce.phase.reduce")
                .with_kind(SpanKind::Internal)
                .start(&global::tracer("mmm.mapreduce"));
            
            self.execute_reduce_phase_traced(reduce, &map_results, env)
                .instrument(reduce_span.clone())
                .await?;
            
            reduce_span.end();
        }
        
        Ok(map_results)
    }
    
    #[instrument(
        name = "mapreduce.agent.execute",
        skip(self, context, template_steps),
        fields(
            agent.id = %agent_id,
            agent.item_id = %item_id,
            agent.worktree = %context.worktree_name,
        )
    )]
    async fn execute_agent_traced(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        context: &mut AgentContext,
    ) -> Result<AgentResult> {
        let span = Span::current();
        
        // Propagate trace context to agent
        let mut carrier = HashMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&span.context(), &mut carrier);
        });
        
        // Add trace context to agent environment
        for (key, value) in carrier {
            context.variables.insert(format!("TRACE_{}", key), value);
        }
        
        // Execute with tracing
        let result = self.execute_agent_internal(item_id, item, template_steps, context).await;
        
        // Record result in span
        match &result {
            Ok(agent_result) => {
                span.set_status(Status::Ok);
                span.set_attribute(KeyValue::new("agent.duration_ms", 
                    agent_result.duration.as_millis() as i64));
            }
            Err(e) => {
                span.record_error(e);
                span.set_status(Status::error(e.to_string()));
            }
        }
        
        result
    }
}
```

3. **Metrics Collection**
```rust
use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};

pub struct MapReduceMetrics {
    meter: Meter,
    jobs_started: Counter<u64>,
    jobs_completed: Counter<u64>,
    jobs_failed: Counter<u64>,
    agents_started: Counter<u64>,
    agents_completed: Counter<u64>,
    agents_failed: Counter<u64>,
    agent_duration: Histogram<f64>,
    active_agents: UpDownCounter<i64>,
    queue_depth: UpDownCounter<i64>,
}

impl MapReduceMetrics {
    pub fn new() -> Self {
        let meter = global::meter("mmm.mapreduce");
        
        Self {
            jobs_started: meter
                .u64_counter("mapreduce.jobs.started")
                .with_description("Total number of MapReduce jobs started")
                .init(),
            jobs_completed: meter
                .u64_counter("mapreduce.jobs.completed")
                .with_description("Total number of MapReduce jobs completed")
                .init(),
            jobs_failed: meter
                .u64_counter("mapreduce.jobs.failed")
                .with_description("Total number of MapReduce jobs failed")
                .init(),
            agents_started: meter
                .u64_counter("mapreduce.agents.started")
                .with_description("Total number of agents started")
                .init(),
            agents_completed: meter
                .u64_counter("mapreduce.agents.completed")
                .with_description("Total number of agents completed")
                .init(),
            agents_failed: meter
                .u64_counter("mapreduce.agents.failed")
                .with_description("Total number of agents failed")
                .init(),
            agent_duration: meter
                .f64_histogram("mapreduce.agent.duration")
                .with_description("Agent execution duration in seconds")
                .with_unit("s")
                .init(),
            active_agents: meter
                .i64_up_down_counter("mapreduce.agents.active")
                .with_description("Number of currently active agents")
                .init(),
            queue_depth: meter
                .i64_up_down_counter("mapreduce.queue.depth")
                .with_description("Current queue depth")
                .init(),
            meter,
        }
    }
    
    pub fn record_agent_start(&self, job_id: &str) {
        self.agents_started.add(1, &[
            KeyValue::new("job.id", job_id.to_string()),
        ]);
        self.active_agents.add(1, &[]);
    }
    
    pub fn record_agent_complete(&self, job_id: &str, duration: Duration) {
        self.agents_completed.add(1, &[
            KeyValue::new("job.id", job_id.to_string()),
        ]);
        self.agent_duration.record(duration.as_secs_f64(), &[
            KeyValue::new("job.id", job_id.to_string()),
        ]);
        self.active_agents.add(-1, &[]);
    }
}
```

4. **Context Propagation**
```rust
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub trace_flags: u8,
}

impl TraceContext {
    pub fn from_env(env: &HashMap<String, String>) -> Option<Self> {
        // Extract W3C trace context from environment
        let traceparent = env.get("TRACE_traceparent")?;
        let parts: Vec<&str> = traceparent.split('-').collect();
        
        if parts.len() != 4 {
            return None;
        }
        
        Some(Self {
            trace_id: parts[1].to_string(),
            span_id: parts[2].to_string(),
            trace_flags: u8::from_str_radix(parts[3], 16).ok()?,
        })
    }
    
    pub fn to_header(&self) -> String {
        format!("00-{}-{}-{:02x}", self.trace_id, self.span_id, self.trace_flags)
    }
}
```

### Architecture Changes
- Add OpenTelemetry SDK dependencies
- Instrument all async functions
- Add trace context to agent environment
- Integrate metrics collection

### Data Structures
```rust
pub struct SpanAttributes {
    pub job_id: String,
    pub agent_id: Option<String>,
    pub item_id: Option<String>,
    pub operation: String,
    pub custom: HashMap<String, Value>,
}

pub struct TracingConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub sampling_rate: f64,
    pub export_batch_size: usize,
    pub export_interval_ms: u64,
}
```

### APIs and Interfaces
```rust
pub trait Traceable {
    fn span(&self) -> &Span;
    fn add_event(&self, name: &str, attributes: Vec<KeyValue>);
    fn set_attribute(&self, key: &str, value: impl Into<AttributeValue>);
}
```

## Dependencies

- **Prerequisites**: 
  - [51 - Event Logging]
  - [53 - Structured Errors]
- **Affected Components**: 
  - All async functions in MapReduce
  - Agent execution context
  - Error handling paths
- **External Dependencies**: 
  - `opentelemetry` crate
  - `opentelemetry-otlp` crate
  - `tracing-opentelemetry` crate

## Testing Strategy

- **Unit Tests**: 
  - Test span creation
  - Verify context propagation
  - Test attribute setting
  - Validate metrics recording
  
- **Integration Tests**: 
  - Test end-to-end tracing
  - Verify trace export
  - Test sampling logic
  - Validate metrics export
  
- **Performance Tests**: 
  - Measure tracing overhead
  - Test with high span volume
  - Benchmark export performance
  
- **User Acceptance**: 
  - View traces in Jaeger
  - Query metrics in Prometheus
  - Correlate logs with traces

## Documentation Requirements

- **Code Documentation**: 
  - Document span hierarchy
  - Explain context propagation
  - Document custom attributes
  
- **User Documentation**: 
  - Setup guide for Jaeger
  - Prometheus configuration
  - Sampling strategies guide
  
- **Architecture Updates**: 
  - Add tracing architecture diagram
  - Document span relationships

## Implementation Notes

- Use batch span processor for efficiency
- Implement head-based sampling
- Consider tail-based sampling for errors
- Add resource detection for cloud environments
- Implement span links for relationships
- Use semantic conventions for attributes
- Consider trace state for vendor-specific data

## Migration and Compatibility

- OpenTelemetry is opt-in initially
- Existing logging continues to work
- Gradual instrumentation rollout
- Configuration-based enablement
- Backward compatible with plain tracing