---
number: 99
title: Kubernetes MapReduce Orchestration
category: parallel
priority: high
status: draft
dependencies: [97, 98]
created: 2025-01-18
---

# Specification 99: Kubernetes MapReduce Orchestration

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [97 - Basic Kubernetes Job Execution, 98 - Kubernetes Storage and Secrets Management]

## Context

Prodigy's MapReduce workflows currently execute agents sequentially or with limited local parallelism, constraining the ability to process large datasets efficiently. With basic Kubernetes execution and storage management in place, the next critical capability is orchestrating parallel MapReduce operations across multiple Kubernetes pods.

MapReduce workflows require work distribution, parallel agent execution, result aggregation, and failure handling. Kubernetes provides Indexed Jobs for parallel execution, but coordination between agents requires additional infrastructure like work queues and shared storage for intermediate results.

## Objective

Implement comprehensive MapReduce orchestration on Kubernetes, enabling parallel execution of map phase agents across multiple pods with efficient work distribution, result aggregation, and robust failure handling. The solution must scale from 10 to 1000+ parallel agents while maintaining coordination and data consistency.

## Requirements

### Functional Requirements

#### Parallel Job Orchestration
- Generate Kubernetes Indexed Jobs for map phase execution
- Distribute work items across parallel pods efficiently
- Support configurable parallelism (1-1000+ agents)
- Handle dynamic work item assignment
- Coordinate reduce phase after map completion

#### Work Distribution System
- Implement work queue for dynamic task distribution
- Support multiple queue backends (Redis, NATS, SQS)
- Handle work item deduplication and ordering
- Provide progress tracking and monitoring
- Support priority-based work assignment

#### Result Aggregation
- Collect map phase results from all agents
- Store intermediate results in shared storage
- Trigger reduce phase after map completion
- Handle partial failures in map phase
- Support streaming aggregation for large datasets

#### Failure Handling and Recovery
- Detect and restart failed agents
- Implement dead letter queue for failed work items
- Support partial retry strategies
- Handle pod evictions and node failures
- Provide circuit breaker for cascading failures

#### MapReduce Lifecycle Management
- Create and manage all required Kubernetes resources
- Monitor job progress and status
- Clean up resources after completion
- Support job cancellation and timeout
- Provide detailed execution metrics

### Non-Functional Requirements

#### Scalability
- Support 1000+ parallel agents
- Handle 100,000+ work items efficiently
- Scale work queue based on load
- Efficient resource utilization (>80%)
- Minimal overhead per agent (<5%)

#### Performance
- Work item distribution latency <100ms
- Agent startup time <30 seconds
- Result aggregation time proportional to data size
- Queue throughput >10,000 items/second
- End-to-end MapReduce latency <2x sequential time

#### Reliability
- Automatic retry of failed work items (max 3 attempts)
- Graceful handling of partial failures
- Data consistency during failures
- Progress preservation across restarts
- No data loss during pod evictions

## Acceptance Criteria

- [ ] MapReduce workflows execute with `--max-parallel=50` on Kubernetes
- [ ] Work items distributed efficiently across all available agents
- [ ] Failed work items automatically retried up to configured limits
- [ ] Map phase results aggregated correctly in reduce phase
- [ ] Dead letter queue captures permanently failed items
- [ ] Progress tracking shows real-time execution status
- [ ] Pod failures don't lose work items or results
- [ ] Reduce phase waits for all map agents to complete
- [ ] Resource cleanup occurs after workflow completion
- [ ] Supports both Redis and NATS as queue backends

## Technical Details

### Implementation Approach

#### Phase 1: Indexed Jobs and Work Distribution
1. Generate Kubernetes Indexed Jobs for parallel execution
2. Implement work queue client for agent coordination
3. Add work item distribution logic
4. Support configurable parallelism

#### Phase 2: Result Collection and Aggregation
1. Implement shared storage for intermediate results
2. Add result collection mechanisms
3. Trigger reduce phase coordination
4. Handle partial result scenarios

