---
number: 96
title: Container Runtime Integration for Workflow Execution
category: parallel
priority: high
status: draft
dependencies: [93, 94, 95]
created: 2025-01-17
---

# Specification 96: Container Runtime Integration for Workflow Execution

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [93 - Storage Abstraction Layer, 94 - PostgreSQL Storage Backend, 95 - Storage Migration Tools]

## Context

With the storage abstraction layer providing database-backed storage suitable for containerized environments, Prodigy needs to evolve its execution model to leverage container runtimes for workflow and agent execution. Currently, Prodigy uses git worktrees and local process execution, which limits scalability, resource isolation, and deployment flexibility. Container-based execution enables horizontal scaling, better resource management, consistent environments, and cloud-native deployments.

Container runtimes like Docker and Kubernetes provide process isolation, resource limits, networking abstractions, and orchestration capabilities that are essential for running Prodigy at scale. By executing workflows and MapReduce agents in containers, we can achieve better fault isolation, enable multi-tenancy, support heterogeneous execution environments, and integrate with modern CI/CD and orchestration platforms.

## Objective

Implement comprehensive container runtime support for Prodigy workflow execution, enabling workflows and MapReduce agents to run in isolated containers with proper resource management, networking, and storage integration. The implementation must support multiple container runtimes (Docker, Kubernetes), maintain backward compatibility with local execution, provide efficient container image management, and enable seamless scaling from local development to production deployments.

## Requirements

### Functional Requirements
- Support Docker as primary container runtime
- Enable Kubernetes Job/Pod execution for cloud deployments
- Container image management with caching and versioning
- Volume mounting for workflow files and artifacts
- Network isolation and service discovery
- Resource limits (CPU, memory, disk) per container
- Container lifecycle management (create, start, stop, remove)
- Log streaming from containers to storage backend
- Environment variable and secret injection
- Support for custom container images per workflow

### Non-Functional Requirements
- Container startup time <5 seconds for cached images
- Support 1000+ concurrent containers
- Minimal overhead vs native execution (<10%)
- Automatic cleanup of stopped containers
- Graceful shutdown with timeout handling
- Container health monitoring and restart policies
- Security through least-privilege container execution
- Observability with metrics and distributed tracing

## Acceptance Criteria

- [ ] Workflows execute successfully in Docker containers
- [ ] MapReduce agents run in parallel containers
- [ ] Container resource limits are enforced
- [ ] Logs stream from containers to storage backend
- [ ] Kubernetes execution works with Jobs API
- [ ] Container images are efficiently cached
- [ ] Network isolation prevents cross-workflow interference
- [ ] Secrets and configs mount correctly in containers
- [ ] Performance meets latency and throughput requirements
- [ ] Integration tests pass with container execution
- [ ] Monitoring exposes container metrics

## Technical Details

### Implementation Approach

1. **Container Runtime Abstraction**
   ```rust
   #[async_trait]
   pub trait ContainerRuntime: Send + Sync {
       /// Create a new container
       async fn create_container(&self, config: ContainerConfig) -> Result<ContainerId>;

       /// Start a container
       async fn start_container(&self, id: &ContainerId) -> Result<()>;

       /// Stop a container
       async fn stop_container(&self, id: &ContainerId, timeout: Duration) -> Result<()>;

       /// Remove a container
       async fn remove_container(&self, id: &ContainerId) -> Result<()>;

       /// Stream container logs
       async fn stream_logs(&self, id: &ContainerId) -> Result<LogStream>;

       /// Get container status
       async fn container_status(&self, id: &ContainerId) -> Result<ContainerStatus>;

       /// Execute command in container
       async fn exec(&self, id: &ContainerId, cmd: Vec<String>) -> Result<ExecResult>;

       /// Copy files to/from container
       async fn copy_to(&self, id: &ContainerId, src: &Path, dest: &Path) -> Result<()>;
       async fn copy_from(&self, id: &ContainerId, src: &Path, dest: &Path) -> Result<()>;
   }

   pub struct ContainerConfig {
       pub image: String,
       pub command: Vec<String>,
       pub env: HashMap<String, String>,
       pub volumes: Vec<VolumeMount>,
       pub network: NetworkConfig,
       pub resources: ResourceLimits,
       pub labels: HashMap<String, String>,
       pub user: Option<String>,
       pub working_dir: Option<PathBuf>,
       pub restart_policy: RestartPolicy,
   }
   ```

