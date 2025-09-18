---
number: 97
title: Basic Kubernetes Job Execution
category: parallel
priority: high
status: draft
dependencies: [96]
created: 2025-01-18
---

# Specification 97: Basic Kubernetes Job Execution

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [96 - Container Runtime Integration]

## Context

Prodigy currently executes workflows using local git worktrees and process execution, which creates resource constraints when running multiple workflows concurrently. A user running 5-10 workflows simultaneously experiences CPU and memory bottlenecks on their local machine, making it difficult to work productively while workflows execute.

The solution is to enable Prodigy to burst workflow execution to Kubernetes clusters, starting with basic single-workflow execution as Kubernetes Jobs. This provides immediate relief from local resource constraints while laying the foundation for more advanced orchestration features.

## Objective

Implement basic Kubernetes Job execution for Prodigy workflows, enabling users to execute workflows on Kubernetes clusters instead of locally. The implementation must support both local clusters (Minikube, Kind) for development and managed clusters (EKS, GKE, AKS) for production, while maintaining full backward compatibility with local execution.

## Requirements

### Functional Requirements

#### Core Kubernetes Integration
- Connect to Kubernetes clusters using kubeconfig files
- Generate Kubernetes Job manifests from workflow definitions
- Submit Jobs to clusters via Kubernetes API
- Monitor Job status and completion
- Stream logs from Job pods in real-time
- Handle Job failures and timeouts gracefully

#### CLI Integration
- Add `--cluster` flag to specify target Kubernetes cluster
- Add `--namespace` flag to specify Kubernetes namespace
- Support multiple cluster configurations
- Default to local execution when no cluster specified
- Validate cluster connectivity before job submission

#### Job Lifecycle Management
- Create Job with appropriate labels and annotations
- Monitor Job until completion or failure
- Clean up completed Jobs after configurable TTL
- Support Job cancellation via Ctrl+C
- Provide meaningful error messages for failures

#### Container Image Management
- Use configurable container image for workflow execution
- Support image pull policies (Always, IfNotPresent, Never)
- Handle image pull failures gracefully
- Support private container registries

### Non-Functional Requirements

#### Performance
- Job submission latency <2 seconds
- Log streaming latency <1 second
- Support up to 10 concurrent workflows initially
- Pod startup time <60 seconds

#### Reliability
- Automatic retry on transient failures
- Graceful handling of cluster connectivity issues
- Proper cleanup of failed Jobs
- Timeout handling for long-running workflows

#### Usability
- Clear progress indicators during execution
- Intuitive error messages
- Consistent behavior with local execution
- Help text for new Kubernetes flags

## Acceptance Criteria

- [ ] `prodigy cook workflow.yaml --cluster minikube` executes workflow on Kubernetes
- [ ] `prodigy cook workflow.yaml` continues to work locally (backward compatibility)
- [ ] Logs stream in real-time from Kubernetes pods to terminal
- [ ] Failed Jobs provide clear error messages
- [ ] Ctrl+C cancels running Kubernetes Jobs
- [ ] Completed Jobs are cleaned up automatically
- [ ] Works with Minikube locally and EKS/GKE in cloud
- [ ] Multiple cluster configurations supported via config file
- [ ] Job status shows progress and completion state
- [ ] Container image can be configured per cluster

## Technical Details

### Implementation Approach

#### Phase 1: Kubernetes Client Integration
1. Add kube-rs dependency for Kubernetes API access
2. Implement cluster configuration management
3. Add kubeconfig loading and validation
4. Create basic Job submission functionality

#### Phase 2: Job Manifest Generation
1. Convert workflow definitions to Kubernetes Job specs
2. Handle environment variable injection
3. Configure resource limits and requests
4. Add appropriate labels and annotations

#### Phase 3: Job Monitoring and Logs
1. Implement Job status polling
2. Add real-time log streaming from pods
3. Handle multiple pods per Job (if applicable)
4. Provide progress indicators

#### Phase 4: Error Handling and Cleanup
1. Add comprehensive error handling
2. Implement Job cleanup on completion
3. Handle cancellation and timeouts
4. Add retry logic for transient failures

### Architecture Changes

