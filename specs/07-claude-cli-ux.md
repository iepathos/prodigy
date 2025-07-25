# Specification 07: Claude CLI UX Improvements

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [03-claude-integration]

## Context

The current integration with Claude CLI commands (lint, review, implement-spec, add-spec) suffers from poor user experience when executing long-running operations. The command hangs without feedback, then dumps all output at once when complete. This creates uncertainty for users who cannot tell if the command is running, stuck, or making progress. We need to improve this experience with better feedback mechanisms and optional background execution.

## Objective

Enhance the user experience for Claude CLI command execution by providing real-time feedback, progress indicators, and optional background job management for long-running operations.

## Requirements

### Functional Requirements
- Real-time streaming of Claude CLI output as it becomes available
- Visual progress indicators during command execution
- Background job execution option for long-running commands
- Job management commands to monitor and retrieve results
- Automatic job history and log retention
- Configurable timeouts and cancellation support

### Non-Functional Requirements
- Minimal performance overhead for streaming output
- Reliable job state persistence across mmm restarts
- Clean and intuitive command-line interface
- Backward compatibility with existing Claude CLI integration

## Acceptance Criteria

- [ ] Users see immediate feedback when Claude CLI commands start executing
- [ ] Output streams to the terminal in real-time instead of being buffered
- [ ] Progress indicators show elapsed time and command status
- [ ] Background execution can be triggered with `--background` flag
- [ ] `mmm claude jobs` command lists all running and recent jobs
- [ ] `mmm claude watch <job-id>` shows real-time output of a running job
- [ ] `mmm claude logs <job-id>` retrieves output of completed jobs
- [ ] Jobs persist across mmm restarts with proper state recovery
- [ ] Long-running commands can be cancelled with `mmm claude cancel <job-id>`
- [ ] Job history is automatically cleaned up after configurable retention period

## Technical Details

### Implementation Approach

1. **Streaming Output Handler**
   - Replace `Command::output()` with `Command::spawn()` for real-time streaming
   - Use `tokio::io::AsyncBufReadExt` to read stdout/stderr line by line
   - Implement progress indicator using `indicatif` crate or similar
   - Show elapsed time and optional spinner during execution

2. **Background Job System**
   - Create `ClaudeJob` struct to track job state and metadata
   - Store jobs in SQLite with schema:
     ```sql
     CREATE TABLE claude_jobs (
         id TEXT PRIMARY KEY,
         command TEXT NOT NULL,
         args TEXT NOT NULL,
         status TEXT NOT NULL,
         started_at TIMESTAMP NOT NULL,
         completed_at TIMESTAMP,
         exit_code INTEGER,
         output_path TEXT,
         error_path TEXT
     );
     ```
   - Implement job lifecycle: pending → running → completed/failed
   - Use tokio tasks for background execution

3. **Job Management Commands**
   - Add new `ClaudeJobCommands` enum to CLI structure
   - Implement job listing with filtering options
   - Create tail-like functionality for watching active jobs
   - Support job cancellation via process termination

### Architecture Changes

1. **New Module**: `src/claude/jobs.rs`
   - `JobManager` struct for job lifecycle management
   - `JobExecutor` for running jobs in background
   - `OutputStreamer` for real-time output handling

2. **CLI Extensions**:
   ```rust
   enum ClaudeCommands {
       Run { 
           command: String, 
           args: Vec<String>,
           #[arg(long)]
           background: bool,
       },
       Jobs {
           #[arg(long)]
           status: Option<JobStatus>,
           #[arg(long)]
           limit: Option<usize>,
       },
       Watch { job_id: String },
       Logs { job_id: String },
       Cancel { job_id: String },
   }
   ```

3. **State Management Integration**
   - Extend StateManager to handle job persistence
   - Add job cleanup on startup for orphaned jobs
   - Implement job retention policies

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeJob {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub output_path: Option<PathBuf>,
    pub error_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
```

### APIs and Interfaces

```rust
pub trait JobManager {
    async fn create_job(&self, command: &str, args: Vec<String>) -> Result<ClaudeJob>;
    async fn start_job(&self, job_id: &str) -> Result<()>;
    async fn get_job(&self, job_id: &str) -> Result<Option<ClaudeJob>>;
    async fn list_jobs(&self, filter: JobFilter) -> Result<Vec<ClaudeJob>>;
    async fn cancel_job(&self, job_id: &str) -> Result<()>;
    async fn cleanup_old_jobs(&self, retention_days: u32) -> Result<usize>;
}

pub trait OutputStreamer {
    async fn stream_output(&self, child: &mut Child) -> Result<()>;
    async fn save_output(&self, job_id: &str, output: &str) -> Result<PathBuf>;
}
```

## Dependencies

- **Prerequisites**: 
  - Specification 03 (Claude Integration) must be implemented
  - SQLite state management system must be operational
  
- **Affected Components**: 
  - `src/claude/mod.rs` - Add job management exports
  - `src/main.rs` - Extend CLI command handling
  - `src/state/mod.rs` - Add job-related database operations
  
- **External Dependencies**: 
  - `indicatif` crate for progress indicators
  - Enhanced `tokio` features for process management

## Testing Strategy

- **Unit Tests**: 
  - Job state transitions and persistence
  - Output streaming buffer handling
  - Job filtering and listing logic
  
- **Integration Tests**: 
  - Full job lifecycle from creation to completion
  - Concurrent job execution
  - Job recovery after mmm restart
  
- **Performance Tests**: 
  - Streaming overhead measurement
  - Database query performance with many jobs
  
- **User Acceptance**: 
  - Manual testing of all job commands
  - Verification of real-time output streaming
  - Background job reliability testing

## Documentation Requirements

- **Code Documentation**: 
  - Document job lifecycle and state transitions
  - Explain streaming implementation details
  - Add examples for each job command
  
- **User Documentation**: 
  - Update CLI help text with new options
  - Add job management guide to README
  - Include troubleshooting section
  
- **Architecture Updates**: 
  - Add job system to architecture diagram
  - Document database schema changes

## Implementation Notes

1. **Output Buffering**: Consider implementing a ring buffer for job output to prevent unlimited memory growth
2. **File Storage**: For large outputs, consider streaming directly to files instead of keeping in memory
3. **Progress Estimation**: While we can't know Claude's actual progress, show elapsed time and data transfer rates
4. **Error Recovery**: Handle cases where Claude CLI crashes or hangs
5. **Security**: Ensure job outputs don't leak sensitive information between projects

## Migration and Compatibility

- Existing Claude CLI integration remains unchanged by default
- Background execution is opt-in via `--background` flag
- Job system tables are created automatically on first use
- No breaking changes to existing mmm configurations
- Consider adding `mmm claude migrate-jobs` for future schema updates