2. **Docker Runtime Implementation**
   ```rust
   pub struct DockerRuntime {
       client: Docker,
       config: DockerConfig,
       image_cache: Arc<ImageCache>,
       network_manager: Arc<NetworkManager>,
   }

   #[async_trait]
   impl ContainerRuntime for DockerRuntime {
       async fn create_container(&self, config: ContainerConfig) -> Result<ContainerId> {
           // Ensure image is available
           self.ensure_image(&config.image).await?;

           // Create container configuration
           let docker_config = Config {
               image: Some(config.image),
               cmd: Some(config.command),
               env: Some(config.env.into_iter()
                   .map(|(k, v)| format!("{}={}", k, v))
                   .collect()),
               host_config: Some(HostConfig {
                   binds: Some(config.volumes.into_iter()
                       .map(|v| format!("{}:{}:{}", v.host_path, v.container_path, v.mode))
                       .collect()),
                   memory: config.resources.memory_limit,
                   cpu_shares: config.resources.cpu_shares,
                   network_mode: Some(config.network.mode),
                   restart_policy: Some(config.restart_policy.into()),
                   ..Default::default()
               }),
               labels: Some(config.labels),
               user: config.user,
               working_dir: config.working_dir.map(|p| p.to_string_lossy().to_string()),
               ..Default::default()
           };

           let response = self.client
               .create_container::<String, String>(None, docker_config)
               .await?;

           Ok(ContainerId(response.id))
       }

       async fn stream_logs(&self, id: &ContainerId) -> Result<LogStream> {
           let options = LogsOptions {
               stdout: true,
               stderr: true,
               follow: true,
               timestamps: true,
               ..Default::default()
           };

           let stream = self.client.logs(&id.0, Some(options));

           Ok(LogStream::from_docker(stream))
       }
   }

   impl DockerRuntime {
       async fn ensure_image(&self, image: &str) -> Result<()> {
           if self.image_cache.has(image).await {
               return Ok(());
           }

           info!("Pulling Docker image: {}", image);

           let options = CreateImageOptions {
               from_image: image,
               ..Default::default()
           };

           let mut stream = self.client.create_image(Some(options), None, None);

           while let Some(info) = stream.next().await {
               let info = info?;
               debug!("Pull progress: {:?}", info);
           }

           self.image_cache.add(image).await;
           Ok(())
       }
   }
   ```

3. **Kubernetes Runtime Implementation**
   ```rust
   pub struct KubernetesRuntime {
       client: Client,
       namespace: String,
       config: KubernetesConfig,
   }

   #[async_trait]
   impl ContainerRuntime for KubernetesRuntime {
       async fn create_container(&self, config: ContainerConfig) -> Result<ContainerId> {
           // Create Kubernetes Job for workflow execution
           let job = Job {
               metadata: ObjectMeta {
                   name: Some(format!("prodigy-{}", Uuid::new_v4())),
                   namespace: Some(self.namespace.clone()),
                   labels: Some(config.labels),
                   ..Default::default()
               },
               spec: Some(JobSpec {
                   template: PodTemplateSpec {
                       metadata: Some(ObjectMeta {
                           labels: Some(config.labels.clone()),
                           ..Default::default()
                       }),
                       spec: Some(PodSpec {
                           containers: vec![Container {
                               name: "prodigy-agent".to_string(),
                               image: Some(config.image),
                               command: Some(config.command),
                               env: Some(config.env.into_iter()
                                   .map(|(k, v)| EnvVar {
                                       name: k,
                                       value: Some(v),
                                       ..Default::default()
                                   })
                                   .collect()),
                               volume_mounts: Some(self.create_volume_mounts(&config.volumes)),
                               resources: Some(self.create_resource_requirements(&config.resources)),
                               working_dir: config.working_dir.map(|p| p.to_string_lossy().to_string()),
                               ..Default::default()
                           }],
                           volumes: Some(self.create_volumes(&config.volumes)),
                           restart_policy: Some(config.restart_policy.to_k8s()),
                           ..Default::default()
                       }),
                   },
                   backoff_limit: Some(3),
                   ttl_seconds_after_finished: Some(3600),
                   ..Default::default()
               }),
               ..Default::default()
           };

           let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);
           let result = jobs.create(&PostParams::default(), &job).await?;

           Ok(ContainerId(result.metadata.uid.unwrap()))
       }

       async fn stream_logs(&self, id: &ContainerId) -> Result<LogStream> {
           // Find pod for job
           let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
           let pod = self.find_pod_for_job(id).await?;

           let params = LogParams {
               follow: true,
               timestamps: true,
               ..Default::default()
           };

           let stream = pods.log_stream(&pod.metadata.name.unwrap(), &params).await?;

           Ok(LogStream::from_k8s(stream))
       }
   }
   ```

