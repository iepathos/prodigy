# Specification 58: Session State Management Refactor

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [56-cook-orchestrator-refactor]

## Context

Session state management is currently intertwined with the cook module, making it difficult to test and reuse. The current implementation:

- Mixes session tracking with execution logic
- Has inconsistent state updates
- Lacks clear boundaries between session, iteration, and workflow state
- Makes it hard to implement features like session resume and state recovery
- Provides limited visibility into session progress

A clean session management abstraction would enable better testing, clearer state transitions, and support for advanced features like session persistence and recovery.

## Objective

Extract session state management into a dedicated, testable component with clear state transitions, event-driven updates, and support for persistence and recovery.

## Requirements

### Functional Requirements
- Track all session lifecycle events (start, iterate, complete, fail)
- Support both direct and worktree execution modes
- Enable session persistence and recovery
- Provide session progress and status queries
- Support concurrent session tracking
- Track per-iteration metrics and changes

### Non-Functional Requirements
- Thread-safe state management
- Event-driven architecture for state changes
- Minimal memory footprint
- Support for state observers/listeners
- Clean API for state queries

## Acceptance Criteria

- [ ] Session state completely decoupled from cook module
- [ ] All state transitions clearly defined and documented
- [ ] 95% unit test coverage for session management
- [ ] Support for session persistence to disk
- [ ] Event system for state change notifications
- [ ] Session recovery from persisted state
- [ ] Concurrent session support with isolation

## Technical Details

### Implementation Approach

1. **State Machine Design**
   ```rust
   // Core session states
   #[derive(Debug, Clone, PartialEq)]
   pub enum SessionState {
       Created,
       Running { iteration: u32 },
       Paused { reason: String },
       Completed { summary: SessionSummary },
       Failed { error: String },
   }

   // State transitions
   pub enum SessionEvent {
       Started { config: SessionConfig },
       IterationStarted { number: u32 },
       IterationCompleted { changes: IterationChanges },
       AnalysisCompleted { results: AnalysisResult },
       CommandExecuted { command: String, success: bool },
       Paused { reason: String },
       Resumed,
       Completed,
       Failed { error: String },
   }
   ```

2. **Session Manager Interface**
   ```rust
   #[async_trait]
   pub trait SessionManager: Send + Sync {
       // Lifecycle management
       async fn create_session(&self, config: SessionConfig) -> Result<SessionId>;
       async fn start_session(&self, id: SessionId) -> Result<()>;
       async fn complete_session(&self, id: SessionId) -> Result<SessionSummary>;
       
       // State updates
       async fn record_event(&self, id: SessionId, event: SessionEvent) -> Result<()>;
       async fn get_state(&self, id: SessionId) -> Result<SessionState>;
       
       // Queries
       async fn get_progress(&self, id: SessionId) -> Result<SessionProgress>;
       async fn list_active_sessions(&self) -> Result<Vec<SessionInfo>>;
       
       // Persistence
       async fn save_checkpoint(&self, id: SessionId) -> Result<()>;
       async fn restore_session(&self, id: SessionId) -> Result<()>;
   }
   ```

3. **Event-Driven Updates**
   ```rust
   #[async_trait]
   pub trait SessionObserver: Send + Sync {
       async fn on_event(&self, session_id: SessionId, event: &SessionEvent);
   }

   pub struct ObservableSessionManager {
       inner: Box<dyn SessionManager>,
       observers: Arc<RwLock<Vec<Box<dyn SessionObserver>>>>,
   }
   ```

### Architecture Changes

1. **Session Configuration**
   ```rust
   pub struct SessionConfig {
       pub project_path: PathBuf,
       pub workflow: WorkflowConfig,
       pub execution_mode: ExecutionMode,
       pub max_iterations: u32,
       pub focus: Option<String>,
       pub options: SessionOptions,
   }

   pub enum ExecutionMode {
       Direct,
       Worktree { name: String },
   }

   pub struct SessionOptions {
       pub fail_fast: bool,
       pub auto_merge: bool,
       pub collect_metrics: bool,
       pub verbose: bool,
   }
   ```

