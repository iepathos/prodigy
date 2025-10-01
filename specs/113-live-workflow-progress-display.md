---
number: 113
title: Live Workflow Progress Display
category: foundation
priority: high
status: draft
dependencies: [110]
created: 2025-10-01
---

# Specification 113: Live Workflow Progress Display

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [110 - Terminal UI Foundation]

## Context

Current workflow execution (`prodigy run`) provides basic progress messages but lacks:
- Real-time visibility into MapReduce execution
- Individual agent status and progress
- Resource usage monitoring (CPU, memory)
- Throughput metrics and ETAs
- Phase-based progress tracking
- Visual feedback during long-running operations

Users cannot effectively monitor parallel agent execution or troubleshoot performance issues during MapReduce workflows.

## Objective

Implement comprehensive live progress displays for workflow execution, including multi-agent MapReduce dashboards, real-time resource monitoring, phase-based progress tracking, and event streaming using the terminal UI foundation.

## Requirements

### Functional Requirements

**FR1**: Enhanced workflow startup display
- Box display showing workflow metadata
- Mode, max parallel agents, total items
- Job ID for tracking
- Phase breakdown (setup, map, reduce)
- Estimated duration (if available from history)

**FR2**: Setup phase progress
- Checklist of setup steps with status indicators
- ✓ for complete, ⏳ for in-progress
- Show work item loading and filtering
- Display item count after filtering
- Show sort/priority information

**FR3**: MapReduce map phase dashboard
- Overall progress bar with percentage and ETA
- Active agent count and status summary
- Individual agent progress bars (up to 10 visible)
- Real-time throughput metrics (items/sec)
- Resource usage (CPU %, memory usage)
- Recent completions log (scrolling list)
- Success/failure counters
- Keyboard controls hint

**FR4**: Agent status display
- Agent ID/number
- Current item being processed
- Current step/operation
- Progress bar for current item
- Time spent on current item
- Status indicator (⏳ running, ✓ complete, ✗ failed)

**FR5**: Resource monitoring
- CPU usage percentage with bar
- Memory usage (used / total) with bar
- Disk I/O sparkline (if available)
- Peak resource values
- Warning indicators if resources constrained

**FR6**: Reduce phase progress
- Linear progress through reduce steps
- Checklist of reduce operations
- Status for each operation
- Duration estimates
- Final commit status

**FR7**: Workflow completion summary
- Box display with complete statistics
- Phase breakdowns (setup, map, reduce) with timings
- Success/failure counts and percentages
- Average time per item
- Peak throughput achieved
- Resource usage summary
- Output locations (results, DLQ, events)
- Suggested next actions

**FR8**: Live event streaming (`prodigy events follow`)
- Real-time event tail with colored output
- Timestamp with millisecond precision
- Event type with color coding
- Agent ID and context
- Indented details for complex events
- Filter by event type or agent
- Keyboard control (Ctrl+C to stop)

**FR9**: Enhanced error display
- Boxed error messages with context
- Error type and primary message
- Affected resource (file, worktree, etc.)
- Detailed explanation
- Step-by-step resolution instructions
- Alternative actions
- Related command suggestions

### Non-Functional Requirements

**NFR1**: Performance - UI updates at 5-10 Hz without flickering
**NFR2**: Responsiveness - No UI blocking during updates
**NFR3**: Scalability - Handles 100+ parallel agents gracefully
**NFR4**: Reliability - Progress tracking resilient to failures
**NFR5**: Usability - Clear visual hierarchy and readable at a glance

## Acceptance Criteria

- [ ] Workflow startup displays all metadata in formatted box
- [ ] Setup phase shows checklist with real-time updates
- [ ] Map phase dashboard shows overall progress bar
- [ ] Individual agent progress bars update smoothly
- [ ] Throughput metrics calculate correctly
- [ ] Resource monitoring shows CPU and memory usage
- [ ] Recent completions log updates in real-time
- [ ] Success/failure counters accurate
- [ ] Keyboard controls work (q, p, l)
- [ ] Reduce phase progress displays all steps
- [ ] Completion summary shows comprehensive statistics
- [ ] Phase timing breakdowns accurate
- [ ] Output locations displayed correctly
- [ ] Next action suggestions helpful
- [ ] Event streaming shows colored, formatted events
- [ ] Event timestamps precise to milliseconds
- [ ] Event filtering works correctly
- [ ] Error messages display in formatted boxes
- [ ] Resolution steps clear and actionable
- [ ] UI updates don't flicker or jump
- [ ] Handles agent failures gracefully
- [ ] Works with different terminal sizes (80+ cols)
- [ ] Non-interactive mode falls back gracefully

