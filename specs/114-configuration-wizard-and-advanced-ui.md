---
number: 114
title: Configuration Wizard and Advanced UI Features
category: foundation
priority: medium
status: draft
dependencies: [110]
created: 2025-10-01
---

# Specification 114: Configuration Wizard and Advanced UI Features

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: [110 - Terminal UI Foundation]

## Context

Prodigy's `init` command currently creates basic directory structure without guiding users through configuration. There's no interactive setup wizard, and advanced UI features like full TUI dashboards are not implemented. This limits:
- New user onboarding experience
- Configuration discoverability
- Real-time monitoring capabilities
- Advanced workflow visualization

This specification addresses configuration setup and future advanced UI features.

## Objective

Implement an interactive configuration wizard for `prodigy init` and define the architecture for future advanced UI features including an optional full-screen TUI dashboard for real-time monitoring.

## Requirements

### Functional Requirements

**FR1**: Interactive configuration wizard (`prodigy init`)
- Welcome screen with project overview
- Step-by-step configuration prompts
- Input validation and helpful defaults
- Configuration preview before saving
- Option to start over or modify settings
- Installation of example workflows (optional)
- Success message with next steps

**FR2**: Configuration options
- Project name (auto-detected from directory)
- Storage location (global ~/.prodigy vs local .prodigy)
- Maximum parallel agents (with recommendations)
- Default retry strategy (exponential, fixed, immediate)
- Event logging enabled/disabled
- Event retention policy (7d, 30d, 90d, forever, custom)
- Automatic worktree cleanup (prompt, always, never)
- Example workflow installation (yes/no)

**FR3**: Configuration validation
- Validate parallel agent count (1-100)
- Validate retention policies
- Check disk space for global storage
- Verify write permissions
- Validate custom durations

**FR4**: Configuration summary display
- Boxed summary of all settings
- Highlight important settings
- Show estimated disk usage
- Display created paths
- List installed components

**FR5**: Post-initialization guidance
- Success confirmation message
- List of created directories/files
- Example commands to try next
- Link to documentation
- Quick start guide reference

**FR6**: Advanced UI architecture (future)
- Define architecture for full-screen TUI dashboard
- Use `ratatui` for complex layouts
- Support multiple views (overview, agents, events, DLQ)
- Keyboard navigation between views
- Real-time data refresh

### Non-Functional Requirements

**NFR1**: Usability - Wizard completes in < 2 minutes
**NFR2**: Safety - Invalid configurations prevented
**NFR3**: Flexibility - All settings overridable via flags
**NFR4**: Extensibility - Easy to add new configuration options

## Acceptance Criteria

- [ ] `prodigy init` launches interactive wizard
- [ ] Welcome screen displays project information
- [ ] All configuration prompts have sensible defaults
- [ ] Input validation prevents invalid values
- [ ] Parallel agent count recommendations based on system resources
- [ ] Storage location shows disk space availability
- [ ] Retention policy validation works correctly
- [ ] Configuration preview displays all settings
- [ ] User can modify settings before finalizing
- [ ] Configuration file created at correct location
- [ ] Directory structure created properly
- [ ] Example workflows installed if requested
- [ ] Success message shows next steps
- [ ] Example commands are accurate and helpful
- [ ] Non-interactive mode works with flags
- [ ] TUI dashboard architecture documented
- [ ] All prompts skip gracefully in non-interactive mode

## Technical Details

### Implementation Approach

**Phase 1: Configuration Wizard**
1. Create wizard flow with dialoguer prompts
2. Implement input validation
3. Add system capability detection
4. Create configuration preview display
5. Implement configuration file writing

**Phase 2: Post-Init Features**
1. Create success display with formatted output
2. Generate helpful next steps
3. Add example workflow installation
4. Create quick start guide

**Phase 3: Advanced UI Architecture (Future)**
1. Design TUI dashboard layout
2. Create ratatui-based prototype
3. Implement view navigation
4. Add data refresh mechanisms
5. Document architecture for future implementation

