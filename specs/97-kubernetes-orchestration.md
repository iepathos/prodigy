---
number: 97
title: Kubernetes Orchestration for Scalable Workflow Execution
category: parallel
priority: high
status: draft
dependencies: [94, 95, 96]
created: 2025-01-18
---

# Specification 97: Kubernetes Orchestration for Scalable Workflow Execution

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [94 - PostgreSQL Storage Backend, 95 - Storage Backend Configuration, 96 - Container Runtime Integration]

## Context

Prodigy's current execution model using local git worktrees and process execution becomes resource-constrained when running multiple workflows concurrently. As workflows become more complex and MapReduce jobs require more parallel agents, the need for scalable, cloud-based execution becomes critical. Kubernetes provides the ideal platform for orchestrating containerized workflows with automatic scaling, resource management, failure recovery, and multi-region deployment capabilities.

The transition to Kubernetes enables Prodigy to burst from local execution to cloud resources on-demand, support massive parallelism for MapReduce operations, provide consistent execution environments across development and production, and lay the foundation for eventual platform-as-a-service offerings. By leveraging managed Kubernetes services (EKS, GKE, AKS), we can focus on workflow orchestration rather than infrastructure management.

## Objective

Implement comprehensive Kubernetes orchestration support for Prodigy, enabling workflows to execute as Kubernetes Jobs with automatic scaling, resource management, and cloud-burst capabilities. The system must support both local Kubernetes clusters (Minikube, Kind) for development and managed Kubernetes services for production, while maintaining backward compatibility with local execution modes.

## Requirements

### Functional Requirements

#### Core Kubernetes Integration
- Generate Kubernetes Job manifests from workflow definitions
- Submit and manage Jobs via Kubernetes API
- Stream logs and events from running pods
- Handle job completion, failure, and timeout scenarios
- Support both batch/v1 Jobs and custom CRDs

#### Cluster Management
- Connect to multiple Kubernetes clusters simultaneously
- Support kubeconfig-based authentication
- Enable cluster selection via CLI flags or configuration
- Automatic cluster health checking and failover
- Support for managed services (EKS, GKE, AKS, Autopilot)

#### Resource Orchestration
- Pod resource requests and limits configuration
- Node affinity and pod anti-affinity rules
- Spot/preemptible instance support for cost optimization
- Horizontal pod autoscaling for MapReduce agents
- Priority classes for workflow scheduling

#### Storage Integration
- PersistentVolumeClaims for shared workflow data
- ConfigMaps for workflow definitions
- Secrets management for credentials (Claude API keys, Git tokens)
- Volume mounting for git repositories and artifacts
- Support for ReadWriteMany volumes for parallel access

#### Networking and Service Discovery
- Service creation for inter-pod communication
- Ingress/LoadBalancer for external access
- Network policies for security isolation
- DNS-based service discovery
- Support for service mesh integration (optional)

#### MapReduce Optimization
- Parallel job execution with configurable parallelism
- Work queue implementation (Redis/NATS in-cluster)
- Dynamic agent scaling based on queue depth
- Failed item retry with exponential backoff
- Dead letter queue persistence in PVCs

### Non-Functional Requirements

#### Performance
- Job submission latency <1 second
- Support 1000+ concurrent pods
- Pod startup time <30 seconds with image caching
- Log streaming latency <500ms
- Automatic cleanup of completed jobs

#### Scalability
- Scale from 0 to 100 nodes automatically
- Support multi-region deployments
- Handle 10,000+ jobs per day
- Efficient resource bin-packing
- Cost optimization through spot instances

#### Reliability
- Automatic job restart on node failure
- Checkpoint persistence for long-running workflows
- Graceful handling of cluster upgrades
- Circuit breaker for API rate limiting
- Multi-cluster failover capability

#### Security
- RBAC with minimal required permissions
- Pod security policies/standards enforcement
- Network segmentation per workflow/tenant
- Secrets encryption at rest
- Audit logging of all operations

## Acceptance Criteria

- [ ] Workflows can execute as Kubernetes Jobs with `--cluster` flag
- [ ] MapReduce workflows spawn parallel pods up to configured limits
- [ ] Logs stream in real-time from running pods to CLI
- [ ] Failed jobs automatically retry with backoff
- [ ] Cluster autoscaler provisions nodes on-demand
- [ ] Spot instances reduce costs by >60% for batch work
- [ ] Git repositories accessible via PVCs or ephemeral clones
- [ ] Secrets (Claude key, Git tokens) injected securely
- [ ] Resource limits prevent runaway containers
- [ ] Completed jobs and pods cleaned up automatically
- [ ] Works with Minikube locally and EKS/GKE in production
- [ ] Backward compatible with local execution mode
- [ ] Monitoring via Prometheus metrics and Grafana dashboards
- [ ] Cost tracking per workflow execution

## Technical Details

### Implementation Approach

#### Phase 1: Basic Kubernetes Job Execution
1. Add Kubernetes client library (kube-rs or k8s-openapi)
2. Implement Job manifest generation from workflow definitions
3. Add `--cluster` flag to CLI with kubeconfig support
4. Basic job submission and status tracking
5. Log streaming from pod containers

