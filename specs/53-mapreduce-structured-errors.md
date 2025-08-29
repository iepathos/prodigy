---
number: 53
title: MapReduce Structured Error Types
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-29
---

# Specification 53: MapReduce Structured Error Types

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current MapReduce implementation uses generic `anyhow::Error` types throughout, making it difficult to handle specific error cases programmatically, provide meaningful error messages to users, and implement sophisticated retry logic based on error types. Structured errors with rich context are essential for production-grade distributed systems.

## Objective

Implement a comprehensive structured error type system for MapReduce operations that provides clear error categorization, rich context for debugging, actionable error messages, and enables programmatic error handling and recovery strategies.

## Requirements

### Functional Requirements
- Define specific error types for all failure modes
- Include relevant context in each error
- Support error chaining and causality
- Provide actionable error messages
- Enable error categorization for retry logic
- Support error serialization for persistence
- Include correlation IDs for tracing
- Provide error recovery hints

### Non-Functional Requirements
- Zero-cost abstractions using thiserror
- Maintain backward compatibility
- Support error aggregation for batch operations
- Enable efficient error pattern matching
- Minimize error construction overhead

## Acceptance Criteria

- [ ] MapReduceError enum covers all error scenarios
- [ ] Each error includes relevant context fields
- [ ] Errors implement std::error::Error trait
- [ ] Retry logic uses error categorization
- [ ] Error messages are user-friendly
- [ ] Errors serializable to JSON
- [ ] Error recovery hints provided
- [ ] Integration with event logging
- [ ] Error metrics collection enabled
- [ ] Backward compatible with existing code

## Technical Details

### Implementation Approach