## Technical Details

### Implementation Approach

**Phase 1: Workflow Startup and Setup**
1. Implement workflow metadata display
2. Create setup phase checklist
3. Add work item loading visualization
4. Implement filtering/sorting display

**Phase 2: MapReduce Dashboard**
1. Create multi-progress display structure
2. Implement overall progress bar
3. Add agent progress bars with updates
4. Integrate resource monitoring
5. Add recent completions log

**Phase 3: Completion and Summaries**
1. Implement reduce phase progress
2. Create completion summary display
3. Add statistics calculation
4. Implement next action suggestions

**Phase 4: Event Streaming and Errors**
1. Implement live event tail functionality
2. Add event filtering
3. Create enhanced error display
4. Add resolution guidance system

### Module Structure

```rust
src/cook/execution/progress/
├── mod.rs                  // Public API
├── workflow_display.rs     // Workflow-level progress
├── mapreduce_dashboard.rs  // MapReduce live dashboard
├── agent_tracker.rs        // Individual agent tracking
├── resource_monitor.rs     // CPU/memory monitoring
├── event_stream.rs         // Live event streaming
└── summary_formatter.rs    // Completion summaries

src/cook/interaction/
├── error_display.rs        // Enhanced error formatting
└── guidance.rs             // Resolution guidance system
```

### Key Data Structures

```rust
// Workflow progress state
pub struct WorkflowProgress {
    pub job_id: String,
    pub mode: WorkflowMode,
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub active_agents: usize,
    pub current_phase: Phase,
    pub start_time: Instant,
    pub throughput: f64,
    pub eta: Option<Duration>,
}

// Agent tracking
pub struct AgentTracker {
    pub agent_id: String,
    pub worktree: String,
    pub current_item: Option<String>,
    pub current_step: Option<String>,
    pub progress: f32,
    pub start_time: Instant,
    pub items_processed: usize,
    pub status: AgentStatus,
}

pub enum AgentStatus {
    Starting,
    Running,
    Waiting,
    Complete,
    Failed,
}

// Resource monitoring
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub disk_io: Vec<u64>, // For sparkline
}

// Event stream
pub struct StreamedEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub agent_id: Option<String>,
    pub message: String,
    pub details: Option<serde_json::Value>,
}
```

### Display Implementation

**Multi-Progress Dashboard:**
```rust
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub struct MapReduceDashboard {
    multi: MultiProgress,
    overall_bar: ProgressBar,
    agent_bars: HashMap<String, ProgressBar>,
    stats_bar: ProgressBar,
    log_bar: ProgressBar,
}

impl MapReduceDashboard {
    pub fn new(total_items: usize, max_agents: usize) -> Self {
        let multi = MultiProgress::new();

        // Overall progress
        let overall = multi.add(ProgressBar::new(total_items as u64));
        overall.set_style(
            ProgressStyle::default_bar()
                .template("  Overall    [█████████████████████░░░░░░░░░░░]  67/142  47%   ⏱ 5m 32s  ETA: 6m 15s")
                .progress_chars("█▓░")
        );

        // Stats line
        let stats = multi.add(ProgressBar::new_spinner());
        stats.set_style(
            ProgressStyle::default_spinner()
                .template("  Active Agents: {prefix}     Success: {msg}")
        );

        // Agent bars (dynamically added)
        let agent_bars = HashMap::new();

        // Log area
        let log = multi.add(ProgressBar::new_spinner());
        log.set_style(
            ProgressStyle::default_spinner()
                .template("  Recent: {msg}")
        );

        Self {
            multi,
            overall_bar: overall,
            agent_bars,
            stats_bar: stats,
            log_bar: log,
        }
    }

    pub fn update(&mut self, progress: &WorkflowProgress) {
        self.overall_bar.set_position(progress.completed_items as u64);
        self.stats_bar.set_prefix(format!("{}", progress.active_agents));
        self.stats_bar.set_message(
            format!("{}  •  Failed: {}", progress.completed_items, progress.failed_items)
        );
    }

    pub fn add_agent(&mut self, agent_id: &str) {
        let bar = self.multi.add(ProgressBar::new(100));
        bar.set_style(
            ProgressStyle::default_bar()
                .template("  {prefix} [{bar:10}] {msg} {elapsed_precise}")
                .progress_chars("█▓░")
        );
        bar.set_prefix(format!("Agent-{}", agent_id));
        self.agent_bars.insert(agent_id.to_string(), bar);
    }
}
```

