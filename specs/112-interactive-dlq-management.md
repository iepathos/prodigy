---
number: 112
title: Interactive DLQ Management
category: foundation
priority: high
status: draft
dependencies: [110]
created: 2025-10-01
---

# Specification 112: Interactive DLQ Management

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [110 - Terminal UI Foundation]

## Context

The Dead Letter Queue (DLQ) commands are currently stub implementations that only print placeholder messages. Users have no way to:
- View failed items in a formatted, readable way
- Analyze failure patterns and common errors
- Interactively select items for retry
- Monitor retry progress in real-time
- Understand failure context and next steps

This specification implements rich, interactive DLQ management commands with formatted tables, failure analysis, interactive retry selection, and live progress displays.

## Objective

Implement comprehensive DLQ management commands with formatted tables, failure pattern analysis, interactive retry selection, and real-time progress monitoring using the terminal UI foundation.

## Requirements

### Functional Requirements

**FR1**: Enhanced `prodigy dlq list` output
- Display failed items in formatted table with borders
- Show columns: #, Item, Error, Retries, Failed At
- Visual indicator for exhausted retries (⚠)
- Relative time display for failure timestamps
- Summary section with statistics
- Group by error type option (--group-by error)
- Filter by eligibility (--eligible flag)
- Limit display (--limit N)

**FR2**: Failure pattern analysis (`prodigy dlq analyze`)
- Bar chart of errors by type with percentages
- Most common error identification
- Recommendations based on error patterns
- Export analysis to file (--export PATH)
- Summary statistics (total failures, unique errors, etc.)
- Temporal analysis (failures over time)

**FR3**: Interactive retry selection (`prodigy dlq retry`)
- Multi-select interface for choosing items to retry
- Show item name, error type, retry count for each
- Display eligibility status (can retry / max retries reached)
- Configuration options:
  - Maximum parallel retries (with recommendations)
  - Retry strategy (exponential backoff, fixed delay, immediate)
- Preview of retry plan before execution
- Confirmation with summary

**FR4**: Live retry progress display
- Overall progress bar with percentage and ETA
- Individual agent progress bars
- Real-time status updates (Running, Complete, Failed)
- Statistics panel (Success, Running, Pending, Failed counts)
- Throughput and timing metrics
- Scrolling log of recent completions
- Keyboard controls (q=quit gracefully, p=pause, l=logs)

**FR5**: Retry completion summary
- Success/failure counts with percentages
- Duration per item and total duration
- List of successful items with timings
- List of still-failing items with new error info
- Remaining DLQ count
- Suggested next actions

**FR6**: DLQ item inspection (`prodigy dlq inspect`)
- Detailed view of single failed item
- Show full error message and stack trace
- Display retry history with timestamps
- Show original work item data
- Display correlation ID for event tracking
- Show related events from event log

**FR7**: DLQ statistics (`prodigy dlq stats`)
- Overall DLQ health metrics
- Success rate over time
- Average retry count
- Most problematic items (highest retry count)
- Error distribution chart
- Time-based trends

### Non-Functional Requirements

**NFR1**: Performance - Table rendering < 200ms for 1000 items
**NFR2**: Responsiveness - Live progress updates at 2-10 Hz
**NFR3**: Usability - Clear, actionable error messages and recommendations
**NFR4**: Safety - Confirmation required for bulk operations

## Acceptance Criteria

- [ ] `prodigy dlq list` displays formatted table with all columns
- [ ] Summary statistics show correct counts
- [ ] `--group-by error` displays bar chart with percentages
- [ ] Relative timestamps display correctly
- [ ] `prodigy dlq analyze` identifies error patterns
- [ ] Analysis provides actionable recommendations
- [ ] Interactive retry selection shows all eligible items
- [ ] Multi-select works with keyboard navigation
- [ ] Retry configuration options presented clearly
- [ ] Live progress display shows all metrics
- [ ] Progress bars update smoothly in real-time
- [ ] Agent status transitions displayed correctly
- [ ] Keyboard controls (q, p, l) work during retry
- [ ] Completion summary shows accurate statistics
- [ ] Still-failing items shown with updated error info
- [ ] Next action suggestions are helpful
- [ ] `prodigy dlq inspect` shows all item details
- [ ] Full error messages and stack traces displayed
- [ ] Retry history chronologically ordered
- [ ] Correlation IDs link to event logs
- [ ] `prodigy dlq stats` displays health metrics
- [ ] Charts and graphs render correctly
- [ ] All commands gracefully fall back in non-interactive mode

## Technical Details

### Implementation Approach

**Phase 1: Enhanced List and Analysis**
1. Implement DLQ list command with table formatting
2. Add grouping and filtering options
3. Create failure pattern analysis
4. Implement bar charts and statistics