#### Phase 3: Failure Handling and Recovery
1. Add dead letter queue implementation
2. Implement retry logic with exponential backoff
3. Handle pod evictions and restarts
4. Add circuit breaker patterns

#### Phase 4: Advanced Orchestration
1. Support multiple queue backends
2. Add priority-based work assignment
3. Implement streaming aggregation
4. Add advanced monitoring and metrics

### Architecture Changes

```rust
// MapReduce orchestrator module
pub mod mapreduce {
    use std::sync::Arc;
    use tokio::sync::{RwLock, Semaphore};

    pub struct MapReduceOrchestrator {
        kubernetes_client: kube::Client,
        queue_client: Arc<dyn WorkQueue>,
        storage_client: Arc<dyn ResultStorage>,
        config: MapReduceConfig,
    }

    impl MapReduceOrchestrator {
        pub async fn execute_mapreduce(
            &self,
            workflow: &MapReduceWorkflow,
        ) -> Result<MapReduceResult> {
            // 1. Setup infrastructure (queue, storage, secrets)
            let infrastructure = self.setup_infrastructure(workflow).await?;

            // 2. Populate work queue with items
            self.populate_work_queue(&workflow.input_items).await?;

            // 3. Launch map phase jobs
            let map_jobs = self.launch_map_phase(workflow, &infrastructure).await?;

            // 4. Monitor map phase completion
            self.monitor_map_phase(&map_jobs).await?;

            // 5. Execute reduce phase
            let reduce_result = self.execute_reduce_phase(workflow, &infrastructure).await?;

            // 6. Cleanup resources
            self.cleanup_infrastructure(&infrastructure).await?;

            Ok(reduce_result)
        }
    }

    // Work queue abstraction
    pub trait WorkQueue: Send + Sync {
        async fn enqueue_items(&self, items: &[WorkItem]) -> Result<()>;
        async fn dequeue_item(&self) -> Result<Option<WorkItem>>;
        async fn ack_item(&self, item: &WorkItem) -> Result<()>;
        async fn nack_item(&self, item: &WorkItem) -> Result<()>;
        async fn get_queue_depth(&self) -> Result<usize>;
    }

    // Redis queue implementation
    pub struct RedisWorkQueue {
        client: redis::Client,
        queue_name: String,
        dlq_name: String,
    }

    impl WorkQueue for RedisWorkQueue {
        async fn enqueue_items(&self, items: &[WorkItem]) -> Result<()> {
            let mut conn = self.client.get_async_connection().await?;
            for item in items {
                let serialized = serde_json::to_string(item)?;
                redis::cmd("LPUSH")
                    .arg(&self.queue_name)
                    .arg(serialized)
                    .query_async(&mut conn)
                    .await?;
            }
            Ok(())
        }

        async fn dequeue_item(&self) -> Result<Option<WorkItem>> {
            let mut conn = self.client.get_async_connection().await?;
            let result: Option<String> = redis::cmd("RPOP")
                .arg(&self.queue_name)
                .query_async(&mut conn)
                .await?;

            match result {
                Some(data) => {
                    let item: WorkItem = serde_json::from_str(&data)?;
                    Ok(Some(item))
                }
                None => Ok(None),
            }
        }
    }
}

// Enhanced workflow structure for MapReduce
#[derive(Debug, Clone)]
pub struct MapReduceWorkflow {
    pub name: String,
    pub input_items: Vec<WorkItem>,
    pub map_agent_template: AgentTemplate,
    pub reduce_commands: Vec<Command>,
    pub max_parallel: u32,
    pub max_retries: u32,
    pub timeout: Duration,
    pub queue_config: QueueConfig,
}

#[derive(Debug, Clone)]
pub struct WorkItem {
    pub id: String,
    pub data: serde_json::Value,
    pub metadata: HashMap<String, String>,
    pub attempt_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub backend: QueueBackend,
    pub connection_string: Option<String>,
    pub queue_name: String,
    pub dlq_name: String,
    pub visibility_timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum QueueBackend {
    Redis,
    Nats,
    Sqs,
    InMemory, // For testing
}
```