### Module Structure

```rust
src/init/
├── mod.rs              // Public API
├── wizard.rs           // Interactive wizard flow
├── config_builder.rs   // Configuration construction
├── validator.rs        // Input validation
├── installer.rs        // Example workflow installation
└── display.rs          // Wizard display utilities

src/tui/ (future)
├── mod.rs              // TUI application
├── dashboard.rs        // Main dashboard view
├── agent_view.rs       // Agent monitoring view
├── event_view.rs       // Event log view
├── dlq_view.rs         // DLQ management view
└── navigation.rs       // Keyboard navigation
```

### Key Data Structures

```rust
// Configuration builder
pub struct ProdigyConfigBuilder {
    pub project_name: Option<String>,
    pub storage_location: StorageLocation,
    pub max_parallel: usize,
    pub retry_strategy: RetryStrategy,
    pub event_logging: bool,
    pub event_retention: RetentionPolicy,
    pub auto_cleanup: AutoCleanupPolicy,
    pub install_examples: bool,
}

pub enum StorageLocation {
    Global,  // ~/.prodigy
    Local,   // .prodigy
}

pub enum RetentionPolicy {
    Days(u32),
    Forever,
}

pub enum AutoCleanupPolicy {
    Prompt,
    Always,
    Never,
}

// System recommendations
pub struct SystemRecommendations {
    pub recommended_parallel: usize,
    pub available_memory: u64,
    pub available_disk: u64,
    pub cpu_count: usize,
}
```

### Wizard Flow Implementation

```rust
use dialoguer::{Input, Select, Confirm};
use console::style;

pub async fn run_wizard() -> Result<ProdigyConfig> {
    // Welcome screen
    display_welcome();

    // Project name
    let project_name = Input::<String>::new()
        .with_prompt("Project name")
        .default(detect_project_name()?)
        .interact()?;

    // Storage location
    let storage_items = vec![
        "Global (~/.prodigy) - Recommended",
        "Local (.prodigy)    - Project-specific",
    ];
    let storage_choice = Select::new()
        .with_prompt("Storage location")
        .items(&storage_items)
        .default(0)
        .interact()?;

    // Max parallel agents with recommendations
    let recommendations = detect_system_capabilities()?;
    let max_parallel = Input::<usize>::new()
        .with_prompt("Maximum parallel agents for MapReduce")
        .default(recommendations.recommended_parallel)
        .validate_with(|input: &usize| {
            if *input >= 1 && *input <= 100 {
                Ok(())
            } else {
                Err("Must be between 1 and 100")
            }
        })
        .interact()?;

    // Retry strategy
    let retry_items = vec![
        "Exponential backoff (1s, 2s, 4s...)",
        "Fixed delay (5s)",
        "Immediate retry",
    ];
    let retry_choice = Select::new()
        .with_prompt("Default retry strategy for failed items")
        .items(&retry_items)
        .default(0)
        .interact()?;

    // Event logging
    let event_logging = Confirm::new()
        .with_prompt("Enable event logging")
        .default(true)
        .interact()?;

    // Event retention
    let retention_items = vec![
        "30 days",
        "7 days",
        "90 days",
        "Forever",
        "Custom",
    ];
    let retention_choice = Select::new()
        .with_prompt("Event retention policy")
        .items(&retention_items)
        .default(0)
        .interact()?;

    // Auto cleanup
    let cleanup_items = vec![
        "Prompt each time",
        "Always clean",
        "Never clean",
    ];
    let cleanup_choice = Select::new()
        .with_prompt("Automatically clean merged worktrees")
        .items(&cleanup_items)
        .default(0)
        .interact()?;

    // Example workflows
    let install_examples = Confirm::new()
        .with_prompt("Install example workflows")
        .default(true)
        .interact()?;

    // Build config
    let config = ProdigyConfigBuilder {
        project_name: Some(project_name),
        storage_location: if storage_choice == 0 {
            StorageLocation::Global
        } else {
            StorageLocation::Local
        },
        max_parallel,
        retry_strategy: match retry_choice {
            0 => RetryStrategy::ExponentialBackoff,
            1 => RetryStrategy::FixedDelay,
            _ => RetryStrategy::Immediate,
        },
        event_logging,
        event_retention: parse_retention_choice(retention_choice)?,
        auto_cleanup: match cleanup_choice {
            0 => AutoCleanupPolicy::Prompt,
            1 => AutoCleanupPolicy::Always,
            _ => AutoCleanupPolicy::Never,
        },
        install_examples,
    };

    // Preview
    display_config_preview(&config)?;

    // Confirm
    let confirmed = Select::new()
        .with_prompt("Confirm configuration")
        .items(&["Yes, create configuration", "No, start over", "Advanced settings"])
        .default(0)
        .interact()?;

    if confirmed == 0 {
        Ok(config.build()?)
    } else if confirmed == 1 {
        run_wizard().await // Start over
    } else {
        run_advanced_wizard(config).await
    }
}
```

