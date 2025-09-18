---
number: 98
title: Kubernetes Storage and Secrets Management
category: parallel
priority: high
status: draft
dependencies: [97]
created: 2025-01-18
---

# Specification 98: Kubernetes Storage and Secrets Management

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [97 - Basic Kubernetes Job Execution]

## Context

While basic Kubernetes Job execution enables cloud burst capabilities, production workflows require secure credential management and persistent storage for git repositories, artifacts, and intermediate results. Currently, workflows rely on local filesystem access and environment variables for secrets, which doesn't translate to containerized Kubernetes execution.

Kubernetes provides native primitives for these concerns: Secrets for sensitive data like Claude API keys and Git tokens, ConfigMaps for workflow definitions, and PersistentVolumes for shared storage. Proper implementation of these primitives is essential for production-ready Kubernetes execution.

## Objective

Implement comprehensive storage and secrets management for Kubernetes-based workflow execution, enabling secure credential injection, persistent git repository access, and efficient data sharing between workflow components. The solution must be secure by default, support credential rotation, and provide multiple storage strategies for different use cases.

## Requirements

### Functional Requirements

#### Secrets Management
- Secure injection of Claude API keys into workflow containers
- Git authentication tokens for repository access
- Support for multiple secret sources (files, environment, external systems)
- Automatic secret creation and updates
- Secret rotation without workflow restart

#### Storage Integration
- PersistentVolumeClaims for git repository workspaces
- ConfigMaps for workflow definitions and configurations
- EmptyDir volumes for temporary data
- Support for different storage classes and access modes
- Efficient git repository cloning and caching strategies

#### Git Repository Access
- Multiple authentication methods (SSH keys, personal access tokens, GitHub Apps)
- Support for private repositories
- Shallow cloning and sparse checkout optimization
- Git LFS support for large files
- Repository caching to reduce clone times

#### Workflow Configuration
- Inject workflow YAML as ConfigMaps
- Support for workflow parameterization
- Environment-specific configuration overlays
- Validation of configuration before Job submission

### Non-Functional Requirements

#### Security
- Secrets encrypted at rest in etcd
- Least-privilege access to secrets
- No secrets in container images or logs
- Audit trail for secret access
- Support for external secret management systems

#### Performance
- Git clone time <30 seconds for typical repositories
- Secret injection latency <5 seconds
- ConfigMap creation time <2 seconds
- Efficient storage utilization

#### Reliability
- Graceful handling of storage failures
- Automatic retry for transient issues
- Backup and recovery procedures
- Monitoring of storage health

## Acceptance Criteria

- [ ] Claude API keys injected securely via Kubernetes Secrets
- [ ] Git repositories cloned using injected credentials
- [ ] Workflow definitions loaded from ConfigMaps
- [ ] PersistentVolumes provide workspace for git operations
- [ ] Private repositories accessible with proper authentication
- [ ] Secrets can be updated without recreating Jobs
- [ ] Storage volumes cleaned up after workflow completion
- [ ] Multiple workflows can share cached git repositories
- [ ] Git LFS files downloaded correctly when present
- [ ] Workflow parameters can be injected via environment variables

## Technical Details

### Implementation Approach

#### Phase 1: Basic Secrets Integration
1. Create Kubernetes Secrets from user-provided credentials
2. Inject secrets as environment variables in Job pods
3. Support file-based secret mounting
4. Implement secret validation and error handling

#### Phase 2: Git Authentication and Cloning
1. Support multiple git authentication methods
2. Implement init containers for git cloning
3. Handle private repository access
4. Add support for SSH key authentication

#### Phase 3: Storage Management
1. Create PVCs for workspace persistence
2. Implement ConfigMap generation for workflows
3. Add volume mounting to Job specifications
4. Support different storage classes

#### Phase 4: Advanced Features
1. Repository caching and optimization
2. Git LFS support
3. Secret rotation mechanisms
4. External secret management integration

### Architecture Changes

