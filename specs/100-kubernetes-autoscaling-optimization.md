---
number: 100
title: Kubernetes Autoscaling and Cost Optimization
category: optimization
priority: medium
status: draft
dependencies: [97, 98, 99]
created: 2025-01-18
---

# Specification 100: Kubernetes Autoscaling and Cost Optimization

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [97 - Basic Kubernetes Job Execution, 98 - Kubernetes Storage and Secrets Management, 99 - Kubernetes MapReduce Orchestration]

## Context

With Kubernetes execution, storage management, and MapReduce orchestration in place, the focus shifts to production-ready features that optimize cost and performance. Running workflows in managed Kubernetes can be expensive without proper resource optimization, autoscaling, and cost controls.

Key optimization opportunities include: leveraging spot/preemptible instances for 60-80% cost reduction, implementing cluster autoscaling to scale resources up and down based on demand, right-sizing resource requests and limits, monitoring and alerting for cost and performance issues, and implementing intelligent scheduling strategies.

## Objective

Implement comprehensive autoscaling and cost optimization features for Kubernetes-based workflow execution, enabling automatic resource scaling, cost-effective spot instance usage, intelligent resource allocation, and detailed cost tracking. The system must balance cost efficiency with reliability and performance requirements.

## Requirements

### Functional Requirements

#### Cluster Autoscaling Integration
- Integration with Kubernetes Cluster Autoscaler
- Support for Karpenter (AWS) and GKE Autopilot
- Node pool configuration for different workload types
- Automatic scale-down when workflows complete
- Multi-zone and multi-instance-type support

#### Spot Instance Management
- Spot/preemptible instance scheduling for batch workloads
- Graceful handling of spot instance interruptions
- Mixed instance type clusters (spot + on-demand)
- Pod disruption budgets for reliability
- Automatic migration on spot termination

#### Resource Optimization
- Horizontal Pod Autoscaler (HPA) for queue-based scaling
- Vertical Pod Autoscaler (VPA) recommendations
- Resource request/limit optimization based on usage
- Bin-packing optimization for node utilization
- Quality of Service (QoS) class management

#### Cost Tracking and Reporting
- Per-workflow cost attribution
- Resource usage metrics and dashboards
- Cost alerts and budgets
- Optimization recommendations
- Historical cost analysis

#### Monitoring and Observability
- Prometheus metrics for autoscaling decisions
- Grafana dashboards for cost and performance
- Custom metrics for business logic scaling
- Distributed tracing for workflow execution
- Log aggregation and analysis

### Non-Functional Requirements

#### Cost Efficiency
- 60-80% cost reduction through spot instances
- >85% node utilization efficiency
- Automatic scale-down within 5 minutes of completion
- Cost per workflow execution tracking
- Budget-based execution controls

#### Performance
- Autoscaling response time <2 minutes
- Node provisioning time <3 minutes
- Minimal impact on workflow execution time
- Predictable performance despite spot interruptions
- Efficient resource bin-packing

#### Reliability
- Automatic fallback to on-demand instances
- Pod disruption budget compliance
- Graceful handling of node terminations
- Data persistence during interruptions
- Circuit breaker for cost overruns

## Acceptance Criteria

- [ ] Workflows automatically scale cluster nodes based on demand
- [ ] Spot instances used for >80% of batch workloads
- [ ] Node scale-down occurs within 5 minutes of workflow completion
- [ ] Cost per workflow tracked and reported in dashboards
- [ ] HPA scales agents based on work queue depth
- [ ] VPA provides resource optimization recommendations
- [ ] Pod disruption budgets prevent service disruption
- [ ] Cost alerts trigger when budgets exceeded
- [ ] Metrics exported to Prometheus for monitoring
- [ ] Grafana dashboards show cost and performance trends

## Technical Details

### Implementation Approach

#### Phase 1: Cluster Autoscaling Configuration
1. Configure cluster autoscaler for node scaling
2. Set up node pools with appropriate instance types
3. Implement spot instance node groups
4. Add node affinity and taints for workload separation