**Phase 2: Interactive Retry**
1. Build multi-select interface for item selection
2. Implement retry configuration options
3. Create preview and confirmation screens
4. Integrate with existing DLQ retry infrastructure

**Phase 3: Live Progress**
1. Implement multi-progress display using indicatif
2. Create agent status tracking
3. Add real-time updates via polling or events
4. Implement keyboard controls

**Phase 4: Inspection and Stats**
1. Implement detailed item inspection
2. Create statistics dashboard
3. Add trend analysis
4. Implement export functionality

### Module Structure

```rust
src/cli/commands/dlq/
├── mod.rs              // Command routing
├── list.rs            // List command with formatting
├── analyze.rs         // Failure pattern analysis
├── retry.rs           // Interactive retry
├── inspect.rs         // Item inspection
├── stats.rs           // Statistics dashboard
└── display.rs         // Shared display utilities

src/cook/execution/dlq/
├── analyzer.rs        // Pattern analysis logic
├── retry_config.rs    // Retry configuration
└── progress.rs        // Progress tracking for retry
```

### Key Data Structures

```rust
// DLQ item display info
pub struct DlqItemDisplay {
    pub item_number: usize,
    pub item_id: String,
    pub error_type: String,
    pub error_message: String,
    pub retry_count: usize,
    pub max_retries: usize,
    pub failed_at: DateTime<Utc>,
    pub eligible_for_retry: bool,
}

// Failure pattern analysis
pub struct FailureAnalysis {
    pub total_items: usize,
    pub unique_errors: usize,
    pub error_distribution: Vec<ErrorTypeCount>,
    pub most_common_error: String,
    pub recommendations: Vec<String>,
}

pub struct ErrorTypeCount {
    pub error_type: String,
    pub count: usize,
    pub percentage: f64,
}

// Retry configuration
pub struct RetryConfig {
    pub max_parallel: usize,
    pub strategy: RetryStrategy,
    pub selected_items: Vec<String>,
}

pub enum RetryStrategy {
    ExponentialBackoff { base_delay: Duration },
    FixedDelay { delay: Duration },
    Immediate,
}

// Retry progress tracking
pub struct RetryProgress {
    pub total_items: usize,
    pub completed: usize,
    pub failed: usize,
    pub running: usize,
    pub agents: Vec<AgentProgress>,
    pub recent_completions: VecDeque<CompletionInfo>,
}
```

### Display Formats

**DLQ List Table:**
```rust
use comfy_table::presets::UTF8_FULL;

let mut table = Table::new();
table.load_preset(UTF8_FULL);
table.set_header(vec!["#", "Item", "Error", "Retries", "Failed At"]);

// Add rows with color coding
for item in items {
    let retry_display = if item.retry_count >= item.max_retries {
        format!("{}/{} ⚠", item.retry_count, item.max_retries)
            .red()
    } else {
        format!("{}/{}", item.retry_count, item.max_retries)
    };
    table.add_row(vec![
        item.item_number.to_string(),
        item.item_id,
        item.error_type,
        retry_display,
        format_relative_time(item.failed_at),
    ]);
}
```

**Failure Analysis Bar Chart:**
```rust
// ASCII bar chart
for error in analysis.error_distribution {
    let bar_width = (error.percentage * 50.0) as usize;
    let bar = "█".repeat(bar_width) + "░".repeat(50 - bar_width);
    println!(
        "  {:<30} {:>3} items    {} {:>3}%",
        error.error_type,
        error.count,
        bar,
        error.percentage as u32
    );
}
```

**Live Retry Progress:**
```rust
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

let multi = MultiProgress::new();

// Overall progress
let overall = multi.add(ProgressBar::new(total as u64));
overall.set_style(
    ProgressStyle::default_bar()
        .template("  Overall    [{bar:40}] {pos}/{len}  {percent}%    ⏱ {elapsed}  ETA: {eta}")
        .progress_chars("█▓░")
);

// Agent progress bars
for agent in agents {
    let agent_bar = multi.add(ProgressBar::new_spinner());
    agent_bar.set_style(
        ProgressStyle::default_spinner()
            .template("  {prefix} {spinner} {msg}")
    );
    agent_bar.set_prefix(format!("Agent-{}", agent.id));
}
```

### Integration with DLQ System

**Reading DLQ Files:**
```rust
// Load DLQ file for job
let dlq_path = storage.dlq_path(job_id)?;
let items: Vec<DlqItem> = serde_json::from_reader(File::open(dlq_path)?)?;
```

**Retry Execution:**
- Reuse existing `DlqReprocessor` infrastructure
- Add progress callback for live updates
- Stream progress events to display