```rust
// Secret management module
pub mod secrets {
    use k8s_openapi::api::core::v1::Secret;
    use std::collections::BTreeMap;

    pub struct SecretManager {
        client: kube::Client,
        namespace: String,
    }

    impl SecretManager {
        pub async fn create_workflow_secrets(
            &self,
            workflow_id: &str,
            credentials: &WorkflowCredentials,
        ) -> Result<String> {
            let secret_name = format!("prodigy-secrets-{}", workflow_id);
            let secret = self.build_secret(&secret_name, credentials)?;

            let secret_api: Api<Secret> = Api::namespaced(self.client.clone(), &self.namespace);
            secret_api.create(&PostParams::default(), &secret).await?;

            Ok(secret_name)
        }

        pub async fn update_secrets(
            &self,
            secret_name: &str,
            credentials: &WorkflowCredentials,
        ) -> Result<()> {
            // Implementation for secret updates
        }
    }
}

// Storage management module
pub mod storage {
    use k8s_openapi::api::core::v1::{PersistentVolumeClaim, ConfigMap};

    pub struct StorageManager {
        client: kube::Client,
        namespace: String,
    }

    impl StorageManager {
        pub async fn create_workspace_pvc(
            &self,
            workflow_id: &str,
            size: &str,
        ) -> Result<String> {
            let pvc_name = format!("prodigy-workspace-{}", workflow_id);
            let pvc = self.build_pvc(&pvc_name, size)?;

            let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(self.client.clone(), &self.namespace);
            pvc_api.create(&PostParams::default(), &pvc).await?;

            Ok(pvc_name)
        }

        pub async fn create_workflow_configmap(
            &self,
            workflow_id: &str,
            workflow_content: &str,
        ) -> Result<String> {
            let cm_name = format!("prodigy-workflow-{}", workflow_id);
            let configmap = self.build_configmap(&cm_name, workflow_content)?;

            let cm_api: Api<ConfigMap> = Api::namespaced(self.client.clone(), &self.namespace);
            cm_api.create(&PostParams::default(), &configmap).await?;

            Ok(cm_name)
        }
    }
}

// Enhanced workflow credentials
#[derive(Debug, Clone)]
pub struct WorkflowCredentials {
    pub claude_api_key: Option<String>,
    pub git_token: Option<String>,
    pub git_ssh_key: Option<String>,
    pub docker_registry_auth: Option<String>,
    pub custom_secrets: HashMap<String, String>,
}

// Git repository configuration
#[derive(Debug, Clone)]
pub struct GitRepositoryConfig {
    pub url: String,
    pub branch: Option<String>,
    pub auth_method: GitAuthMethod,
    pub shallow: bool,
    pub lfs: bool,
    pub sparse_checkout: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub enum GitAuthMethod {
    Token(String),
    SshKey(String),
    None,
}
```

### Secret Management Patterns

```yaml
# Generated Secret for workflow
apiVersion: v1
kind: Secret
metadata:
  name: prodigy-secrets-{{ workflow_id }}
  namespace: {{ namespace }}
  labels:
    app: prodigy
    workflow: {{ workflow_name }}
  annotations:
    prodigy.io/created-at: {{ timestamp }}
type: Opaque
data:
  claude-api-key: {{ claude_key | base64 }}
  git-token: {{ git_token | base64 }}
  git-ssh-key: {{ ssh_key | base64 }}
```

### Storage Volume Patterns

```yaml
# PVC for workspace
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: prodigy-workspace-{{ workflow_id }}
  namespace: {{ namespace }}
  labels:
    app: prodigy
    workflow: {{ workflow_name }}
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: {{ workspace_size | default("10Gi") }}
  storageClassName: {{ storage_class | default("standard") }}

---
# ConfigMap for workflow definition
apiVersion: v1
kind: ConfigMap
metadata:
  name: prodigy-workflow-{{ workflow_id }}
  namespace: {{ namespace }}
  labels:
    app: prodigy
    workflow: {{ workflow_name }}
data:
  workflow.yaml: |
    {{ workflow_content | indent(4) }}
  config.env: |
    {{ workflow_env_vars | indent(4) }}
```

### Enhanced Job Specification

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: prodigy-{{ workflow_id }}
  namespace: {{ namespace }}
