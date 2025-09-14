---
number: 72
title: Enhanced Progress Visualization and Monitoring
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 72: Enhanced Progress Visualization and Monitoring

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The whitepaper emphasizes "Progress tracking" as a core feature, mentioning:
- "Progress visualization"
- "No progress tracking - Long operations provide no visibility into status"
- "Progress shows items completed/remaining"

Currently, progress indication is basic and doesn't provide the rich, informative visualization needed for long-running MapReduce operations.

## Objective

Implement comprehensive progress visualization that provides real-time insights into workflow execution, including ETA calculation, resource usage, parallel execution status, and detailed per-item progress for MapReduce operations.

## Requirements

### Functional Requirements
- Real-time progress bars for all operations
- ETA calculation based on historical data
- Multi-level progress (workflow, phase, step, item)
- Parallel execution visualization
- Resource usage monitoring (CPU, memory, disk)
- Success/failure rate tracking
- Live log streaming with filtering
- Progress persistence for resume
- Web dashboard option
- CLI and TUI (Terminal UI) modes

### Non-Functional Requirements
- Minimal performance overhead (<1%)
- Smooth updates without flicker
- Responsive to terminal resize
- Accessible output for CI/CD systems
- Machine-readable progress format

## Acceptance Criteria

- [ ] Progress bars show percentage, items, and ETA
- [ ] MapReduce shows per-agent progress
- [ ] Resource usage displayed in real-time
- [ ] Success/failure rates visible
- [ ] Log filtering by level and source
- [ ] Progress survives terminal disconnect
- [ ] Web dashboard accessible at localhost:8080
- [ ] JSON progress output for automation
- [ ] Color-coded status indicators
- [ ] Historical trend visualization

## Technical Details

### Implementation Approach

1. **Progress Tracking Architecture**:
   ```rust
   pub struct ProgressTracker {
       workflow_progress: Arc<RwLock<WorkflowProgress>>,
       phase_progress: Arc<RwLock<HashMap<String, PhaseProgress>>>,
       item_progress: Arc<RwLock<HashMap<String, ItemProgress>>>,
       metrics: Arc<SystemMetrics>,
       history: Arc<ProgressHistory>,
       renderer: Box<dyn ProgressRenderer>,
   }

   #[derive(Debug, Clone, Serialize)]
   pub struct WorkflowProgress {
       pub id: String,
       pub name: String,
       pub status: WorkflowStatus,
       pub start_time: Instant,
       pub eta: Option<Duration>,
       pub total_steps: usize,
       pub completed_steps: usize,
       pub failed_steps: usize,
       pub current_phase: Option<String>,
       pub resource_usage: ResourceUsage,
   }

   #[derive(Debug, Clone, Serialize)]
   pub struct PhaseProgress {
       pub name: String,
       pub phase_type: PhaseType,
       pub status: PhaseStatus,
       pub start_time: Instant,
       pub total_items: usize,
       pub processed_items: usize,
       pub successful_items: usize,
       pub failed_items: usize,
       pub active_agents: Vec<AgentProgress>,
       pub throughput: f64, // items per second
       pub avg_item_time: Duration,
   }

   #[derive(Debug, Clone, Serialize)]
   pub struct AgentProgress {
       pub id: String,
       pub worktree: String,
       pub current_item: Option<String>,
       pub status: AgentStatus,
       pub items_processed: usize,
       pub start_time: Instant,
       pub last_update: Instant,
       pub current_step: Option<String>,
       pub memory_usage: usize,
       pub cpu_usage: f32,
   }
   ```

2. **Multi-Progress Display Manager**:
   ```rust
   pub struct MultiProgressDisplay {
       multi_progress: MultiProgress,
       main_bar: ProgressBar,
       phase_bars: HashMap<String, ProgressBar>,
       agent_bars: HashMap<String, ProgressBar>,
       log_area: ProgressBar,
       update_interval: Duration,
   }

   impl MultiProgressDisplay {
       pub fn new(mode: DisplayMode) -> Self {
           let multi_progress = MultiProgress::new();

           // Main workflow progress
           let main_bar = multi_progress.add(ProgressBar::new(100));
           main_bar.set_style(
               ProgressStyle::default_bar()
                   .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) | {msg}")
                   .progress_chars("#>-")
           );

           // Initialize display
           let display = Self {
               multi_progress,
               main_bar,
               phase_bars: HashMap::new(),
               agent_bars: HashMap::new(),
               log_area: ProgressBar::new(0),
               update_interval: Duration::from_millis(100),
           };

           display.start_update_loop();
           display
       }

       pub fn update_workflow(&self, progress: &WorkflowProgress) {
           let percentage = (progress.completed_steps as f64
                            / progress.total_steps as f64 * 100.0) as u64;

           self.main_bar.set_position(percentage);

           let eta_str = progress.eta
               .map(|d| format!("ETA: {}", humantime::format_duration(d)))
               .unwrap_or_else(|| "ETA: calculating...".to_string());

           self.main_bar.set_message(format!(
               "{} | {} | CPU: {:.1}% | Mem: {}",
               progress.current_phase.as_ref().unwrap_or(&"Starting".to_string()),
               eta_str,
               progress.resource_usage.cpu_percent,
               humanize_bytes(progress.resource_usage.memory_bytes)
           ));
       }

       pub fn add_phase(&mut self, phase_id: &str, total_items: usize) -> ProgressBar {
           let bar = self.multi_progress.add(ProgressBar::new(total_items as u64));
           bar.set_style(
               ProgressStyle::default_bar()
                   .template("  {spinner:.blue} [{bar:30.yellow/red}] {pos}/{len} {msg}")
                   .progress_chars("=>-")
           );
           self.phase_bars.insert(phase_id.to_string(), bar.clone());
           bar
       }

       pub fn add_agent(&mut self, agent_id: &str) -> ProgressBar {
           let bar = self.multi_progress.add(ProgressBar::new_spinner());
           bar.set_style(
               ProgressStyle::default_spinner()
                   .template("    {spinner:.green} Agent {prefix}: {msg}")
           );
           bar.set_prefix(agent_id.to_string());
           self.agent_bars.insert(agent_id.to_string(), bar.clone());
           bar
       }
   }
   ```