**Resource Monitor:**
```rust
use sysinfo::{System, SystemExt, ProcessExt};

pub struct ResourceMonitor {
    system: System,
    samples: VecDeque<ResourceUsage>,
    max_samples: usize,
}

impl ResourceMonitor {
    pub fn sample(&mut self) -> ResourceUsage {
        self.system.refresh_all();

        let cpu = self.system.global_cpu_info().cpu_usage();
        let memory_used = self.system.used_memory();
        let memory_total = self.system.total_memory();

        ResourceUsage {
            cpu_percent: cpu,
            memory_used,
            memory_total,
            disk_io: vec![], // TODO: Implement disk I/O sampling
        }
    }

    pub fn display(&self) -> String {
        let latest = self.samples.back().unwrap();
        let cpu_bar = create_bar(latest.cpu_percent as usize, 100, 30);
        let mem_percent = (latest.memory_used * 100) / latest.memory_total;
        let mem_bar = create_bar(mem_percent as usize, 100, 30);

        format!(
            "  CPU:     [{}] {:.1}%\n  Memory:  [{}] {} / {}",
            cpu_bar,
            latest.cpu_percent,
            mem_bar,
            format_bytes(latest.memory_used),
            format_bytes(latest.memory_total)
        )
    }
}

fn create_bar(value: usize, max: usize, width: usize) -> String {
    let filled = (value * width) / max;
    "█".repeat(filled) + "░".repeat(width - filled)
}
```

**Event Streaming:**
```rust
pub struct EventStreamer {
    reader: BufReader<File>,
    filters: Vec<EventFilter>,
}

impl EventStreamer {
    pub async fn stream(&mut self) -> Result<()> {
        println!("╭─ Following Events: {} ────────────────────────────╮", job_id);
        println!("│ Streaming live events (press Ctrl+C to stop)        │");
        println!("╰──────────────────────────────────────────────────────╯");
        println!();

        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF, wait and retry
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Ok(_) => {
                    let event: StreamedEvent = serde_json::from_str(&line)?;
                    if self.should_display(&event) {
                        self.display_event(&event);
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn display_event(&self, event: &StreamedEvent) {
        let timestamp = event.timestamp.format("%H:%M:%S%.3f");
        let colored_type = self.color_event_type(&event.event_type);

        println!(
            "{}  {}    {}",
            style(timestamp).dim(),
            colored_type,
            event.message
        );

        if let Some(details) = &event.details {
            println!("                                  {}",
                style(format!("→ {}", details)).dim());
        }
    }

    fn color_event_type(&self, event_type: &EventType) -> String {
        use console::style;
        match event_type {
            EventType::AgentStarted | EventType::ItemSuccess => {
                style(format!("{:14}", format!("{:?}", event_type))).green().to_string()
            }
            EventType::ItemFailed | EventType::AgentError => {
                style(format!("{:14}", format!("{:?}", event_type))).red().to_string()
            }
            EventType::Checkpoint => {
                style(format!("{:14}", format!("{:?}", event_type))).yellow().to_string()
            }
            _ => {
                style(format!("{:14}", format!("{:?}", event_type))).cyan().to_string()
            }
        }
    }
}
```