### Indexed Job Generation

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: prodigy-mapreduce-{{ workflow_id }}
  namespace: {{ namespace }}
  labels:
    app: prodigy
    workflow: {{ workflow_name }}
    phase: map
spec:
  parallelism: {{ max_parallel }}
  completions: {{ max_parallel }}  # Fixed pool of workers
  completionMode: Indexed
  backoffLimit: {{ max_retries }}
  ttlSecondsAfterFinished: 3600
  template:
    metadata:
      labels:
        app: prodigy
        workflow: {{ workflow_name }}
        phase: map
    spec:
      restartPolicy: Never
      containers:
      - name: map-agent
        image: {{ image }}
        command: ["prodigy", "agent", "mapreduce-worker"]
        env:
        - name: JOB_COMPLETION_INDEX
          valueFrom:
            fieldRef:
              fieldPath: metadata.annotations['batch.kubernetes.io/job-completion-index']
        - name: WORKFLOW_ID
          value: "{{ workflow_id }}"
        - name: QUEUE_URL
          value: "{{ queue_url }}"
        - name: QUEUE_NAME
          value: "{{ queue_name }}"
        - name: DLQ_NAME
          value: "{{ dlq_name }}"
        - name: MAX_RETRIES
          value: "{{ max_retries }}"
        - name: CLAUDE_API_KEY
          valueFrom:
            secretKeyRef:
              name: prodigy-secrets-{{ workflow_id }}
              key: claude-api-key
        resources:
          requests:
            cpu: "0.5"
            memory: "1Gi"
          limits:
            cpu: "2"
            memory: "4Gi"
        volumeMounts:
        - name: shared-storage
          mountPath: /shared
        - name: agent-template
          mountPath: /config
      volumes:
      - name: shared-storage
        persistentVolumeClaim:
          claimName: prodigy-shared-{{ workflow_id }}
      - name: agent-template
        configMap:
          name: prodigy-agent-template-{{ workflow_id }}
```

### Work Queue Infrastructure

```yaml
# Redis StatefulSet for work queue
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: prodigy-redis-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  serviceName: prodigy-redis-{{ workflow_id }}
  replicas: 1
  selector:
    matchLabels:
      app: prodigy-redis
      workflow: {{ workflow_id }}
  template:
    metadata:
      labels:
        app: prodigy-redis
        workflow: {{ workflow_id }}
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        ports:
        - containerPort: 6379
        resources:
          requests:
            cpu: "0.1"
            memory: "256Mi"
          limits:
            cpu: "0.5"
            memory: "1Gi"
        volumeMounts:
        - name: redis-data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: redis-data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 10Gi

---
apiVersion: v1
kind: Service
metadata:
  name: prodigy-redis-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  selector:
    app: prodigy-redis
    workflow: {{ workflow_id }}
  ports:
  - port: 6379
    targetPort: 6379
  clusterIP: None
```

### MapReduce Agent Logic

```rust
// Agent implementation for MapReduce workers
pub struct MapReduceAgent {
    queue_client: Arc<dyn WorkQueue>,
    storage_client: Arc<dyn ResultStorage>,
    agent_template: AgentTemplate,
    worker_id: String,
}