4. **Workflow Container Executor**
   ```rust
   pub struct ContainerWorkflowExecutor {
       runtime: Arc<dyn ContainerRuntime>,
       storage: Arc<dyn UnifiedStorage>,
       config: ContainerExecutorConfig,
   }

   impl ContainerWorkflowExecutor {
       pub async fn execute_workflow(
           &self,
           workflow: &Workflow,
           context: WorkflowContext,
       ) -> Result<WorkflowResult> {
           // Prepare container configuration
           let container_config = self.prepare_container_config(workflow, &context)?;

           // Create and start container
           let container_id = self.runtime.create_container(container_config).await?;
           self.runtime.start_container(&container_id).await?;

           // Stream logs to storage
           let log_handle = self.stream_logs_to_storage(&container_id).await?;

           // Monitor container execution
           let result = self.monitor_execution(&container_id, workflow.timeout).await?;

           // Cleanup
           self.runtime.stop_container(&container_id, Duration::from_secs(30)).await?;
           self.runtime.remove_container(&container_id).await?;

           Ok(result)
       }

       async fn prepare_container_config(
           &self,
           workflow: &Workflow,
           context: &WorkflowContext,
       ) -> Result<ContainerConfig> {
           let image = workflow.container_image
               .clone()
               .unwrap_or_else(|| self.config.default_image.clone());

           // Mount workflow files
           let workflow_mount = VolumeMount {
               host_path: context.workflow_dir.clone(),
               container_path: PathBuf::from("/workspace"),
               mode: VolumeMode::ReadWrite,
           };

           // Mount storage credentials
           let storage_mount = self.create_storage_config_mount().await?;

           // Environment variables
           let mut env = HashMap::new();
           env.insert("PRODIGY_STORAGE_BACKEND".to_string(),
                     self.config.storage_backend.to_string());
           env.insert("PRODIGY_WORKFLOW_ID".to_string(),
                     context.workflow_id.clone());
           env.extend(workflow.env.clone());

           Ok(ContainerConfig {
               image,
               command: workflow.command.clone(),
               env,
               volumes: vec![workflow_mount, storage_mount],
               network: NetworkConfig::default(),
               resources: ResourceLimits {
                   memory_limit: workflow.memory_limit,
                   cpu_shares: workflow.cpu_shares,
                   ..Default::default()
               },
               labels: HashMap::from([
                   ("prodigy.workflow".to_string(), workflow.name.clone()),
                   ("prodigy.job_id".to_string(), context.job_id.clone()),
               ]),
               user: None,
               working_dir: Some(PathBuf::from("/workspace")),
               restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
           })
       }
   }
   ```