### Progress Update Strategy

**Polling vs Push:**
- Use polling for progress updates (every 200-500ms)
- Use push for critical events (completion, failure)
- Batch updates to prevent UI thrashing

**Update Frequency:**
- Overall progress: 500ms
- Agent status: 200ms
- Resource usage: 1s
- Recent log: On change (debounced 100ms)

**Concurrency:**
```rust
// Progress updater task
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    loop {
        interval.tick().await;
        let progress = tracker.get_progress().await;
        dashboard.update(&progress);
    }
});
```

## Dependencies

- **Prerequisites**: [110 - Terminal UI Foundation]
- **Affected Components**:
  - `src/cook/execution/mapreduce/` - Add progress tracking
  - `src/cook/workflow/executor.rs` - Integrate progress display
  - `src/cli/commands/events.rs` - Implement follow command
- **External Dependencies**:
  - Inherits from Spec 110: console, indicatif, comfy-table
  - `sysinfo = "0.37"` (already in Cargo.toml) for resource monitoring

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_progress_calculation() {
    // Test percentage and ETA calculations
}

#[test]
fn test_resource_bar_display() {
    // Test bar chart generation
}

#[test]
fn test_event_filtering() {
    // Test event filter logic
}

#[test]
fn test_throughput_calculation() {
    // Test items/sec calculation
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_mapreduce_dashboard() {
    // Run workflow with progress tracking
    // Verify all metrics updated
}

#[tokio::test]
async fn test_event_streaming() {
    // Start event stream
    // Generate events
    // Verify display
}
```

### Performance Tests

- Verify UI updates don't impact workflow performance
- Test with 100+ parallel agents
- Measure update latency < 50ms
- Check memory usage stays bounded

### Manual Testing

- Run MapReduce workflow and observe dashboard
- Test keyboard controls during execution
- Verify resource monitoring accuracy
- Test event streaming with filters
- Check error display formatting
- Verify on narrow terminals (80 cols)

## Documentation Requirements

### Code Documentation

- Document progress tracking architecture
- Add examples for creating dashboards
- Document event streaming API

### User Documentation

- Update CLAUDE.md with progress display info
- Document keyboard controls
- Add troubleshooting for display issues

### Architecture Documentation

- Document progress tracking data flow
- Explain update strategy and concurrency
- Document resource monitoring approach

## Implementation Notes

### Throughput Calculation

```rust
fn calculate_throughput(
    completed: usize,
    start_time: Instant,
    window: Duration
) -> f64 {
    let elapsed = start_time.elapsed();
    if elapsed < window {
        return 0.0;
    }
    completed as f64 / elapsed.as_secs_f64()
}
```

### ETA Calculation

```rust
fn calculate_eta(
    completed: usize,
    total: usize,
    throughput: f64
) -> Option<Duration> {
    if throughput == 0.0 {
        return None;
    }
    let remaining = total - completed;
    let seconds = remaining as f64 / throughput;
    Some(Duration::from_secs_f64(seconds))
}
```

### Terminal Size Handling

```rust
use console::Term;

let term = Term::stdout();
let (width, height) = term.size();

// Adjust display based on terminal size
let max_visible_agents = if width < 100 {
    5
} else if width < 120 {
    10
} else {
    15
};
```

### Non-Interactive Fallback

When not interactive:
- No live progress bars
- Periodic text updates every 10s
- Simple percentage progress
- Final summary only

## Migration and Compatibility

### Breaking Changes

None - Enhanced display is additive.

### Migration Path

1. Add progress tracking alongside existing output
2. Make live display optional via flag (--progress=live|simple|none)
3. Default to live in interactive mode, simple in non-interactive

### Backward Compatibility

- Simple text progress remains available
- JSON output mode unaffected
- Exit codes unchanged
- Log files unaffected

## Success Metrics

- Live dashboard provides clear visibility into execution
- Users can identify bottlenecks and issues in real-time
- Resource monitoring helps optimize parallel execution
- Event streaming aids debugging
- Error messages provide clear resolution paths
- UI updates smoothly without flickering
- Performance impact < 5% overhead