#### Phase 2: Storage and Secrets Management
1. PVC creation for shared workflow data
2. ConfigMap generation for workflow definitions
3. Secret creation for credentials with rotation support
4. Volume mount configuration in pod specs
5. Git repository cloning strategies (init containers vs. gitRepo volumes)

#### Phase 3: MapReduce Orchestration
1. Parallel job creation with indexed jobs
2. Work queue implementation (Redis StatefulSet)
3. Dynamic scaling based on queue depth (HPA/KEDA)
4. DLQ persistence and reprocessing
5. Reduce phase coordination via leader election

#### Phase 4: Production Features
1. Cluster autoscaler integration (Karpenter preferred)
2. Spot instance support with interruption handling
3. Multi-cluster management and failover
4. Prometheus metrics and OpenTelemetry tracing
5. Cost allocation and chargebacks

### Architecture Changes

```rust
// New Kubernetes executor module
pub mod kubernetes {
    pub struct K8sExecutor {
        client: kube::Client,
        namespace: String,
        cluster_config: ClusterConfig,
    }

    impl Executor for K8sExecutor {
        async fn submit_workflow(&self, workflow: &Workflow) -> Result<JobId>;
        async fn get_status(&self, job_id: &JobId) -> Result<JobStatus>;
        async fn stream_logs(&self, job_id: &JobId) -> Result<LogStream>;
        async fn cancel_job(&self, job_id: &JobId) -> Result<()>;
    }
}

// Extended CLI arguments
pub struct KubernetesArgs {
    /// Target Kubernetes cluster
    #[arg(long)]
    cluster: Option<String>,

    /// Kubernetes namespace
    #[arg(long, default = "prodigy")]
    namespace: String,

    /// Enable spot instances
    #[arg(long)]
    use_spot: bool,

    /// Maximum parallel pods
    #[arg(long, default = 10)]
    max_parallel: u32,
}
```

### Data Structures

```yaml
# Generated Job manifest example
apiVersion: batch/v1
kind: Job
metadata:
  name: prodigy-workflow-{{ workflow_id }}
  namespace: prodigy
  labels:
    app: prodigy
    workflow: {{ workflow_name }}
    mode: mapreduce
spec:
  parallelism: {{ max_parallel }}
  completions: {{ total_items }}
  backoffLimit: 3
  ttlSecondsAfterFinished: 3600
  template:
    metadata:
      labels:
        app: prodigy
        workflow: {{ workflow_name }}
      annotations:
        prometheus.io/scrape: "true"
    spec:
      serviceAccountName: prodigy-agent
      nodeSelector:
        workload: batch
        node.kubernetes.io/lifecycle: spot  # For cost optimization
      tolerations:
      - key: "spot"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"
      containers:
      - name: prodigy-agent
        image: {{ registry }}/prodigy-agent:{{ version }}
        imagePullPolicy: IfNotPresent
        resources:
          requests:
            cpu: "1"
            memory: "2Gi"
          limits:
            cpu: "4"
            memory: "8Gi"
        env:
        - name: WORKFLOW_ID
          value: "{{ workflow_id }}"
        - name: WORK_ITEM_INDEX
          valueFrom:
            fieldRef:
              fieldPath: metadata.annotations['batch.kubernetes.io/job-completion-index']
        - name: CLAUDE_API_KEY
          valueFrom:
            secretKeyRef:
              name: prodigy-secrets
              key: claude-api-key
        - name: GIT_TOKEN
          valueFrom:
            secretKeyRef:
              name: prodigy-secrets
              key: git-token
        volumeMounts:
        - name: workspace
          mountPath: /workspace
        - name: workflow-config
          mountPath: /config
      initContainers:
      - name: git-clone
        image: alpine/git
        command: ["sh", "-c"]
        args:
        - |
          git clone --depth 1 https://oauth2:${GIT_TOKEN}@github.com/{{ repo }} /workspace
        env:
        - name: GIT_TOKEN
          valueFrom:
            secretKeyRef:
              name: prodigy-secrets
              key: git-token
        volumeMounts:
        - name: workspace
          mountPath: /workspace
      volumes:
      - name: workspace
        emptyDir:
          sizeLimit: 10Gi
      - name: workflow-config
        configMap:
          name: workflow-{{ workflow_id }}
      restartPolicy: OnFailure
```

### APIs and Interfaces

```rust
// Kubernetes executor interface
pub trait KubernetesExecutor {
    /// Submit workflow as Kubernetes Job
    async fn submit_workflow(
        &self,
        workflow: &Workflow,
        options: &K8sOptions,
    ) -> Result<K8sJob>;

    /// Monitor job status
    async fn watch_job(
        &self,
        job: &K8sJob,
    ) -> Result<JobStatusStream>;

    /// Stream logs from pods
    async fn stream_logs(
        &self,
        job: &K8sJob,
        follow: bool,
    ) -> Result<LogStream>;

    /// Scale job parallelism
    async fn scale_job(
        &self,
        job: &K8sJob,
        parallelism: u32,
    ) -> Result<()>;

    /// Clean up job resources
    async fn cleanup_job(
        &self,
        job: &K8sJob,
    ) -> Result<()>;
}

// Configuration for Kubernetes execution
pub struct K8sOptions {
    pub cluster: Option<String>,
    pub namespace: String,
    pub service_account: Option<String>,
    pub image: String,
    pub image_pull_policy: ImagePullPolicy,
    pub resources: ResourceRequirements,
    pub node_selector: HashMap<String, String>,
    pub tolerations: Vec<Toleration>,
    pub spot_instances: bool,
    pub max_parallel: u32,
    pub timeout: Duration,
}
```