```rust
// New Kubernetes executor module
pub mod kubernetes {
    use kube::{Client, Api, api::PostParams};
    use k8s_openapi::api::batch::v1::Job;

    pub struct KubernetesExecutor {
        client: Client,
        namespace: String,
        image: String,
        ttl_seconds: Option<i32>,
    }

    impl Executor for KubernetesExecutor {
        async fn execute_workflow(&self, workflow: &Workflow) -> Result<ExecutionResult> {
            // Generate Job manifest
            let job = self.create_job_manifest(workflow)?;

            // Submit to cluster
            let job_api: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);
            let created_job = job_api.create(&PostParams::default(), &job).await?;

            // Monitor and stream logs
            self.monitor_job(&created_job).await
        }
    }
}

// Extended CLI structure
#[derive(Parser)]
pub struct CookArgs {
    /// Workflow file to execute
    pub workflow: PathBuf,

    /// Target Kubernetes cluster (optional)
    #[arg(long)]
    pub cluster: Option<String>,

    /// Kubernetes namespace
    #[arg(long, default_value = "default")]
    pub namespace: String,

    /// Container image for execution
    #[arg(long)]
    pub image: Option<String>,
}

// Configuration structure
#[derive(Deserialize, Serialize)]
pub struct KubernetesConfig {
    pub clusters: HashMap<String, ClusterConfig>,
    pub default_image: String,
    pub default_namespace: String,
}

#[derive(Deserialize, Serialize)]
pub struct ClusterConfig {
    pub kubeconfig_path: Option<PathBuf>,
    pub context: Option<String>,
    pub namespace: Option<String>,
    pub image: Option<String>,
}
```

### Job Manifest Template

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: prodigy-{{ workflow_id }}
  namespace: {{ namespace }}
  labels:
    app: prodigy
    workflow: {{ workflow_name }}
    version: {{ prodigy_version }}
  annotations:
    prodigy.io/workflow-file: {{ workflow_file }}
    prodigy.io/submitted-by: {{ username }}
    prodigy.io/submitted-at: {{ timestamp }}
spec:
  ttlSecondsAfterFinished: {{ ttl_seconds | default(3600) }}
  backoffLimit: 2
  template:
    metadata:
      labels:
        app: prodigy
        workflow: {{ workflow_name }}
    spec:
      restartPolicy: Never
      containers:
      - name: prodigy-agent
        image: {{ image }}
        imagePullPolicy: IfNotPresent
        command: ["prodigy", "agent", "execute"]
        args: ["/workspace/workflow.yaml"]
        env:
        - name: PRODIGY_EXECUTION_MODE
          value: "kubernetes"
        - name: WORKFLOW_ID
          value: "{{ workflow_id }}"
        resources:
          requests:
            cpu: "0.5"
            memory: "1Gi"
          limits:
            cpu: "2"
            memory: "4Gi"
        volumeMounts:
        - name: workflow-data
          mountPath: /workspace
      volumes:
      - name: workflow-data
        configMap:
          name: prodigy-workflow-{{ workflow_id }}
```

### Configuration File Format

```yaml
# ~/.prodigy/kubernetes.yaml
clusters:
  minikube:
    kubeconfig_path: ~/.kube/config
    context: minikube
    namespace: prodigy-dev
    image: prodigy-agent:latest

  production:
    kubeconfig_path: ~/.kube/prod-config
    context: eks-cluster
    namespace: prodigy
    image: your-registry/prodigy-agent:v1.0.0

default_image: prodigy-agent:latest
default_namespace: default
```

## Dependencies

### Prerequisites
- Specification 96: Container Runtime Integration (for container image management)

### Affected Components
- CLI: New flags for cluster selection
- Executor: New Kubernetes executor alongside local executor
- Configuration: Cluster connection settings
- Container: Prodigy agent container image

### External Dependencies
- kube-rs: Kubernetes API client for Rust
- tokio: Async runtime for Kubernetes operations
- Kubernetes cluster: Local (Minikube/Kind) or managed (EKS/GKE/AKS)

## Testing Strategy

### Unit Tests
- Job manifest generation from workflows
- Configuration loading and validation
- Error handling for invalid inputs
- Resource limit calculations

### Integration Tests
- Job submission to test cluster (Kind)
- Log streaming functionality
- Job cancellation and cleanup
- Multiple cluster configurations

### End-to-End Tests
- Complete workflow execution on Kubernetes
- Backward compatibility with local execution
- Error scenarios (cluster down, image pull failures)
- Performance under load

## Documentation Requirements

### User Documentation
- Quick start guide for Minikube setup
- Configuration reference for clusters
- Troubleshooting common issues
- Migration guide from local execution

### Developer Documentation
- Kubernetes executor architecture
- Job manifest generation logic
- Error handling patterns
- Testing with local clusters

## Implementation Notes

### Container Image Requirements
The prodigy-agent container must include:
- Prodigy binary with agent subcommand
- All required dependencies (git, curl, etc.)
- Claude CLI if needed for workflows
- Proper entrypoint and signal handling

### Security Considerations
- Use least-privilege service accounts
- Validate all user inputs
- Secure kubeconfig file handling
- Audit log Job submissions

### Error Recovery
- Retry transient API failures
- Handle pod evictions gracefully
- Provide actionable error messages
- Log debugging information

## Migration and Compatibility

### Backward Compatibility
- Default execution remains local
- All existing CLI flags continue to work
- Workflow YAML format unchanged
- Configuration files are optional

### Migration Path
1. Install Kubernetes support alongside existing code
2. Test with --cluster flag on development workflows
3. Configure production clusters
4. Gradually migrate workflows to Kubernetes
5. Keep local execution for development and testing

### Breaking Changes
None - this is purely additive functionality.