#### Phase 2: Pod Autoscaling
1. Implement HPA based on queue depth metrics
2. Configure VPA for resource recommendations
3. Add custom metrics for business logic scaling
4. Implement KEDA for advanced autoscaling scenarios

#### Phase 3: Cost Tracking and Monitoring
1. Implement cost attribution per workflow
2. Add Prometheus metrics for cost and usage
3. Create Grafana dashboards for visualization
4. Set up alerting for cost and performance issues

#### Phase 4: Advanced Optimization
1. Implement predictive scaling
2. Add cost optimization recommendations
3. Implement budget-based execution controls
4. Add machine learning for usage pattern prediction

### Architecture Changes

```rust
// Autoscaling configuration module
pub mod autoscaling {
    use prometheus::{Counter, Gauge, Histogram};

    pub struct AutoscalingManager {
        kubernetes_client: kube::Client,
        metrics_client: PrometheusClient,
        cost_tracker: Arc<CostTracker>,
        config: AutoscalingConfig,
    }

    impl AutoscalingManager {
        pub async fn configure_autoscaling(
            &self,
            workflow: &Workflow,
        ) -> Result<AutoscalingResources> {
            // Configure HPA based on queue metrics
            let hpa = self.create_hpa_for_workflow(workflow).await?;

            // Set up node affinity for spot instances
            let node_affinity = self.create_spot_affinity(workflow)?;

            // Configure pod disruption budget
            let pdb = self.create_pod_disruption_budget(workflow).await?;

            Ok(AutoscalingResources { hpa, node_affinity, pdb })
        }

        pub async fn monitor_costs(&self, workflow_id: &str) -> Result<CostMetrics> {
            self.cost_tracker.get_workflow_costs(workflow_id).await
        }
    }

    // Cost tracking implementation
    pub struct CostTracker {
        metrics_storage: Arc<dyn MetricsStorage>,
        pricing_data: Arc<CloudPricingData>,
    }

    impl CostTracker {
        pub async fn track_resource_usage(
            &self,
            workflow_id: &str,
            resource_usage: &ResourceUsage,
        ) -> Result<()> {
            let cost = self.calculate_cost(resource_usage).await?;
            self.metrics_storage.store_cost_metric(workflow_id, cost).await?;
            Ok(())
        }

        pub async fn get_workflow_costs(&self, workflow_id: &str) -> Result<CostMetrics> {
            self.metrics_storage.get_cost_metrics(workflow_id).await
        }
    }
}

// Enhanced node configuration
#[derive(Debug, Clone)]
pub struct NodePoolConfig {
    pub name: String,
    pub instance_types: Vec<String>,
    pub spot_instances: bool,
    pub min_nodes: u32,
    pub max_nodes: u32,
    pub taints: Vec<NodeTaint>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SpotInstanceConfig {
    pub enabled: bool,
    pub max_price: Option<f64>,
    pub interruption_handling: InterruptionHandling,
    pub fallback_to_ondemand: bool,
}

#[derive(Debug, Clone)]
pub enum InterruptionHandling {
    Graceful { drain_timeout: Duration },
    Immediate,
    Retry { max_attempts: u32 },
}

// Cost and metrics structures
#[derive(Debug, Clone)]
pub struct CostMetrics {
    pub workflow_id: String,
    pub compute_cost: f64,
    pub storage_cost: f64,
    pub network_cost: f64,
    pub total_cost: f64,
    pub currency: String,
    pub time_period: Duration,
}

#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_hours: f64,
    pub memory_gb_hours: f64,
    pub storage_gb_hours: f64,
    pub network_gb: f64,
    pub instance_type: String,
    pub spot_instance: bool,
}
```

### Horizontal Pod Autoscaler (HPA) Configuration

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: prodigy-mapreduce-hpa-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  scaleTargetRef:
    apiVersion: batch/v1
    kind: Job
    name: prodigy-mapreduce-{{ workflow_id }}
  minReplicas: 1
  maxReplicas: {{ max_parallel }}
  metrics:
  - type: External
    external:
      metric:
        name: redis_queue_depth
        selector:
          matchLabels:
            queue_name: "{{ queue_name }}"
      target:
        type: AverageValue
        averageValue: "10"  # Scale up when >10 items per pod
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 100
        periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