## Dependencies

### Prerequisites
- Specification 94: PostgreSQL Storage Backend (for job state persistence)
- Specification 95: Storage Backend Configuration (for cluster-specific configs)
- Specification 96: Container Runtime Integration (for container image management)

### Affected Components
- CLI: New flags for cluster selection and Kubernetes options
- Executor: New Kubernetes executor implementation
- Configuration: Cluster connection settings
- Storage: Job state and logs in database
- Monitoring: Metrics and traces from Kubernetes

### External Dependencies
- kube-rs or k8s-openapi: Kubernetes API client
- tokio: Async runtime for Kubernetes operations
- Kubernetes cluster: Local (Minikube) or managed (EKS/GKE)
- Container registry: For prodigy-agent images
- Optional: KEDA for advanced autoscaling

## Testing Strategy

### Unit Tests
- Job manifest generation from workflows
- Resource limit calculations
- Label and selector generation
- Configuration validation

### Integration Tests
- Job submission to test cluster (Kind)
- Log streaming verification
- Secret and ConfigMap management
- Volume mounting and data persistence
- Multi-pod coordination for MapReduce

### Performance Tests
- Measure job submission latency
- Concurrent pod scaling limits
- Log streaming throughput
- Resource utilization efficiency
- Cluster autoscaling response time

### User Acceptance
- Execute sample workflows on Kubernetes
- Verify cost reduction with spot instances
- Test failover between clusters
- Validate monitoring and alerting
- Confirm backward compatibility

## Documentation Requirements

### Code Documentation
- Document all Kubernetes API interactions
- Explain manifest generation logic
- Describe resource calculation algorithms
- Detail error handling strategies

### User Documentation
- Quick start guide for Minikube setup
- Production deployment guide for EKS/GKE
- Cluster configuration reference
- Troubleshooting common issues
- Cost optimization best practices

### Architecture Updates
- Update ARCHITECTURE.md with Kubernetes executor
- Document cluster architecture patterns
- Explain storage strategies for Kubernetes
- Describe security model and RBAC requirements

## Implementation Notes

### Container Image Strategy
- Multi-stage Dockerfile for minimal image size
- Cache Claude CLI and common dependencies
- Support custom images per workflow
- Regular security scanning and updates
- Consider distroless base images

### Cost Optimization
- Leverage spot instances for 70% cost reduction
- Right-size resource requests based on profiling
- Use cluster autoscaler to scale down when idle
- Implement pod disruption budgets for spot interruptions
- Consider reserved instances for baseline capacity

### Security Considerations
- Use workload identity for cloud provider access
- Rotate secrets automatically
- Implement network policies for isolation
- Enable pod security standards
- Audit log all API operations

### Observability
- Export Prometheus metrics from agents
- Trace workflow execution with OpenTelemetry
- Centralize logs with Fluentd/Fluent Bit
- Create Grafana dashboards for monitoring
- Set up alerts for job failures and resource issues

## Migration and Compatibility

### Backward Compatibility
- Default to local execution without --cluster flag
- Support existing workflow YAML format
- Preserve git worktree functionality
- Maintain file-based storage as fallback

### Migration Path
1. Deploy Kubernetes executor alongside existing code
2. Test with non-critical workflows first
3. Gradually migrate workflows to Kubernetes
4. Phase out git worktrees for cloud execution
5. Maintain local mode for development

### Breaking Changes
- None: Kubernetes support is additive
- Existing workflows continue to work unchanged
- New configuration options are optional
- Storage backend selection is automatic

### Configuration Migration
```yaml
# Old configuration (still supported)
execution:
  mode: local
  worktree_dir: ~/.prodigy/worktrees

# New configuration (optional)
execution:
  mode: kubernetes
  clusters:
    development:
      kubeconfig: ~/.kube/config
      context: minikube
      namespace: prodigy-dev
    production:
      kubeconfig: ~/.kube/prod-config
      context: eks-production
      namespace: prodigy
      spot_instances: true
      max_parallel: 100
```

## Future Enhancements

### Phase 2 Features
- Argo Workflows integration for complex DAGs
- Knative serving for serverless execution
- GPU support for ML workflows
- Multi-cluster federation
- Workflow marketplace with Helm charts

### Platform Evolution
- Multi-tenancy with namespace isolation
- Usage metering and billing
- Self-service portal for workflow submission
- Workflow templates and parameterization
- GitOps-based deployment with FluxCD

### Advanced Orchestration
- Workflow dependencies and chaining
- Conditional execution based on results
- Human-in-the-loop approvals
- Scheduled and recurring workflows
- Blue-green deployments for workflows