2. **Session Progress Tracking**
   ```rust
   pub struct SessionProgress {
       pub state: SessionState,
       pub iterations_completed: u32,
       pub total_iterations: u32,
       pub files_changed: HashSet<PathBuf>,
       pub commands_executed: Vec<ExecutedCommand>,
       pub duration: Duration,
       pub current_phase: Option<String>,
   }

   pub struct IterationChanges {
       pub files_modified: Vec<PathBuf>,
       pub lines_added: usize,
       pub lines_removed: usize,
       pub commands_run: Vec<String>,
       pub git_commits: Vec<CommitInfo>,
   }
   ```

3. **Persistence Layer**
   ```rust
   pub struct PersistedSession {
       pub id: SessionId,
       pub config: SessionConfig,
       pub state: SessionState,
       pub events: Vec<TimestampedEvent>,
       pub checkpoints: Vec<SessionCheckpoint>,
   }

   pub struct SessionCheckpoint {
       pub iteration: u32,
       pub timestamp: DateTime<Utc>,
       pub state_snapshot: StateSnapshot,
       pub resumable: bool,
   }
   ```

### Data Structures

1. **Session Storage Backend**
   ```rust
   #[async_trait]
   pub trait SessionStorage: Send + Sync {
       async fn save(&self, session: &PersistedSession) -> Result<()>;
       async fn load(&self, id: SessionId) -> Result<Option<PersistedSession>>;
       async fn list(&self) -> Result<Vec<SessionId>>;
       async fn delete(&self, id: SessionId) -> Result<()>;
   }

   // File-based implementation
   pub struct FileSessionStorage {
       base_path: PathBuf,
   }
   ```

2. **In-Memory State Tracking**
   ```rust
   pub struct InMemorySessionManager {
       sessions: Arc<RwLock<HashMap<SessionId, SessionData>>>,
       storage: Option<Box<dyn SessionStorage>>,
   }

   struct SessionData {
       config: SessionConfig,
       state: SessionState,
       events: Vec<TimestampedEvent>,
       metrics: SessionMetrics,
   }
   ```

## Dependencies

- **Prerequisites**: [56-cook-orchestrator-refactor]
- **Affected Components**: 
  - cook module (primary consumer)
  - worktree management
  - metrics collection
  - CLI progress display
- **External Dependencies**: None new

## Testing Strategy

- **Unit Tests**: 
  - State machine transition tests
  - Event recording and replay
  - Concurrent session management
  - Persistence and recovery
- **Integration Tests**: 
  - Full session lifecycle
  - Multi-session scenarios
  - Recovery from crashes
- **Performance Tests**: 
  - Large event stream handling
  - Concurrent session overhead
  - Persistence performance

## Documentation Requirements

- **Code Documentation**: 
  - State machine diagram
  - Event flow documentation
  - API usage examples
- **Architecture Updates**: 
  - Update ARCHITECTURE.md with session management
  - Document state persistence format
- **User Guide**: 
  - Session recovery procedures
  - Progress monitoring

## Implementation Notes

1. **State Machine Invariants**
   - Sessions must start in Created state
   - Can only transition to Running from Created or Paused
   - Terminal states (Completed, Failed) are final
   - All transitions must generate events

2. **Concurrency Considerations**
   - Use fine-grained locking per session
   - Event observers called asynchronously
   - Storage operations are async
   - Support multiple readers, single writer

3. **Error Recovery**
   - Graceful handling of storage failures
   - Automatic state recovery on restart
   - Corruption detection and repair
   - Event log replay for consistency

## Migration and Compatibility

1. **Backward Compatibility**
   - Support existing session tracking
   - Migrate state format incrementally
   - Preserve existing behavior

2. **Migration Path**
   - Phase 1: Extract session types
   - Phase 2: Implement new manager
   - Phase 3: Migrate cook module
   - Phase 4: Add persistence support

3. **Feature Flags**
   - Enable new session manager optionally
   - Support rollback if issues arise
   - Gradual rollout of features