**Analysis Integration:**
- Parse error types from DLQ items
- Group and count by error type
- Calculate statistics and percentages
- Generate recommendations based on patterns

## Dependencies

- **Prerequisites**: [110 - Terminal UI Foundation]
- **Affected Components**:
  - `src/cli/commands/dlq.rs` - Complete implementation
  - `src/cook/execution/dlq_reprocessor.rs` - Add progress callbacks
  - DLQ storage system
- **External Dependencies**:
  - Inherits from Spec 110: console, dialoguer, comfy-table, indicatif

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_dlq_list_formatting() {
    // Test table generation with various items
}

#[test]
fn test_failure_analysis() {
    // Test pattern detection and statistics
}

#[test]
fn test_retry_strategy_selection() {
    // Test interactive selection flow
}

#[test]
fn test_progress_tracking() {
    // Test progress calculation and display
}

#[test]
fn test_recommendation_generation() {
    // Test that recommendations make sense for error types
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_dlq_list_with_items() {
    // Create DLQ with items
    // Run list command
    // Verify table output
}

#[tokio::test]
async fn test_interactive_retry() {
    // Create DLQ items
    // Run interactive retry
    // Verify selection and execution
}

#[tokio::test]
async fn test_live_progress_display() {
    // Start retry operation
    // Monitor progress updates
    // Verify completion summary
}
```

### Manual Testing

- Test with small DLQ (< 10 items)
- Test with large DLQ (1000+ items)
- Test grouping with many error types
- Verify bar charts render correctly
- Test interactive retry selection
- Monitor live progress smoothness
- Test keyboard controls during retry

## Documentation Requirements

### Code Documentation

- Document all display utilities
- Add examples for creating charts and tables
- Document retry configuration options

### User Documentation

- Update CLAUDE.md with DLQ command examples
- Add troubleshooting guide for common errors
- Document retry strategies and when to use each

### Architecture Documentation

- Document DLQ analysis algorithms
- Explain progress tracking architecture
- Document integration with event system

## Implementation Notes

### Error Type Classification

Extract error type from error messages:
```rust
fn classify_error(error_message: &str) -> String {
    if error_message.contains("timeout") {
        "TestTimeout".to_string()
    } else if error_message.contains("compilation") {
        "CompilationError".to_string()
    } else if error_message.contains("memory") {
        "OutOfMemory".to_string()
    } else if error_message.contains("api") || error_message.contains("529") {
        "ClaudeApiError".to_string()
    } else {
        "Unknown".to_string()
    }
}
```

### Recommendation Engine

Generate recommendations based on error patterns:
```rust
fn generate_recommendations(analysis: &FailureAnalysis) -> Vec<String> {
    let mut recommendations = Vec::new();

    for error_type in &analysis.error_distribution {
        match error_type.error_type.as_str() {
            "CompilationError" => {
                recommendations.push(
                    "Review compilation environment and dependencies".to_string()
                );
            }
            "TestTimeout" => {
                recommendations.push(
                    "Consider increasing test timeout or optimizing slow tests".to_string()
                );
            }
            "OutOfMemory" => {
                recommendations.push(
                    "Increase memory limits or optimize memory-intensive operations".to_string()
                );
            }
            "ClaudeApiError" => {
                recommendations.push(
                    "Check API rate limits and retry with exponential backoff".to_string()
                );
            }
            _ => {}
        }
    }

    recommendations
}
```

### Progress Update Frequency

Balance between smoothness and performance:
- Overall progress: Update every 500ms
- Agent status: Update every 200ms
- Throughput: Calculate every 1s
- Recent completions: Show last 5 items

### Non-Interactive Fallback

When not interactive:
- `dlq list` - Output as JSON or simple table
- `dlq retry` - Require explicit item IDs or --all flag
- `dlq analyze` - Output text analysis without charts
- Progress - Show periodic text updates instead of live bars

## Migration and Compatibility

### Breaking Changes

None - DLQ commands are currently stubs, so any implementation is new functionality.

### Migration Path

1. Implement new DLQ commands with full functionality
2. Add `--json` flags for machine-readable output
3. Ensure backward compatibility with existing DLQ storage format

### Backward Compatibility

- DLQ file format remains unchanged
- All existing DLQ data readable by new commands
- Non-interactive mode for CI/CD compatibility

## Success Metrics

- DLQ items displayed clearly with all relevant information
- Failure analysis identifies actionable patterns
- Interactive retry selection reduces errors
- Live progress display is smooth and informative
- Users can quickly identify and resolve DLQ issues
- Recommendations help users fix root causes
- All operations work in both interactive and non-interactive modes