impl MapReduceAgent {
    pub async fn run_worker(&self) -> Result<()> {
        loop {
            // Dequeue work item
            match self.queue_client.dequeue_item().await? {
                Some(work_item) => {
                    match self.process_work_item(&work_item).await {
                        Ok(result) => {
                            // Store result and ack item
                            self.storage_client.store_result(&work_item.id, &result).await?;
                            self.queue_client.ack_item(&work_item).await?;
                        }
                        Err(e) => {
                            // Handle failure
                            if work_item.attempt_count >= work_item.max_retries {
                                // Send to DLQ
                                self.queue_client.send_to_dlq(&work_item).await?;
                            } else {
                                // Retry with backoff
                                self.queue_client.nack_item(&work_item).await?;
                            }
                        }
                    }
                }
                None => {
                    // Check if workflow is complete
                    if self.is_workflow_complete().await? {
                        break;
                    }
                    // Wait before polling again
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        Ok(())
    }

    async fn process_work_item(&self, item: &WorkItem) -> Result<WorkResult> {
        // Execute agent template with work item data
        let context = create_work_context(item);
        let result = self.agent_template.execute(context).await?;
        Ok(result)
    }
}
```

### Result Storage and Aggregation

```rust
// Result storage abstraction
pub trait ResultStorage: Send + Sync {
    async fn store_result(&self, item_id: &str, result: &WorkResult) -> Result<()>;
    async fn get_all_results(&self) -> Result<Vec<WorkResult>>;
    async fn get_results_count(&self) -> Result<usize>;
    async fn cleanup_results(&self) -> Result<()>;
}

// Shared storage implementation
pub struct SharedFileStorage {
    base_path: PathBuf,
}

impl ResultStorage for SharedFileStorage {
    async fn store_result(&self, item_id: &str, result: &WorkResult) -> Result<()> {
        let result_path = self.base_path.join(format!("{}.json", item_id));
        let serialized = serde_json::to_string(result)?;
        tokio::fs::write(result_path, serialized).await?;
        Ok(())
    }

    async fn get_all_results(&self) -> Result<Vec<WorkResult>> {
        let mut results = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension() == Some(std::ffi::OsStr::new("json")) {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                let result: WorkResult = serde_json::from_str(&content)?;
                results.push(result);
            }
        }

        Ok(results)
    }
}
```

## Dependencies

### Prerequisites
- Specification 97: Basic Kubernetes Job Execution
- Specification 98: Kubernetes Storage and Secrets Management

### Affected Components
- MapReduce executor: New orchestrator for parallel execution
- Agent: Enhanced with work queue integration
- CLI: Extended with MapReduce-specific flags
- Monitoring: Progress tracking and metrics collection

### External Dependencies
- Redis or NATS for work queue
- Kubernetes API with Indexed Jobs support (v1.21+)
- Shared storage for result aggregation

## Testing Strategy

### Unit Tests
- Work queue operations (enqueue, dequeue, ack/nack)
- Result storage and retrieval
- Job manifest generation for Indexed Jobs
- Failure handling logic

### Integration Tests
- End-to-end MapReduce workflow execution
- Queue persistence across pod restarts
- Result aggregation from multiple agents
- Dead letter queue functionality

### Performance Tests
- Throughput with varying parallelism levels
- Queue performance under load
- Memory usage with large result sets
- Scaling behavior with 100+ agents

### Chaos Tests
- Pod evictions during execution
- Queue service failures
- Storage unavailability
- Network partitions

## Documentation Requirements

### User Documentation
- MapReduce workflow configuration guide
- Queue backend setup instructions
- Performance tuning recommendations
- Troubleshooting parallel execution issues

### Architecture Documentation
- MapReduce orchestration flow
- Work distribution algorithms
- Failure handling strategies
- Monitoring and observability setup

## Implementation Notes

### Queue Backend Selection
- **Redis**: Simple, fast, good for moderate scale
- **NATS JetStream**: Cloud-native, better for high scale
- **AWS SQS**: Managed service, good for AWS deployments
- **In-Memory**: Testing and development only

### Failure Handling Strategy
1. Transient failures: Automatic retry with exponential backoff
2. Persistent failures: Send to dead letter queue after max retries
3. Agent failures: Kubernetes restarts pods automatically
4. Infrastructure failures: Circuit breaker prevents cascade

### Performance Optimization
- Use connection pooling for queue clients
- Batch work item operations when possible
- Implement result streaming for large datasets
- Cache frequently accessed data

## Migration and Compatibility

### Backward Compatibility
- Single-agent workflows continue to work without changes
- MapReduce workflows degrade gracefully to sequential execution
- Existing workflow YAML format unchanged

### Migration Path
1. Add MapReduce orchestrator alongside existing executors
2. Test with simple parallel workflows
3. Enable queue backends gradually
4. Migrate complex MapReduce workflows
5. Optimize based on production usage

### Breaking Changes
None - MapReduce orchestration is additive functionality.