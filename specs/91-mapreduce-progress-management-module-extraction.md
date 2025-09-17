---
number: 91
title: MapReduce Progress Management Module Extraction
category: optimization
priority: medium
status: draft
dependencies: [87, 88]
created: 2025-09-17
---

# Specification 91: MapReduce Progress Management Module Extraction

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [87 - Agent Module, 88 - Command Module]

## Context

Progress tracking and display logic is currently embedded throughout the MapReduce executor, with multiple progress tracking systems (legacy and enhanced) coexisting. The ProgressTracker struct and its implementation span 120+ lines within the main module. Progress updates are scattered across execution methods, making it difficult to maintain consistent progress reporting or add new visualization options.

## Objective

Extract all progress tracking and display functionality into a dedicated module that provides a unified interface for progress monitoring across all MapReduce operations. This will enable consistent progress reporting, easier addition of new progress visualizations, and better separation of progress concerns from execution logic.

## Requirements

### Functional Requirements
- Unify legacy and enhanced progress tracking systems
- Extract progress bar creation and management
- Centralize agent operation status updates
- Support multiple concurrent progress displays
- Enable progress streaming for external consumers
- Maintain backward compatibility with existing progress output

### Non-Functional Requirements
- Minimize performance impact of progress updates
- Support headless operation without terminal
- Enable progress metrics collection
- Ensure thread-safe progress updates
- Support configurable update frequencies

## Acceptance Criteria

- [ ] Progress module created at `src/cook/execution/mapreduce/progress/`
- [ ] ProgressTracker moved to `progress/tracker.rs`
- [ ] Progress display logic in `progress/display.rs`
- [ ] Agent operations tracking in `progress/operations.rs`
- [ ] Progress streaming API in `progress/stream.rs`
- [ ] All progress code removed from main module
- [ ] Main module reduced by approximately 400 lines
- [ ] Progress displays work identically to current
- [ ] New progress streaming API documented
- [ ] Support for custom progress renderers

## Technical Details

### Implementation Approach

1. **Module Structure**:
   ```
   src/cook/execution/mapreduce/progress/
   ├── mod.rs          # Module exports and ProgressManager
   ├── tracker.rs      # Core progress tracking logic
   ├── display.rs      # Terminal and UI rendering
   ├── operations.rs   # Agent operation tracking
   └── stream.rs       # Progress event streaming
   ```

2. **Key Extractions**:
   - `ProgressTracker` struct and impl → `tracker.rs`
   - Progress bar creation and styling → `display.rs`
   - Agent operation updates → `operations.rs`
   - Progress update coordination → `mod.rs`

### Architecture Changes

- Implement observer pattern for progress updates
- Use async channels for progress streaming
- Create progress renderer trait for extensibility
- Separate progress data from presentation

### Data Structures

```rust
pub trait ProgressRenderer: Send + Sync {
    fn render(&self, state: &ProgressState) -> Result<(), RenderError>;
    fn supports_terminal(&self) -> bool;
}

pub struct ProgressManager {
    state: Arc<RwLock<ProgressState>>,
    renderers: Vec<Box<dyn ProgressRenderer>>,
    update_channel: mpsc::Sender<ProgressUpdate>,
}

pub struct ProgressState {
    total_items: usize,
    completed_items: usize,
    agent_states: HashMap<usize, AgentProgress>,
    start_time: Instant,
}

pub enum ProgressUpdate {
    ItemComplete(usize),
    AgentStatus(usize, AgentOperation),
    PhaseChange(PhaseType),
    Error(String),
}
```

### APIs and Interfaces

```rust
impl ProgressManager {
    pub fn new(config: ProgressConfig) -> Self;

    pub async fn start_tracking(&self, total_items: usize);

    pub fn update_agent(&self, index: usize, operation: AgentOperation);

    pub fn item_complete(&self, item_id: &str);

    pub fn subscribe(&self) -> mpsc::Receiver<ProgressEvent>;
}

pub trait ProgressStreamConsumer {
    async fn consume(&mut self, event: ProgressEvent) -> Result<(), StreamError>;
}
```

## Dependencies

- **Prerequisites**:
  - Phase 1: Utils module (completed)
  - Phase 2: Agent module (spec 87)
  - Phase 3: Command module (spec 88)
- **Affected Components**: Agent execution, phase execution, UI
- **External Dependencies**: indicatif, tokio channels

## Testing Strategy

- **Unit Tests**: Test progress state transitions
- **Display Tests**: Verify progress rendering (mock terminal)
- **Stream Tests**: Validate event streaming
- **Performance Tests**: Measure update overhead
- **Integration Tests**: Full progress flow with execution

## Documentation Requirements

- **Code Documentation**: Progress API documentation
- **User Guide**: Configuring progress display
- **Developer Guide**: Creating custom progress renderers
- **Architecture Updates**: Progress system architecture

## Implementation Notes

- Maintain backward compatibility with current display
- Use buffering to reduce terminal update frequency
- Consider using crossbeam channels for performance
- Implement graceful degradation for non-terminal environments
- Add progress persistence for resume scenarios
- Include timing and rate calculations

## Migration and Compatibility

- No visible changes to progress output
- Internal refactoring only
- Existing progress bars continue to work
- New streaming API is additive only
- Consider feature flag for new progress system