5. **MapReduce Container Agent Manager**
   ```rust
   pub struct ContainerAgentManager {
       runtime: Arc<dyn ContainerRuntime>,
       storage: Arc<dyn UnifiedStorage>,
       config: AgentManagerConfig,
       active_agents: Arc<RwLock<HashMap<AgentId, ContainerId>>>,
   }

   impl ContainerAgentManager {
       pub async fn spawn_agent(
           &self,
           work_item: WorkItem,
           agent_config: AgentConfig,
       ) -> Result<AgentId> {
           let agent_id = AgentId::new();

           // Prepare container for agent
           let container_config = ContainerConfig {
               image: agent_config.image.clone(),
               command: vec![
                   "prodigy".to_string(),
                   "agent".to_string(),
                   "--work-item".to_string(),
                   serde_json::to_string(&work_item)?,
               ],
               env: self.prepare_agent_env(&agent_id, &agent_config),
               volumes: vec![self.create_agent_workspace(&agent_id).await?],
               network: NetworkConfig {
                   mode: "bridge".to_string(),
                   aliases: vec![format!("agent-{}", agent_id)],
               },
               resources: agent_config.resources.clone(),
               labels: HashMap::from([
                   ("prodigy.agent".to_string(), agent_id.to_string()),
                   ("prodigy.job".to_string(), agent_config.job_id.clone()),
               ]),
               ..Default::default()
           };

           // Create and start container
           let container_id = self.runtime.create_container(container_config).await?;
           self.runtime.start_container(&container_id).await?;

           // Track active agent
           self.active_agents.write().await.insert(agent_id.clone(), container_id);

           // Monitor agent in background
           let manager = self.clone();
           tokio::spawn(async move {
               manager.monitor_agent(&agent_id).await;
           });

           Ok(agent_id)
       }

       async fn monitor_agent(&self, agent_id: &AgentId) -> Result<()> {
           let container_id = self.active_agents.read().await
               .get(agent_id)
               .cloned()
               .ok_or_else(|| anyhow!("Agent not found"))?;

           loop {
               let status = self.runtime.container_status(&container_id).await?;

               match status {
                   ContainerStatus::Running => {
                       tokio::time::sleep(Duration::from_secs(5)).await;
                   }
                   ContainerStatus::Exited { code } => {
                       if code == 0 {
                           info!("Agent {} completed successfully", agent_id);
                       } else {
                           warn!("Agent {} failed with code {}", agent_id, code);
                       }
                       break;
                   }
                   ContainerStatus::Failed { error } => {
                       error!("Agent {} failed: {}", agent_id, error);
                       break;
                   }
                   _ => {}
               }
           }

           // Cleanup
           self.cleanup_agent(agent_id).await?;
           Ok(())
       }
   }
   ```

### Architecture Changes

- Add container runtime abstraction layer
- Implement Docker and Kubernetes runtime adapters
- Create container-aware workflow executor
- Modify MapReduce to use container agents
- Add container image management subsystem
- Integrate with storage abstraction for state persistence

### Data Structures

```rust
pub struct ContainerExecutorConfig {
    pub runtime: RuntimeType,
    pub default_image: String,
    pub storage_backend: BackendType,
    pub max_concurrent_containers: usize,
    pub container_timeout: Duration,
    pub cleanup_policy: CleanupPolicy,
    pub registry_auth: Option<RegistryAuth>,
}

pub enum RuntimeType {
    Docker,
    Kubernetes,
    Podman,
}

pub struct ResourceLimits {
    pub memory_limit: Option<i64>,
    pub cpu_shares: Option<i64>,
    pub disk_limit: Option<i64>,
    pub network_bandwidth: Option<i64>,
}

pub struct ContainerMetrics {
    pub cpu_usage: f64,
    pub memory_usage: i64,
    pub network_rx: i64,
    pub network_tx: i64,
    pub disk_read: i64,
    pub disk_write: i64,
}
```

## Dependencies

- **Prerequisites**: [93, 94, 95 - Storage layer implementations]
- **Affected Components**: Workflow executor, MapReduce engine, CLI
- **External Dependencies**:
  - bollard - Docker API client
  - kube - Kubernetes client
  - tokio - Async runtime
  - futures - Stream utilities

## Testing Strategy

- **Unit Tests**: Mock container runtime for logic testing
- **Integration Tests**: Real Docker containers for workflows
- **Performance Tests**: Container overhead measurement
- **Stress Tests**: 1000+ concurrent containers
- **Failure Tests**: Container crashes, network failures
- **Security Tests**: Privilege escalation prevention
- **Compatibility Tests**: Different container runtime versions

## Documentation Requirements

- **Setup Guide**: Container runtime installation and configuration
- **Image Management**: Building and managing Prodigy images
- **Deployment Guide**: Kubernetes deployment procedures
- **Security Guide**: Container security best practices
- **Troubleshooting**: Common container issues and solutions

## Implementation Notes

- Use container labels for tracking and cleanup
- Implement graceful shutdown with SIGTERM handling
- Add retry logic for transient container failures
- Use health checks for container readiness
- Implement log rotation for container logs
- Consider using BuildKit for efficient image building
- Add support for GPU containers for ML workloads
- Use container registries for image distribution

## Migration and Compatibility

- Maintain support for local execution without containers
- Provide migration guide from worktree to container execution
- Support hybrid mode with some workflows in containers
- Ensure backward compatibility with existing workflows
- Version container images for reproducibility
- Document breaking changes clearly