### System Capability Detection

```rust
use sysinfo::{System, SystemExt};

fn detect_system_capabilities() -> Result<SystemRecommendations> {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu_count = system.cpus().len();
    let available_memory = system.available_memory();
    let available_disk = get_available_disk_space()?;

    // Recommend parallel agents based on CPU and memory
    let recommended_parallel = std::cmp::min(
        cpu_count,
        (available_memory / (512 * 1024 * 1024)) as usize // 512MB per agent
    ).clamp(1, 10);

    Ok(SystemRecommendations {
        recommended_parallel,
        available_memory,
        available_disk,
        cpu_count,
    })
}
```

### Configuration Preview Display

```rust
fn display_config_preview(config: &ProdigyConfigBuilder) -> Result<()> {
    println!();
    println!("╭─ Configuration Summary ───────────────────────────────────────────────╮");
    println!("│                                                                        │");
    println!("│  Project:              {:<50}│", config.project_name.as_ref().unwrap());
    println!("│  Storage:              {:<50}│", format_storage(&config.storage_location));
    println!("│  Max parallel:         {} agents{:<40}│", config.max_parallel, "");
    println!("│  Retry strategy:       {:<50}│", format_retry(&config.retry_strategy));
    println!("│  Event logging:        {:<50}│", if config.event_logging { "Enabled (30 day retention)" } else { "Disabled" });
    println!("│  Auto cleanup:         {:<50}│", format_cleanup(&config.auto_cleanup));
    println!("│  Example workflows:    {:<50}│", if config.install_examples { "Yes" } else { "No" });
    println!("│                                                                        │");
    println!("╰────────────────────────────────────────────────────────────────────────╯");
    println!();

    Ok(())
}
```

### Advanced TUI Dashboard Architecture (Future)

```rust
// Future implementation using ratatui

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Terminal,
};

pub struct ProdigyDashboard {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    current_view: ViewType,
    state: DashboardState,
}

pub enum ViewType {
    Overview,    // Main dashboard
    Agents,      // Detailed agent view
    Events,      // Event log viewer
    Dlq,         // DLQ management
}

pub struct DashboardState {
    pub workflow_progress: WorkflowProgress,
    pub agents: Vec<AgentTracker>,
    pub recent_events: Vec<StreamedEvent>,
    pub dlq_count: usize,
    pub resource_usage: ResourceUsage,
}

impl ProdigyDashboard {
    pub async fn run(&mut self) -> Result<()> {
        // Main event loop
        loop {
            self.draw()?;
            if self.handle_events().await? {
                break; // Exit requested
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        self.terminal.draw(|f| {
            // Layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),    // Header
                    Constraint::Min(10),      // Main content
                    Constraint::Length(3),    // Footer
                ])
                .split(f.size());

            // Render based on current view
            match self.current_view {
                ViewType::Overview => self.render_overview(f, chunks[1]),
                ViewType::Agents => self.render_agents(f, chunks[1]),
                ViewType::Events => self.render_events(f, chunks[1]),
                ViewType::Dlq => self.render_dlq(f, chunks[1]),
            }
        })?;

        Ok(())
    }
}
```