```

### Spot Instance Job Configuration

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: prodigy-spot-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  template:
    spec:
      # Prefer spot instances
      nodeSelector:
        node.kubernetes.io/lifecycle: spot
        workload-type: batch

      # Tolerate spot instance taints
      tolerations:
      - key: "spot"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"
      - key: "node.kubernetes.io/lifecycle"
        operator: "Equal"
        value: "spot"
        effect: "NoSchedule"

      # Pod affinity for cost optimization
      affinity:
        nodeAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            preference:
              matchExpressions:
              - key: node.kubernetes.io/lifecycle
                operator: In
                values: ["spot"]
          - weight: 50
            preference:
              matchExpressions:
              - key: node.kubernetes.io/instance-type
                operator: In
                values: ["m5.large", "m5.xlarge"]  # Cost-effective instances

      # Graceful termination handling
      terminationGracePeriodSeconds: 300

      containers:
      - name: prodigy-agent
        # ... container spec ...
        lifecycle:
          preStop:
            exec:
              command: ["/bin/sh", "-c"]
              args:
              - |
                # Graceful shutdown on spot termination
                echo "Received termination signal, saving state..."
                prodigy agent checkpoint --workflow-id=${WORKFLOW_ID}
                exit 0

        # Resource requests optimized for spot instances
        resources:
          requests:
            cpu: "0.9"      # Slightly under 1 CPU for better bin-packing
            memory: "1.8Gi" # Slightly under 2Gi for better bin-packing
          limits:
            cpu: "2"
            memory: "4Gi"
```

### Pod Disruption Budget

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: prodigy-mapreduce-pdb-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  minAvailable: 70%  # Keep at least 70% of pods running
  selector:
    matchLabels:
      app: prodigy
      workflow: {{ workflow_name }}
      phase: map
```

### KEDA Autoscaler Configuration

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledJob
metadata:
  name: prodigy-keda-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  jobTargetRef:
    parallelism: 1
    completions: 1
    template:
      # Job template spec
  pollingInterval: 30
  successfulJobsHistoryLimit: 3
  failedJobsHistoryLimit: 1
  maxReplicaCount: {{ max_parallel }}
  scalingStrategy:
    strategy: "gradual"
  triggers:
  - type: redis
    metadata:
      address: prodigy-redis-{{ workflow_id }}:6379
      listName: "{{ queue_name }}"
      listLength: "5"  # Scale when queue depth > 5
```

### Cost Tracking Metrics

```rust
// Prometheus metrics for cost tracking
lazy_static! {
    static ref WORKFLOW_COST_TOTAL: Counter = Counter::new(
        "prodigy_workflow_cost_total",
        "Total cost of workflow execution"
    ).expect("metric can be created");

    static ref NODE_UTILIZATION: Gauge = Gauge::new(
        "prodigy_node_utilization",
        "Current node resource utilization"
    ).expect("metric can be created");

    static ref SPOT_INSTANCE_SAVINGS: Gauge = Gauge::new(
        "prodigy_spot_savings_percent",
        "Percentage savings from spot instance usage"
    ).expect("metric can be created");

    static ref WORKFLOW_DURATION: Histogram = Histogram::new(
        "prodigy_workflow_duration_seconds",
        "Duration of workflow execution"
    ).expect("metric can be created");
}

// Cost calculation implementation
impl CostCalculator {
    pub fn calculate_compute_cost(&self, usage: &ResourceUsage) -> f64 {
        let hourly_rate = if usage.spot_instance {
            self.get_spot_price(&usage.instance_type) * 0.3  // ~70% discount
        } else {
            self.get_ondemand_price(&usage.instance_type)
        };

        hourly_rate * usage.cpu_hours
    }

    pub fn calculate_storage_cost(&self, usage: &ResourceUsage) -> f64 {
        // EBS/GCP Persistent Disk pricing
        let storage_rate = 0.10; // $0.10 per GB-month
        usage.storage_gb_hours * storage_rate / (24.0 * 30.0) // Convert to hourly
    }

    pub fn calculate_network_cost(&self, usage: &ResourceUsage) -> f64 {
        // Data transfer costs
        let egress_rate = 0.09; // $0.09 per GB for first 10TB
        usage.network_gb * egress_rate
    }
}
```