3. **ETA Calculator**:
   ```rust
   pub struct ETACalculator {
       history: VecDeque<TimePoint>,
       window_size: usize,
   }

   #[derive(Clone)]
   struct TimePoint {
       timestamp: Instant,
       items_completed: usize,
   }

   impl ETACalculator {
       pub fn calculate_eta(
           &mut self,
           current: usize,
           total: usize,
           now: Instant,
       ) -> Option<Duration> {
           // Add current point
           self.history.push_back(TimePoint {
               timestamp: now,
               items_completed: current,
           });

           // Maintain window
           while self.history.len() > self.window_size {
               self.history.pop_front();
           }

           // Need at least 2 points
           if self.history.len() < 2 {
               return None;
           }

           // Calculate rate
           let first = &self.history[0];
           let elapsed = now - first.timestamp;
           let items_done = current - first.items_completed;

           if items_done == 0 {
               return None;
           }

           let rate = items_done as f64 / elapsed.as_secs_f64();
           let remaining = total - current;

           if rate > 0.0 {
               Some(Duration::from_secs_f64(remaining as f64 / rate))
           } else {
               None
           }
       }
   }
   ```

4. **Web Dashboard Server**:
   ```rust
   pub struct DashboardServer {
       progress_tracker: Arc<ProgressTracker>,
       port: u16,
   }

   impl DashboardServer {
       pub async fn start(&self) -> Result<()> {
           let tracker = self.progress_tracker.clone();

           let app = Router::new()
               .route("/", get(serve_dashboard))
               .route("/api/progress", get(progress_endpoint))
               .route("/api/logs", get(logs_endpoint))
               .route("/ws", get(websocket_handler))
               .with_state(tracker);

           let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
           info!("Dashboard available at http://localhost:{}", self.port);

           axum::Server::bind(&addr)
               .serve(app.into_make_service())
               .await?;

           Ok(())
       }

       async fn progress_endpoint(
           State(tracker): State<Arc<ProgressTracker>>,
       ) -> Json<ProgressSnapshot> {
           Json(tracker.snapshot().await)
       }

       async fn websocket_handler(
           ws: WebSocketUpgrade,
           State(tracker): State<Arc<ProgressTracker>>,
       ) -> Response {
           ws.on_upgrade(move |socket| handle_socket(socket, tracker))
       }
   }
   ```

### Architecture Changes
- Add `ProgressTracker` as core component
- Integrate with all executors
- Add web dashboard server
- Implement TUI mode
- Add metrics collection system

### Data Structures
```yaml
# Progress configuration
progress:
  display_mode: rich  # rich, simple, json, none
  update_interval: 100ms
  show_resource_usage: true
  enable_dashboard: true
  dashboard_port: 8080
  log_level: info
  eta_window_size: 20  # samples for ETA calculation

# Example progress output
┌─ Workflow: modernize-codebase ─────────────────────────────┐
│ ▶ [████████████░░░░░░░░░░░░] 45% | ETA: 5m 23s            │
│ Phase: Map (10/20 agents active)                           │
│ CPU: 65.2% | Memory: 1.2 GB | Disk: +450 MB               │
├─────────────────────────────────────────────────────────────┤
│ ▶ Setup phase [████████████████████] 100% Complete        │
│ ▶ Map phase   [██████░░░░░░░░░░░░] 35% (175/500 items)   │
│   ├─ Agent-01: Processing file_123.py...                   │
│   ├─ Agent-02: ✓ Completed file_124.py                    │
│   ├─ Agent-03: ✗ Failed file_125.py (retry 1/3)          │
│   └─ ... 7 more agents                                     │
│ ○ Reduce phase: Waiting...                                 │
├─────────────────────────────────────────────────────────────┤
│ Success Rate: 89% (156/175) | Throughput: 2.3 items/sec   │
│ Failed Items: 19 (will retry) | DLQ: 0                    │
└─────────────────────────────────────────────────────────────┘
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/execution/progress.rs` - Core progress tracking
  - `src/cli/` - CLI display integration
  - `src/cook/execution/mapreduce.rs` - MapReduce progress
- **External Dependencies**:
  - `indicatif` for progress bars
  - `axum` for web dashboard
  - `tui` for terminal UI

## Testing Strategy

- **Unit Tests**:
  - ETA calculation accuracy
  - Progress state management
  - Metric collection
  - Display formatting
- **Integration Tests**:
  - End-to-end progress tracking
  - Dashboard API endpoints
  - WebSocket updates
  - Multi-phase workflows
- **Performance Tests**:
  - Update overhead measurement
  - Large-scale progress tracking
  - Dashboard scalability
  - Terminal rendering performance

## Documentation Requirements

- **Code Documentation**: Document progress architecture
- **User Documentation**:
  - Progress display modes guide
  - Dashboard usage
  - Customization options
  - CI/CD integration
- **Architecture Updates**: Add progress system to architecture

## Implementation Notes

- Use double-buffering for flicker-free updates
- Support NO_COLOR environment variable
- Graceful degradation in non-TTY environments
- Consider progress persistence for crash recovery
- Future: Distributed progress aggregation

## Migration and Compatibility

- Default to simple progress for compatibility
- Rich progress opt-in via configuration
- Machine-readable output maintained
- Backwards compatible with CI systems