## Dependencies

- **Prerequisites**: [110 - Terminal UI Foundation]
- **Affected Components**:
  - `src/init/mod.rs` - Complete rewrite
  - Configuration system
  - Example workflows
- **External Dependencies**:
  - Inherits from Spec 110: console, dialoguer
  - Future: `ratatui = "0.27"` for TUI dashboard

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_system_capability_detection() {
    // Test parallel agent recommendations
}

#[test]
fn test_input_validation() {
    // Test various invalid inputs
}

#[test]
fn test_config_building() {
    // Test configuration object construction
}

#[test]
fn test_retention_policy_parsing() {
    // Test duration parsing
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_init_wizard_non_interactive() {
    // Test with all flags provided
    // Verify config created correctly
}

#[tokio::test]
async fn test_example_workflow_installation() {
    // Test workflow files created
    // Verify content correct
}
```

### Manual Testing

- Run wizard with various inputs
- Test validation errors
- Verify system detection accuracy
- Test on different platforms
- Verify example workflows work
- Test non-interactive mode

## Documentation Requirements

### Code Documentation

- Document wizard flow
- Add examples for configuration building
- Document validation rules

### User Documentation

- Create quick start guide
- Document all configuration options
- Add troubleshooting section

### Architecture Documentation

- Document TUI dashboard architecture
- Explain view navigation system
- Document data flow for real-time updates

## Implementation Notes

### Project Name Detection

```rust
fn detect_project_name() -> Result<String> {
    let current_dir = std::env::current_dir()?;
    current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Could not detect project name"))
}
```

### Disk Space Checking

```rust
use sysinfo::{DiskExt, System, SystemExt};

fn get_available_disk_space() -> Result<u64> {
    let system = System::new_all();
    let disks = system.disks();

    // Find disk for home directory
    let home = std::env::var("HOME")?;
    for disk in disks {
        if home.starts_with(disk.mount_point().to_str().unwrap_or("")) {
            return Ok(disk.available_space());
        }
    }

    Ok(0)
}
```

### Example Workflow Installation

```rust
fn install_example_workflows(base_path: &Path) -> Result<()> {
    let workflows = vec![
        ("debtmap.yml", include_str!("../../examples/debtmap.yml")),
        ("ci.yml", include_str!("../../examples/ci.yml")),
        ("test.yml", include_str!("../../examples/test.yml")),
    ];

    let workflow_dir = base_path.join(".claude/workflows");
    std::fs::create_dir_all(&workflow_dir)?;

    for (name, content) in workflows {
        let path = workflow_dir.join(name);
        std::fs::write(path, content)?;
    }

    Ok(())
}
```

### Non-Interactive Mode

Support all wizard options as CLI flags:
```bash
prodigy init \
  --project-name myproject \
  --storage global \
  --max-parallel 10 \
  --retry-strategy exponential \
  --event-logging \
  --retention 30d \
  --auto-cleanup prompt \
  --install-examples
```

## Migration and Compatibility

### Breaking Changes

None - Current `init` behavior is basic and will be enhanced.

### Migration Path

1. Add interactive wizard as default behavior
2. Support non-interactive mode with flags
3. Detect if run in CI and auto-enable non-interactive mode

### Backward Compatibility

- Basic directory creation still works
- Existing config files not affected
- Manual configuration still possible

## Success Metrics

- New users complete wizard successfully
- Configuration settings are discoverable
- Wizard completes in < 2 minutes
- System recommendations are accurate
- Example workflows help users get started
- TUI dashboard architecture is clear and implementable