### Grafana Dashboard Configuration

```json
{
  "dashboard": {
    "title": "Prodigy Kubernetes Cost and Performance",
    "panels": [
      {
        "title": "Workflow Cost Over Time",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(prodigy_workflow_cost_total[5m])",
            "legendFormat": "Cost per minute"
          }
        ]
      },
      {
        "title": "Node Utilization",
        "type": "stat",
        "targets": [
          {
            "expr": "avg(prodigy_node_utilization)",
            "legendFormat": "Average Utilization"
          }
        ]
      },
      {
        "title": "Spot Instance Savings",
        "type": "gauge",
        "targets": [
          {
            "expr": "prodigy_spot_savings_percent",
            "legendFormat": "Savings %"
          }
        ]
      },
      {
        "title": "Active Workflows",
        "type": "stat",
        "targets": [
          {
            "expr": "count(prodigy_workflow_active)",
            "legendFormat": "Active Workflows"
          }
        ]
      }
    ]
  }
}
```

## Dependencies

### Prerequisites
- Specification 97: Basic Kubernetes Job Execution
- Specification 98: Kubernetes Storage and Secrets Management
- Specification 99: Kubernetes MapReduce Orchestration

### Affected Components
- Kubernetes executor: Enhanced with autoscaling configuration
- Monitoring: Cost and performance metrics collection
- CLI: Flags for autoscaling and cost optimization
- Configuration: Autoscaling and cost control settings

### External Dependencies
- Kubernetes Cluster Autoscaler or Karpenter
- Prometheus for metrics collection
- Grafana for dashboards and visualization
- KEDA for advanced autoscaling (optional)
- Cloud provider cost APIs (AWS/GCP/Azure)

## Testing Strategy

### Unit Tests
- Cost calculation algorithms
- Autoscaling configuration generation
- Metrics collection and aggregation
- Resource optimization recommendations

### Integration Tests
- HPA scaling behavior with load
- Spot instance interruption handling
- Cost tracking accuracy
- Dashboard metric updates

### Performance Tests
- Autoscaling response time under load
- Cost efficiency with different instance types
- Node utilization optimization
- Scaling behavior with burst workloads

### Chaos Tests
- Spot instance termination scenarios
- Node failure during autoscaling
- Cost overrun protection
- Metrics collection failures

## Documentation Requirements

### User Documentation
- Autoscaling configuration guide
- Cost optimization best practices
- Spot instance usage recommendations
- Monitoring and alerting setup

### Operations Documentation
- Cost tracking and budgeting procedures
- Performance tuning guidelines
- Troubleshooting autoscaling issues
- Dashboard and alerting configuration

## Implementation Notes

### Autoscaling Best Practices
- Use multiple scaling metrics for robustness
- Set appropriate stabilization windows
- Configure pod disruption budgets
- Monitor scaling events and costs

### Cost Optimization Strategies
- Use spot instances for all batch workloads
- Right-size resource requests based on profiling
- Implement automatic scale-down policies
- Use reserved instances for baseline capacity

### Monitoring Considerations
- Export custom metrics for business logic
- Set up cost alerts and budgets
- Monitor autoscaling behavior and tune parameters
- Track cost trends and optimization opportunities

## Migration and Compatibility

### Backward Compatibility
- Autoscaling features are optional and disabled by default
- Existing workflows continue to work without changes
- Cost tracking is passive and doesn't affect execution

### Migration Path
1. Deploy monitoring and metrics collection
2. Configure cluster autoscaler and node pools
3. Enable spot instances gradually
4. Set up cost tracking and dashboards
5. Optimize based on collected metrics

### Breaking Changes
None - autoscaling and optimization features are additive.