1. **Core Error Types**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MapReduceError {
    // Job-level errors
    #[error("Job {job_id} initialization failed: {reason}")]
    JobInitializationFailed {
        job_id: String,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Job {job_id} already exists")]
    JobAlreadyExists { job_id: String },
    
    #[error("Job {job_id} not found")]
    JobNotFound { job_id: String },
    
    #[error("Job {job_id} checkpoint corrupted at version {version}")]
    CheckpointCorrupted {
        job_id: String,
        version: u32,
        details: String,
    },
    
    // Agent-level errors
    #[error("Agent {agent_id} failed processing item {item_id}: {reason}")]
    AgentFailed {
        job_id: String,
        agent_id: String,
        item_id: String,
        reason: String,
        worktree: Option<String>,
        duration_ms: u64,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Agent {agent_id} timeout after {duration_secs}s")]
    AgentTimeout {
        job_id: String,
        agent_id: String,
        item_id: String,
        duration_secs: u64,
        last_operation: String,
    },
    
    #[error("Agent {agent_id} resource exhaustion: {resource}")]
    ResourceExhausted {
        job_id: String,
        agent_id: String,
        resource: ResourceType,
        limit: String,
        usage: String,
    },
    
    // Worktree errors
    #[error("Worktree creation failed for agent {agent_id}: {reason}")]
    WorktreeCreationFailed {
        agent_id: String,
        reason: String,
        #[source]
        source: std::io::Error,
    },
    
    #[error("Worktree merge conflict for agent {agent_id} on branch {branch}")]
    WorktreeMergeConflict {
        agent_id: String,
        branch: String,
        conflicts: Vec<String>,
    },
    
    // Command execution errors
    #[error("Command execution failed: {command}")]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        working_dir: PathBuf,
    },
    
    #[error("Shell substitution failed: missing variable {variable}")]
    ShellSubstitutionFailed {
        variable: String,
        command: String,
        available_vars: Vec<String>,
    },
    
    // I/O errors
    #[error("Failed to persist checkpoint for job {job_id}")]
    CheckpointPersistFailed {
        job_id: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    
    #[error("Failed to load work items from {path}")]
    WorkItemLoadFailed {
        path: PathBuf,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    // Configuration errors
    #[error("Invalid MapReduce configuration: {reason}")]
    InvalidConfiguration {
        reason: String,
        field: String,
        value: String,
    },
    
    #[error("JSON path expression invalid: {expression}")]
    InvalidJsonPath {
        expression: String,
        #[source]
        source: serde_json::Error,
    },
    
    // Concurrency errors
    #[error("Deadlock detected in job {job_id}")]
    DeadlockDetected {
        job_id: String,
        waiting_agents: Vec<String>,
    },
    
    #[error("Concurrent modification of job {job_id} state")]
    ConcurrentModification {
        job_id: String,
        operation: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Memory,
    DiskSpace,
    FileDescriptors,
    ThreadPool,
    NetworkConnections,
}
```

2. **Error Context and Metadata**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub correlation_id: String,
    pub timestamp: DateTime<Utc>,
    pub hostname: String,
    pub thread_id: String,
    pub span_trace: Vec<SpanInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanInfo {
    pub name: String,
    pub start: DateTime<Utc>,
    pub attributes: HashMap<String, String>,
}

impl MapReduceError {
    pub fn with_context(self, context: ErrorContext) -> ContextualError {
        ContextualError {
            error: self,
            context,
        }
    }
    
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            Self::AgentTimeout { .. } |
            Self::ResourceExhausted { .. } |
            Self::WorktreeCreationFailed { .. } |
            Self::CheckpointPersistFailed { .. }
        )
    }
    
    pub fn recovery_hint(&self) -> Option<String> {
        match self {
            Self::ResourceExhausted { resource, .. } => {
                Some(format!("Increase {:?} limit or reduce parallelism", resource))
            }
            Self::WorktreeMergeConflict { .. } => {
                Some("Manual conflict resolution required".to_string())
            }
            Self::ShellSubstitutionFailed { variable, available_vars, .. } => {
                Some(format!("Variable '{}' not found. Available: {:?}", variable, available_vars))
            }
            _ => None,
        }
    }
}
```

3. **Error Aggregation**
```rust
#[derive(Debug, Error)]
#[error("Multiple errors occurred during MapReduce execution")]
pub struct AggregatedError {
    pub errors: Vec<MapReduceError>,
    pub total_count: usize,
    pub by_type: HashMap<String, usize>,
}

impl AggregatedError {
    pub fn new(errors: Vec<MapReduceError>) -> Self {
        let mut by_type = HashMap::new();
        for error in &errors {
            *by_type.entry(error.variant_name()).or_insert(0) += 1;
        }
        
        Self {
            total_count: errors.len(),
            errors,
            by_type,
        }
    }
    
    pub fn most_common_error(&self) -> Option<&str> {
        self.by_type
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name.as_str())
    }
}
```

### Architecture Changes
- Replace `anyhow::Error` with `MapReduceError`
- Add error context propagation
- Implement error-based retry strategies
- Add error metrics collection

### Data Structures
```rust
pub struct ErrorReport {
    pub error: MapReduceError,
    pub context: ErrorContext,
    pub stack_trace: Vec<String>,
    pub related_errors: Vec<MapReduceError>,
}

pub struct ErrorStats {
    pub total_errors: usize,
    pub errors_by_type: HashMap<String, usize>,
    pub error_rate: f64,
    pub mean_time_to_recovery: Duration,
}
```

### APIs and Interfaces
```rust
pub type MapReduceResult<T> = Result<T, MapReduceError>;

pub trait ErrorHandler {
    fn handle_error(&self, error: &MapReduceError) -> ErrorAction;
    fn should_retry(&self, error: &MapReduceError) -> bool;
    fn retry_delay(&self, error: &MapReduceError, attempt: u32) -> Duration;
}

pub enum ErrorAction {
    Retry { delay: Duration },
    Fallback { handler: String },
    Propagate,
    Ignore,
    Abort,
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - All MapReduce execution paths
  - Error handling logic
  - Retry mechanisms
- **External Dependencies**: 
  - `thiserror` crate for error derivation

## Testing Strategy

- **Unit Tests**: 
  - Test error construction
  - Verify error categorization
  - Test serialization/deserialization
  - Validate recovery hints
  
- **Integration Tests**: 
  - Test error propagation
  - Verify retry logic
  - Test error aggregation
  - Validate error reporting
  
- **Performance Tests**: 
  - Measure error construction overhead
  - Test with high error rates
  - Benchmark error serialization
  
- **User Acceptance**: 
  - Clear error messages in UI
  - Actionable error hints
  - Useful debug information

## Documentation Requirements

- **Code Documentation**: 
  - Document each error variant
  - Explain retry strategies
  - Document error contexts
  
- **User Documentation**: 
  - Error troubleshooting guide
  - Common error solutions
  - Error code reference
  
- **Architecture Updates**: 
  - Error handling flow diagram
  - Retry strategy documentation

## Implementation Notes

- Use `#[from]` for automatic conversions
- Implement Display with helpful messages
- Consider error codes for i18n
- Add structured logging integration
- Use type aliases for common Results
- Consider error telemetry
- Implement error fingerprinting

## Migration and Compatibility

- Gradual migration from anyhow::Error
- Provide conversion traits
- Maintain error message compatibility
- Update error handling patterns
- Document migration guide