spec:
  template:
    spec:
      initContainers:
      - name: git-clone
        image: alpine/git:latest
        command: ["/bin/sh"]
        args:
        - -c
        - |
          # Setup git authentication
          if [ -n "$GIT_TOKEN" ]; then
            git config --global credential.helper store
            echo "https://oauth2:${GIT_TOKEN}@github.com" > ~/.git-credentials
          elif [ -f "/etc/git-ssh/ssh-key" ]; then
            mkdir -p ~/.ssh
            cp /etc/git-ssh/ssh-key ~/.ssh/id_rsa
            chmod 600 ~/.ssh/id_rsa
            ssh-keyscan github.com >> ~/.ssh/known_hosts
          fi

          # Clone repository
          git clone {{ git_url }} /workspace/repo
          cd /workspace/repo

          {% if git_branch %}
          git checkout {{ git_branch }}
          {% endif %}

          {% if git_lfs %}
          git lfs pull
          {% endif %}
        env:
        - name: GIT_TOKEN
          valueFrom:
            secretKeyRef:
              name: prodigy-secrets-{{ workflow_id }}
              key: git-token
              optional: true
        volumeMounts:
        - name: workspace
          mountPath: /workspace
        - name: git-ssh-key
          mountPath: /etc/git-ssh
          readOnly: true

      containers:
      - name: prodigy-agent
        image: {{ image }}
        env:
        - name: CLAUDE_API_KEY
          valueFrom:
            secretKeyRef:
              name: prodigy-secrets-{{ workflow_id }}
              key: claude-api-key
        - name: WORKFLOW_ID
          value: "{{ workflow_id }}"
        volumeMounts:
        - name: workspace
          mountPath: /workspace
        - name: workflow-config
          mountPath: /config
          readOnly: true

      volumes:
      - name: workspace
        persistentVolumeClaim:
          claimName: prodigy-workspace-{{ workflow_id }}
      - name: workflow-config
        configMap:
          name: prodigy-workflow-{{ workflow_id }}
      - name: git-ssh-key
        secret:
          secretName: prodigy-secrets-{{ workflow_id }}
          items:
          - key: git-ssh-key
            path: ssh-key
          defaultMode: 0600
          optional: true

      restartPolicy: Never
```

### Configuration Extension

```yaml
# ~/.prodigy/kubernetes.yaml extension
clusters:
  production:
    kubeconfig_path: ~/.kube/prod-config
    context: eks-cluster
    namespace: prodigy

    # Storage configuration
    storage:
      workspace_size: "20Gi"
      storage_class: "gp3"
      cleanup_policy: "delete"  # or "retain"

    # Git configuration
    git:
      default_auth_method: "token"
      cache_repositories: true
      cache_size: "100Gi"
      lfs_enabled: true

    # Secret sources
    secrets:
      claude_api_key:
        source: "env"  # or "file", "external"
        key: "CLAUDE_API_KEY"
      git_token:
        source: "file"
        path: "~/.config/prodigy/git-token"
```

## Dependencies

### Prerequisites
- Specification 97: Basic Kubernetes Job Execution

### Affected Components
- Kubernetes executor: Extended with storage and secrets management
- CLI: New flags for credential sources and storage options
- Configuration: Storage and secret configuration options
- Container: Updated to handle mounted volumes and secrets

### External Dependencies
- Kubernetes cluster with RBAC enabled
- Storage provisioner for PVC creation
- Optional: External secret management system (Vault, AWS Secrets Manager)

## Testing Strategy

### Unit Tests
- Secret creation and validation
- ConfigMap generation from workflows
- PVC specification building
- Git authentication method selection

### Integration Tests
- Secret injection into containers
- Git cloning with different authentication methods
- PVC mounting and data persistence
- ConfigMap loading in containers

### Security Tests
- Secret encryption verification
- Access control validation
- Audit log verification
- Credential rotation testing

## Documentation Requirements

### User Documentation
- Credential setup guide for different git providers
- Storage configuration options
- Secret rotation procedures
- Troubleshooting authentication issues

### Security Documentation
- Secret management best practices
- RBAC requirements
- Audit and compliance procedures
- Threat model and mitigations

## Implementation Notes

### Security Best Practices
- Use separate secrets per workflow
- Rotate credentials regularly
- Audit secret access
- Encrypt secrets at rest
- Use least-privilege RBAC

### Storage Optimization
- Implement git repository caching
- Use sparse checkout for large repositories
- Clean up storage after workflow completion
- Monitor storage usage and costs

### Git Authentication Precedence
1. SSH key (if provided)
2. Personal access token
3. GitHub App token (future)
4. Anonymous (public repositories only)

## Migration and Compatibility

### Backward Compatibility
- Local execution continues to use environment variables
- Kubernetes execution without storage/secrets falls back to basic mode
- Configuration is optional and has sensible defaults

### Migration Path
1. Add storage and secrets support to Kubernetes executor
2. Update Job generation to include volumes and secrets
3. Test with existing workflows
4. Document credential setup procedures
5. Migrate workflows gradually

### Breaking Changes
None - this extends existing Kubernetes execution capabilities.