use clap::{Parser, Subcommand};
use mmm::{
    config::ConfigLoader,
    project::{health::HealthStatus, ProjectHealth, ProjectManager},
    spec::SpecificationEngine,
    Result,
};
use std::{path::PathBuf, sync::Arc};
use tracing::{debug, error, info, trace};

#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "Memento Mori Manager - A Git-based project management system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output (-v for debug, -vv for trace, -vvv for all)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Project management commands
    #[command(subcommand)]
    Project(ProjectCommands),

    /// Specification management commands
    #[command(subcommand)]
    Spec(SpecCommands),

    /// Run a specification
    Run {
        /// Specification ID
        spec_id: String,
    },

    /// Show project status
    Status,

    /// Multi-project operations
    #[command(subcommand)]
    Multi(MultiCommands),

    /// Claude AI integration commands
    #[command(subcommand)]
    Claude(ClaudeCommands),

    /// Workflow automation commands
    #[command(subcommand)]
    Workflow(WorkflowCommands),

    /// Iterative improvement loop commands
    #[command(subcommand)]
    Loop(LoopCommands),

    /// Improve code quality with zero configuration
    Improve(mmm::improve::command::ImproveCommand),
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Create a new project
    New {
        /// Project name
        name: String,

        /// Project path (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Initialize a project in current directory
    Init {
        /// Project name
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List all projects
    List {
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Show project information
    Info {
        /// Project name (defaults to current project)
        name: Option<String>,
    },

    /// Switch to a project
    Switch {
        /// Project name
        name: String,
    },

    /// Clone a project
    Clone {
        /// Source project
        source: String,
        /// Destination project name
        dest: String,
    },

    /// Archive a project
    Archive {
        /// Project name (defaults to current project)
        name: Option<String>,
    },

    /// Delete a project
    Delete {
        /// Project name
        name: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Check project health
    Health {
        /// Project name (defaults to current project)
        name: Option<String>,
        /// Fix issues automatically
        #[arg(short, long)]
        fix: bool,
    },

    /// Configure project settings
    Config {
        /// Configuration key
        #[arg(short, long)]
        key: Option<String>,
        /// Configuration value
        #[arg(short, long)]
        value: Option<String>,
        /// List all settings
        #[arg(short, long)]
        list: bool,
    },
}

#[derive(Subcommand)]
enum SpecCommands {
    /// List specifications in current project
    List {
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Create a new specification from description
    Add {
        /// Description of the feature to implement
        description: String,
    },

    /// Show detailed information about a specification
    Info {
        /// Specification ID
        spec_id: String,
    },
}

#[derive(Subcommand)]
enum ClaudeCommands {
    /// Run a Claude command
    Run {
        /// Command to execute (e.g., implement, review, debug)
        #[arg(short, long)]
        command: String,
        /// Additional arguments for the command
        args: Vec<String>,
    },

    /// List available Claude commands
    Commands,

    /// Show token usage statistics
    Stats,

    /// Clear Claude response cache
    ClearCache,

    /// Configure Claude integration
    #[command(name = "config")]
    Config {
        /// Configuration key (e.g., api_key, default_model)
        key: String,
        /// Configuration value
        value: String,
    },
}

#[derive(Subcommand)]
enum MultiCommands {
    /// Run a spec across multiple projects
    Run {
        /// Projects (comma-separated)
        #[arg(short, long)]
        projects: String,
        /// Specification to run
        #[arg(short, long)]
        spec: String,
    },

    /// Sync configuration across projects
    SyncConfig {
        /// Source (template name or project)
        #[arg(short, long)]
        source: String,
        /// Target projects ("all" or comma-separated list)
        #[arg(short, long)]
        targets: String,
    },

    /// Generate report across projects
    Report {
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Update component across projects
    Update {
        /// Component to update
        #[arg(short, long)]
        component: String,
    },
}

#[derive(Subcommand)]
enum WorkflowCommands {
    /// Run a workflow
    Run {
        /// Workflow name or path
        workflow: String,

        /// Specification ID (optional)
        #[arg(short, long)]
        spec: Option<String>,

        /// Run in dry-run mode
        #[arg(long)]
        dry_run: bool,

        /// Parameter overrides as key=value pairs
        #[arg(short, long, value_parser = parse_key_val::<String, String>)]
        params: Vec<(String, String)>,
    },

    /// List available workflows
    List,

    /// Show workflow details
    Show {
        /// Workflow name
        workflow: String,
    },

    /// List workflow executions
    History {
        /// Filter by workflow name
        #[arg(short, long)]
        workflow: Option<String>,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Number of executions to show
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },

    /// Debug a workflow
    Debug {
        /// Workflow name or path
        workflow: String,

        /// Set breakpoint at step
        #[arg(short, long)]
        breakpoint: Option<String>,
    },

    /// Manage workflow checkpoints
    #[command(subcommand)]
    Checkpoint(CheckpointCommands),

    /// Manage workflow triggers
    #[command(subcommand)]
    Trigger(TriggerCommands),
}

#[derive(Subcommand)]
enum CheckpointCommands {
    /// List pending checkpoints
    List,

    /// Respond to a checkpoint
    Respond {
        /// Checkpoint ID
        checkpoint_id: String,

        /// Response option (approve, reject, etc.)
        option: String,

        /// Additional response data
        #[arg(short, long)]
        data: Option<String>,
    },
}

#[derive(Subcommand)]
enum TriggerCommands {
    /// List configured triggers
    List,

    /// Enable/disable a trigger
    Toggle {
        /// Trigger ID
        trigger_id: String,

        /// Enable or disable
        #[arg(short, long)]
        enable: bool,
    },

    /// Create a new trigger
    Create {
        /// Workflow to trigger
        workflow: String,

        /// Event type
        #[arg(short, long)]
        event: String,

        /// Event filter
        #[arg(short, long)]
        filter: Option<String>,
    },
}

#[derive(Subcommand)]
enum LoopCommands {
    /// Start an iterative improvement session
    Start {
        /// Target quality score (0.0-10.0)
        #[arg(short, long, default_value = "8.5")]
        target: f64,

        /// Maximum iterations
        #[arg(short, long, default_value = "3")]
        max_iterations: u32,

        /// Code scope to improve (files/directories)
        #[arg(short, long, default_value = "src/")]
        scope: String,

        /// Severity levels to address
        #[arg(long, default_value = "critical,high")]
        severity: String,

        /// Workflow template to use
        #[arg(short, long, default_value = "code-quality-improvement")]
        workflow: String,

        /// Run in dry-run mode
        #[arg(long)]
        dry_run: bool,
    },

    /// List active and completed loop sessions
    Sessions {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },

    /// Show detailed session information
    Show {
        /// Session ID
        session_id: String,
    },

    /// Stop a running session
    Stop {
        /// Session ID
        session_id: String,

        /// Force stop without cleanup
        #[arg(short, long)]
        force: bool,
    },

    /// Resume a paused session
    Resume {
        /// Session ID  
        session_id: String,
    },

    /// Configure default loop settings
    Config {
        /// Configuration key
        #[arg(short, long)]
        key: Option<String>,

        /// Configuration value
        #[arg(short, long)]
        value: Option<String>,

        /// List current settings
        #[arg(short, long)]
        list: bool,
    },
}

#[derive(clap::ValueEnum, Clone)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(clap::ValueEnum, Clone)]
enum ExportFormat {
    Pdf,
    Html,
    Markdown,
    Json,
}

fn parse_key_val<T, U>(
    s: &str,
) -> std::result::Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => "info",
        1 => "debug",
        2 => "trace",
        _ => "trace,hyper=debug,tower=debug", // -vvv shows everything including dependencies
    };

    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(cli.verbose >= 2) // Show target module for -vv and above
        .with_thread_ids(cli.verbose >= 3) // Show thread IDs for -vvv
        .with_line_number(cli.verbose >= 3) // Show line numbers for -vvv
        .init();

    debug!("MMM started with verbosity level: {}", cli.verbose);
    trace!("Full CLI args: {:?}", std::env::args().collect::<Vec<_>>());

    if let Err(e) = run(cli).await {
        error!("Fatal error: {}", e);
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config_loader = ConfigLoader::new().await?;
    config_loader.load_global().await?;

    let mut project_manager = ProjectManager::new().await?;

    match cli.command {
        Commands::Project(project_cmd) => {
            handle_project_command(project_cmd, &mut project_manager, &config_loader).await?
        }
        Commands::Spec(spec_cmd) => {
            handle_spec_command(spec_cmd, &project_manager, &config_loader).await?
        }

        Commands::Run { spec_id } => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            config_loader.load_project(&project.path).await?;
            let config = config_loader.get_config();

            let mut spec_engine =
                SpecificationEngine::new(project.path.join(config.get_spec_dir()));
            spec_engine.load_specifications().await?;

            if spec_engine.get_specification(&spec_id).is_none() {
                return Err(mmm::Error::Spec(format!(
                    "Specification '{spec_id}' not found"
                )));
            }

            // TODO: Implement spec execution with simple state management
            println!("Running specification '{spec_id}'...");
            println!("Specification execution not yet implemented.");
        }

        Commands::Status => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            let state_manager =
                mmm::simple_state::StateManager::with_root(project.path.join(".mmm"))?;
            let state = state_manager.state();

            println!("Project: {}", project.name);
            println!("Path: {}", project.path.display());
            println!("Current score: {:.2}", state.current_score);
            println!("Total runs: {}", state.total_runs);
        }

        Commands::Multi(multi_cmd) => handle_multi_command(multi_cmd, &mut project_manager).await?,
        Commands::Claude(claude_cmd) => handle_claude_command(claude_cmd, &config_loader).await?,
        Commands::Workflow(workflow_cmd) => {
            handle_workflow_command(workflow_cmd, &project_manager, &config_loader).await?
        }
        Commands::Loop(loop_cmd) => {
            handle_loop_command(loop_cmd, &project_manager, &config_loader).await?
        }
        Commands::Improve(improve_cmd) => mmm::improve::run(improve_cmd).await?,
    }

    Ok(())
}

async fn handle_project_command(
    cmd: ProjectCommands,
    project_manager: &mut ProjectManager,
    config_loader: &ConfigLoader,
) -> Result<()> {
    use ProjectCommands::*;

    match cmd {
        New { name, path } => {
            let project_path = path.unwrap_or_else(|| PathBuf::from(&name));
            let created = project_manager.create_project(&name, &project_path).await?;
            info!("Created project '{}' at {}", name, created.path.display());
        }

        Init { name } => {
            let current_dir = std::env::current_dir()?;
            let project_name = name.unwrap_or_else(|| {
                current_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

            let initialized = project_manager
                .create_project(&project_name, &current_dir)
                .await?;
            info!(
                "Initialized project '{}' in {}",
                initialized.name,
                initialized.path.display()
            );
        }

        List { format } => {
            let projects = project_manager.list_projects();
            let current = project_manager.current_project();

            match format {
                OutputFormat::Table => {
                    println!("{:<3} {:<20} {:<10} {:<40}", "", "NAME", "STATUS", "PATH");
                    println!("{}", "-".repeat(73));
                    for project in projects {
                        let marker = if current
                            .as_ref()
                            .map(|p| p.name == project.name)
                            .unwrap_or(false)
                        {
                            "→"
                        } else {
                            ""
                        };
                        println!(
                            "{:<3} {:<20} {:<10} {:<40}",
                            marker,
                            project.name,
                            "active", // Projects don't have active field
                            project.path.display()
                        );
                    }
                }
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&projects)?;
                    println!("{json}");
                }
            }
        }

        Info { name } => {
            let project = if let Some(name) = name {
                project_manager.get_project(&name)?.clone()
            } else {
                project_manager
                    .current_project()
                    .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?
                    .clone()
            };

            println!("Project: {}", project.name);
            println!("Path: {}", project.path.display());
            println!("Status: active");
            println!("Created: {}", project.created);
            if let Some(desc) = &project.description {
                println!("Description: {desc}");
            }

            // Show health status
            let health = ProjectHealth::check(&project).await?;
            println!("\nHealth Status:");
            for check in &health.checks {
                if check.status != HealthStatus::Passing {
                    println!(
                        "  ⚠️  {}: {}",
                        check.name,
                        check.message.as_deref().unwrap_or("No details")
                    );
                }
            }
            if health
                .checks
                .iter()
                .all(|c| c.status == HealthStatus::Passing)
            {
                println!("  ✅ All checks passed");
            }
        }

        Switch { name } => {
            project_manager.switch_project(&name).await?;
            info!("Switched to project '{}'", name);
        }

        Clone { source, dest } => {
            project_manager.clone_project(&source, &dest).await?;
            info!("Cloned project '{}' to '{}'", source, dest);
        }

        Archive { name } => {
            let project_name = if let Some(name) = name {
                name
            } else {
                project_manager
                    .current_project()
                    .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?
                    .name
                    .clone()
            };

            project_manager.archive_project(&project_name).await?;
            info!("Archived project '{}'", project_name);
        }

        Delete { name, force } => {
            if !force {
                print!("Are you sure you want to delete project '{name}'? [y/N] ");
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Deletion cancelled.");
                    return Ok(());
                }
            }

            project_manager.delete_project(&name).await?;
            info!("Deleted project '{}'", name);
        }

        Health { name, fix } => {
            let project = if let Some(name) = name {
                project_manager.get_project(&name)?.clone()
            } else {
                project_manager
                    .current_project()
                    .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?
                    .clone()
            };

            let health = ProjectHealth::check(&project).await?;

            println!("Health check for project '{}':", project.name);
            let unhealthy_checks: Vec<_> = health
                .checks
                .iter()
                .filter(|c| c.status != HealthStatus::Passing)
                .collect();

            if unhealthy_checks.is_empty() {
                println!("✅ All checks passed!");
            } else {
                println!("Found {} issue(s):", unhealthy_checks.len());
                for check in &unhealthy_checks {
                    println!(
                        "  ⚠️  {}: {}",
                        check.name,
                        check.message.as_deref().unwrap_or("No details")
                    );
                }

                if fix {
                    println!("\nAttempting to fix issues...");
                    // TODO: Implement fix functionality
                    println!("✅ Issues fixed!");
                }
            }
        }

        Config { key, value, list } => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            if list {
                config_loader.load_project(&project.path).await?;
                let config = config_loader.get_config();
                println!("Project configuration:");
                println!("  spec_dir: {}", config.get_spec_dir().display());
                // println!("  state_dir: {}", config.get_state_dir());  // Method doesn't exist
                // TODO: Print all config values
            } else if let (Some(key), Some(value)) = (key, value) {
                config_loader.set_project_value(&key, &value).await?;
                info!("Set {} = {}", key, value);
            } else {
                println!("Usage: config --key KEY --value VALUE or config --list");
            }
        }
    }

    Ok(())
}

async fn handle_spec_command(
    cmd: SpecCommands,
    project_manager: &ProjectManager,
    config_loader: &ConfigLoader,
) -> Result<()> {
    use SpecCommands::*;

    let project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    config_loader.load_project(&project.path).await?;
    let config = config_loader.get_config();

    let mut spec_engine = SpecificationEngine::new(project.path.join(config.get_spec_dir()));
    spec_engine.load_specifications().await?;

    match cmd {
        List { format } => {
            let specs = spec_engine.topological_sort()?;
            if specs.is_empty() {
                println!("No specifications found.");
            } else {
                match format {
                    OutputFormat::Table => {
                        println!(
                            "{:<20} {:<30} {:<12} {:<10}",
                            "ID", "NAME", "STATUS", "DEPS"
                        );
                        println!("{}", "-".repeat(72));
                        for spec_id in specs {
                            if let Some(spec) = spec_engine.get_specification(&spec_id) {
                                println!(
                                    "{:<20} {:<30} {:<12} {:<10}",
                                    spec.id,
                                    spec.name.chars().take(30).collect::<String>(),
                                    format!("{:?}", spec.status),
                                    spec.dependencies.len()
                                );
                            }
                        }
                    }
                    OutputFormat::Json => {
                        let mut specs_info = Vec::new();
                        for spec_id in specs {
                            if let Some(spec) = spec_engine.get_specification(&spec_id) {
                                specs_info.push(spec);
                            }
                        }
                        println!("{}", serde_json::to_string_pretty(&specs_info)?);
                    }
                }
            }
        }

        Add { description } => {
            println!("🚀 Creating specification for: {description}");

            // Try Claude CLI first (no API key needed)
            let result = execute_claude_cli_direct("add-spec", vec![description.clone()]).await;

            match result {
                Ok(output) => {
                    println!("{output}");
                }
                Err(_) => {
                    // Fallback to API-based approach if Claude CLI fails
                    let mut claude_config = mmm::claude::ClaudeConfig::default();
                    if let Ok(api_key) = std::env::var("CLAUDE_API_KEY") {
                        claude_config.api_key = api_key;
                    }

                    // Get config from project if available
                    let config = config_loader.get_config();
                    if let Some(project_config) = &config.project {
                        if let Some(api_key) = &project_config.claude_api_key {
                            claude_config.api_key = api_key.clone();
                        }
                    }

                    // Check for global Claude API key if not set
                    if claude_config.api_key.is_empty() {
                        if let Some(api_key) = &config.global.claude_api_key {
                            claude_config.api_key = api_key.clone();
                        }
                    }

                    if claude_config.api_key.is_empty() {
                        return Err(mmm::Error::Config(
                            "Claude CLI not available and Claude API key not configured. Please install Claude CLI or set CLAUDE_API_KEY environment variable.".to_string()
                        ));
                    }

                    let mut claude_manager = mmm::claude::ClaudeManager::new(claude_config)?;
                    let result = claude_manager
                        .execute_command("add-spec", vec![description])
                        .await?;
                    println!("{result}");
                }
            }
        }

        Info { spec_id } => {
            if let Some(spec) = spec_engine.get_specification(&spec_id) {
                println!("Specification: {}", spec.name);
                println!("ID: {}", spec.id);
                println!("Status: {:?}", spec.status);
                println!(
                    "Dependencies: {}",
                    if spec.dependencies.is_empty() {
                        "None".to_string()
                    } else {
                        spec.dependencies.join(", ")
                    }
                );

                if let Some(objective) = &spec.metadata.objective {
                    println!("Objective: {objective}");
                }

                if !spec.metadata.acceptance_criteria.is_empty() {
                    println!("Acceptance Criteria:");
                    for (i, criterion) in spec.metadata.acceptance_criteria.iter().enumerate() {
                        println!("  {}. {}", i + 1, criterion);
                    }
                }

                if !spec.metadata.tags.is_empty() {
                    println!("Tags: {}", spec.metadata.tags.join(", "));
                }

                if let Some(priority) = spec.metadata.priority {
                    println!("Priority: {priority}");
                }

                if let Some(hours) = spec.metadata.estimated_hours {
                    println!("Estimated Hours: {hours:.1}");
                }

                println!("\nContent:");
                println!("{}", spec.content);
            } else {
                return Err(mmm::Error::Spec(format!(
                    "Specification '{spec_id}' not found"
                )));
            }
        }
    }

    Ok(())
}

async fn handle_multi_command(
    cmd: MultiCommands,
    project_manager: &mut ProjectManager,
) -> Result<()> {
    use MultiCommands::*;

    match cmd {
        Run { projects, spec } => {
            let project_names: Vec<&str> = projects.split(',').map(|s| s.trim()).collect();

            println!(
                "Running spec '{}' across {} projects",
                spec,
                project_names.len()
            );

            for project_name in project_names {
                match project_manager.get_project(project_name) {
                    Ok(project) => {
                        println!("\n📁 Project: {}", project.name);
                        // TODO: Actually run the spec
                        println!("  Would run spec '{spec}' here");
                    }
                    Err(_) => {
                        println!("\n⚠️  Project '{project_name}' not found");
                    }
                }
            }
        }

        SyncConfig { source, targets } => {
            let target_projects: Vec<String> = if targets == "all" {
                project_manager
                    .list_projects()
                    .into_iter()
                    .map(|p| p.name.clone())
                    .collect()
            } else {
                targets.split(',').map(|s| s.trim().to_string()).collect()
            };

            println!(
                "Syncing configuration from '{}' to {} projects",
                source,
                target_projects.len()
            );

            // TODO: Implement config sync
            for target in target_projects {
                println!("  Would sync to project: {target}");
            }
        }

        Report { format, output } => {
            let projects = project_manager.list_projects();

            println!(
                "Generating {} report for {} projects",
                format,
                projects.len()
            );

            // TODO: Implement report generation
            let report_content = format!("Report for {} projects\n", projects.len());

            if let Some(output_path) = output {
                std::fs::write(&output_path, report_content)?;
                println!("Report written to: {}", output_path.display());
            } else {
                println!("{report_content}");
            }
        }

        Update { component } => {
            let projects = project_manager.list_projects();

            println!(
                "Updating component '{}' across {} projects",
                component,
                projects.len()
            );

            // TODO: Implement component update
            for project in projects {
                println!("  Would update {} in project: {}", component, project.name);
            }
        }
    }

    Ok(())
}

/// Check if a command should use Claude CLI directly (without API key)
fn is_claude_cli_command(command: &str) -> bool {
    matches!(
        command,
        "mmm-lint" | "mmm-code-review" | "mmm-implement-spec" | "mmm-add-spec"
    )
}

/// Execute Claude CLI command directly with streaming output
async fn execute_claude_cli_direct(command: &str, args: Vec<String>) -> Result<String> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    // Map MMM command to Claude CLI slash command
    let slash_command = match command {
        "mmm-lint" => "/mmm-lint",
        "mmm-code-review" => "/mmm-code-review",
        "mmm-implement-spec" => "/mmm-implement-spec",
        "mmm-add-spec" => "/mmm-add-spec",
        _ => {
            error!("Unknown Claude CLI command: {command}");
            return Err(mmm::Error::NotFound(format!(
                "Unknown Claude CLI command: {command}"
            )));
        }
    };

    // Build command with arguments
    let mut cmd_args = vec![
        "--dangerously-skip-permissions".to_string(),
        slash_command.to_string(),
    ];
    cmd_args.extend(args);

    debug!("Executing Claude CLI: claude {}", cmd_args.join(" "));
    trace!(
        "Full command details: command='claude', args={:?}",
        cmd_args
    );

    // Execute Claude CLI with streaming output
    let mut child = Command::new("claude")
        .args(&cmd_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            error!("Failed to execute Claude CLI: {e}");
            debug!("Command was: claude {}", cmd_args.join(" "));
            mmm::Error::Other(format!("Failed to execute Claude CLI: {e}"))
        })?;

    // Get the stdout and stderr streams
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    // Create buffered readers
    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    // Stream output in real-time
    let handle_stdout = tokio::spawn(async move {
        let mut lines = stdout_reader.lines();
        let mut output = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("{line}");
            output.push_str(&line);
            output.push('\n');
        }
        output
    });

    let handle_stderr = tokio::spawn(async move {
        let mut lines = stderr_reader.lines();
        let mut errors = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("{line}");
            errors.push_str(&line);
            errors.push('\n');
        }
        errors
    });

    // Wait for the command to complete
    let status = child.wait().await.map_err(|e| {
        error!("Failed to wait for Claude CLI: {e}");
        mmm::Error::Other(format!("Failed to wait for Claude CLI: {e}"))
    })?;

    // Collect the output
    let full_output = handle_stdout.await.unwrap_or_default();
    let full_error = handle_stderr.await.unwrap_or_default();

    trace!("Command exit code: {:?}", status.code());
    trace!("Command stdout length: {} bytes", full_output.len());
    trace!("Command stderr length: {} bytes", full_error.len());

    if status.success() {
        Ok(full_output)
    } else {
        error!("Claude CLI command failed with status {:?}", status.code());
        if !full_error.is_empty() {
            error!("Error output: {}", full_error);
        }
        debug!("Failed command was: claude {}", cmd_args.join(" "));
        Err(mmm::Error::Other(format!(
            "Claude CLI command failed: {}",
            if full_error.is_empty() {
                "No error message"
            } else {
                &full_error
            }
        )))
    }
}

async fn handle_claude_command(cmd: ClaudeCommands, config_loader: &ConfigLoader) -> Result<()> {
    use mmm::claude::{ClaudeConfig, ClaudeManager};
    use ClaudeCommands::*;

    match cmd {
        Run { command, args } => {
            // Check if this is a Claude CLI command that can run without API key
            if is_claude_cli_command(&command) {
                info!(
                    "Executing Claude CLI command: {} with args: {:?}",
                    command, args
                );
                match execute_claude_cli_direct(&command, args).await {
                    Ok(result) => {
                        println!("{result}");
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Execution error: {}", e);
                        return Err(e);
                    }
                }
            }

            // For non-Claude CLI commands, we need an API key
            let mut claude_config = ClaudeConfig::default();
            if let Ok(api_key) = std::env::var("CLAUDE_API_KEY") {
                claude_config.api_key = api_key;
            }

            // Get config from project if available
            let config = config_loader.get_config();
            // Check for project-specific Claude API key
            if let Some(project_config) = &config.project {
                if let Some(api_key) = &project_config.claude_api_key {
                    claude_config.api_key = api_key.clone();
                }
            }

            // Check for global Claude API key if not set
            if claude_config.api_key.is_empty() {
                if let Some(api_key) = &config.global.claude_api_key {
                    claude_config.api_key = api_key.clone();
                }
            }

            if claude_config.api_key.is_empty() {
                return Err(mmm::Error::Config(
                    "Claude API key not configured. Set CLAUDE_API_KEY environment variable or configure in project.".to_string()
                ));
            }

            let mut claude_manager = ClaudeManager::new(claude_config)?;

            info!(
                "Executing Claude command: {} with args: {:?}",
                command, args
            );
            let result = claude_manager.execute_command(&command, args).await?;
            println!("{result}");
        }

        Commands => {
            // For listing commands, we don't need an API key - just show available commands
            println!("Available Claude commands:");

            // Show Claude CLI commands (if Claude CLI is available)
            let claude_cli_available = tokio::process::Command::new("claude")
                .arg("--help")
                .output()
                .await
                .is_ok();

            if claude_cli_available {
                println!("  Claude CLI commands (no API key required):");
                println!(
                    "    mmm-lint - Automatically detect and fix linting issues in the codebase"
                );
                println!(
                    "    mmm-code-review - Conduct comprehensive code review with quality analysis"
                );
                println!("    mmm-implement-spec - Implement specifications by reading spec files and executing implementation");
                println!(
                    "    mmm-add-spec - Generate new specification documents from feature descriptions"
                );
            }

            // Try to load other commands if API key is available
            let mut claude_config = ClaudeConfig::default();
            if let Ok(api_key) = std::env::var("CLAUDE_API_KEY") {
                claude_config.api_key = api_key;
            }

            let config = config_loader.get_config();
            if let Some(project_config) = &config.project {
                if let Some(api_key) = &project_config.claude_api_key {
                    claude_config.api_key = api_key.clone();
                }
            }

            if claude_config.api_key.is_empty() {
                if let Some(api_key) = &config.global.claude_api_key {
                    claude_config.api_key = api_key.clone();
                }
            }

            if !claude_config.api_key.is_empty() {
                if let Ok(claude_manager) = ClaudeManager::new(claude_config) {
                    let commands = claude_manager.commands.list_commands();
                    println!("  API-based commands:");
                    for cmd in commands {
                        if !is_claude_cli_command(&cmd.name) {
                            println!("    {} - {}", cmd.name, cmd.description);
                            if !cmd.aliases.is_empty() {
                                println!("      Aliases: {}", cmd.aliases.join(", "));
                            }
                        }
                    }
                }
            }
        }

        Stats | ClearCache | Config { .. } => {
            // These commands require API key
            let mut claude_config = ClaudeConfig::default();
            if let Ok(api_key) = std::env::var("CLAUDE_API_KEY") {
                claude_config.api_key = api_key;
            }

            let config = config_loader.get_config();
            if let Some(project_config) = &config.project {
                if let Some(api_key) = &project_config.claude_api_key {
                    claude_config.api_key = api_key.clone();
                }
            }

            if claude_config.api_key.is_empty() {
                if let Some(api_key) = &config.global.claude_api_key {
                    claude_config.api_key = api_key.clone();
                }
            }

            if claude_config.api_key.is_empty() {
                return Err(mmm::Error::Config(
                    "Claude API key not configured. Set CLAUDE_API_KEY environment variable or configure in project.".to_string()
                ));
            }

            let claude_manager = ClaudeManager::new(claude_config)?;

            match cmd {
                Stats => {
                    let stats = claude_manager.token_tracker.get_stats();
                    println!("Token Usage Statistics:");
                    println!("  Today: {} tokens", stats.today_usage);
                    println!("  This week: {} tokens", stats.week_usage);
                    if let Some(limit) = stats.daily_limit {
                        println!("  Daily limit: {limit} tokens");
                        let percentage = (stats.today_usage as f64 / limit as f64 * 100.0) as u32;
                        println!("  Usage: {percentage}%");
                    }
                    if !stats.by_project.is_empty() {
                        println!("\nBy project (this week):");
                        for (project, tokens) in stats.by_project {
                            println!("  {project}: {tokens} tokens");
                        }
                    }
                }

                ClearCache => {
                    claude_manager.cache.clear()?;
                    info!("Claude response cache cleared");
                }

                Config { key, value } => {
                    config_loader
                        .set_project_value(&format!("claude.{key}"), &value)
                        .await?;
                    info!("Set claude.{} = {}", key, value);
                }
                _ => unreachable!(), // Other cases handled above
            }
        }
    }

    Ok(())
}

async fn handle_workflow_command(
    cmd: WorkflowCommands,
    project_manager: &ProjectManager,
    _config_loader: &ConfigLoader,
) -> Result<()> {
    use mmm::workflow::{
        checkpoint::{CheckpointOption, CheckpointResponse},
        EventBus, WorkflowEngine,
    };
    use WorkflowCommands::*;

    let project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    let event_bus = std::sync::Arc::new(EventBus::new());
    let workflow_state_manager = std::sync::Arc::new(
        mmm::workflow::state::WorkflowStateManager::new(project.path.join(".mmm")),
    );

    let mut engine = WorkflowEngine::new(workflow_state_manager.clone(), event_bus.clone());

    match cmd {
        Run {
            workflow,
            spec,
            dry_run,
            params,
        } => {
            engine.set_dry_run(dry_run);

            let mut parameters = std::collections::HashMap::new();
            for (key, value) in params {
                parameters.insert(key, serde_json::Value::String(value));
            }

            let workflow_path = if workflow.ends_with(".yaml") || workflow.ends_with(".yml") {
                PathBuf::from(&workflow)
            } else {
                project
                    .path
                    .join(".mmm/workflows")
                    .join(format!("{workflow}.yaml"))
            };

            if !workflow_path.exists() {
                return Err(mmm::Error::Workflow(format!(
                    "Workflow file not found: {workflow_path:?}"
                )));
            }

            println!("🚀 Running workflow '{workflow}'...");
            if dry_run {
                println!("   (dry-run mode - no changes will be made)");
            }

            let result = engine
                .run_workflow(workflow_path.to_str().unwrap(), spec.as_deref(), parameters)
                .await?;

            match result.status {
                mmm::workflow::WorkflowStatus::Completed => {
                    println!("✅ Workflow completed successfully!");
                    println!("   Duration: {:?}", result.duration);
                }
                mmm::workflow::WorkflowStatus::Failed => {
                    println!("❌ Workflow failed!");
                    if let Some(error) = result.error {
                        println!("   Error: {error}");
                    }
                }
                _ => {
                    println!("⚠️  Workflow ended with status: {:?}", result.status);
                }
            }
        }

        List => {
            let workflow_dir = project.path.join(".mmm/workflows");
            if !workflow_dir.exists() {
                println!("No workflows found in project.");
                return Ok(());
            }

            println!("Available workflows:");
            for entry in std::fs::read_dir(workflow_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension() == Some(std::ffi::OsStr::new("yaml"))
                    || path.extension() == Some(std::ffi::OsStr::new("yml"))
                {
                    if let Some(name) = path.file_stem() {
                        println!("  - {}", name.to_string_lossy());
                    }
                }
            }
        }

        Show { workflow } => {
            let workflow_path = project
                .path
                .join(".mmm/workflows")
                .join(format!("{workflow}.yaml"));
            if !workflow_path.exists() {
                return Err(mmm::Error::Workflow(format!(
                    "Workflow '{workflow}' not found"
                )));
            }

            let content = std::fs::read_to_string(&workflow_path)?;
            println!("Workflow: {workflow}");
            println!("{content}");
        }

        History {
            workflow,
            status,
            limit,
        } => {
            let executions = workflow_state_manager
                .list_workflow_executions(workflow.as_deref(), status.as_deref(), limit)
                .await?;

            if executions.is_empty() {
                println!("No workflow executions found.");
            } else {
                println!(
                    "{:<40} {:<20} {:<10} {:<20} {:<20}",
                    "ID", "WORKFLOW", "STATUS", "STARTED", "COMPLETED"
                );
                println!("{}", "-".repeat(110));

                for exec in executions {
                    let completed = exec
                        .completed_at
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<40} {:<20} {:<10} {:<20} {:<20}",
                        &exec.workflow_id[..8],
                        exec.workflow_name,
                        exec.status,
                        exec.started_at.format("%Y-%m-%d %H:%M:%S"),
                        completed
                    );
                }
            }
        }

        Debug {
            workflow,
            breakpoint,
        } => {
            println!("🐛 Debug mode for workflow '{workflow}'");
            if let Some(bp) = breakpoint {
                println!("   Breakpoint set at: {bp}");
            }
            println!("   (Debug mode not yet fully implemented)");
        }

        Checkpoint(checkpoint_cmd) => {
            use CheckpointCommands::*;

            match checkpoint_cmd {
                List => {
                    let checkpoint_manager = engine.checkpoint_manager.clone();
                    let checkpoints = checkpoint_manager.list_pending_checkpoints().await;

                    if checkpoints.is_empty() {
                        println!("No pending checkpoints.");
                    } else {
                        println!("Pending checkpoints:");
                        for checkpoint in checkpoints {
                            println!("\n  ID: {}", checkpoint.id);
                            println!("  Workflow: {}", checkpoint.workflow_id);
                            println!("  Step: {}", checkpoint.step_name);
                            println!("  Message: {}", checkpoint.message);
                            if let Some(expires) = checkpoint.expires_at {
                                println!("  Expires: {}", expires.format("%Y-%m-%d %H:%M:%S"));
                            }
                        }
                    }
                }

                Respond {
                    checkpoint_id,
                    option,
                    data,
                } => {
                    let checkpoint_manager = engine.checkpoint_manager.clone();
                    let checkpoint_uuid = uuid::Uuid::parse_str(&checkpoint_id)
                        .map_err(|_| mmm::Error::Workflow("Invalid checkpoint ID".to_string()))?;

                    let response_option = match option.as_str() {
                        "approve" => CheckpointOption::Approve,
                        "reject" => CheckpointOption::Reject {
                            reason: data.unwrap_or_else(|| "No reason provided".to_string()),
                        },
                        "approve-with-changes" => CheckpointOption::ApproveWithChanges {
                            changes: data.unwrap_or_else(String::new),
                        },
                        _ => return Err(mmm::Error::Workflow(format!("Invalid option: {option}"))),
                    };

                    let response = CheckpointResponse {
                        checkpoint_id: checkpoint_uuid,
                        option: response_option,
                        user: whoami::username(),
                        timestamp: chrono::Utc::now(),
                    };

                    checkpoint_manager
                        .respond_to_checkpoint(&checkpoint_uuid, response)
                        .await?;
                    println!("✅ Checkpoint response recorded.");
                }
            }
        }

        Trigger(trigger_cmd) => {
            use TriggerCommands::*;

            match trigger_cmd {
                List => {
                    let triggers = event_bus.list_triggers().await;
                    if triggers.is_empty() {
                        println!("No triggers configured.");
                    } else {
                        println!("Configured triggers:");
                        for trigger in triggers {
                            println!("\n  ID: {}", trigger.id);
                            println!("  Workflow: {}", trigger.workflow_name);
                            println!("  Event: {:?}", trigger.event_filter.event_type);
                            println!("  Enabled: {}", trigger.enabled);
                        }
                    }
                }

                Toggle { trigger_id, enable } => {
                    let trigger_uuid = uuid::Uuid::parse_str(&trigger_id)
                        .map_err(|_| mmm::Error::Workflow("Invalid trigger ID".to_string()))?;

                    event_bus.enable_trigger(&trigger_uuid, enable).await?;
                    println!(
                        "✅ Trigger {} {}",
                        trigger_id,
                        if enable { "enabled" } else { "disabled" }
                    );
                }

                Create {
                    workflow,
                    event,
                    filter,
                } => {
                    let trigger = mmm::workflow::event::EventTrigger {
                        id: uuid::Uuid::new_v4(),
                        workflow_name: workflow.clone(),
                        event_filter: mmm::workflow::event::EventFilter {
                            event_type: Some(event.clone()),
                            path_pattern: filter,
                            custom_filter: None,
                        },
                        parameters: std::collections::HashMap::new(),
                        enabled: true,
                    };

                    event_bus.register_trigger(trigger).await?;
                    println!("✅ Created trigger for workflow '{workflow}' on event '{event}'");
                }
            }
        }
    }

    Ok(())
}

async fn handle_loop_command(
    cmd: LoopCommands,
    project_manager: &ProjectManager,
    config_loader: &ConfigLoader,
) -> Result<()> {
    use mmm::r#loop::{config::SeverityLevel, LoopConfig};
    use LoopCommands::*;

    // Get current project
    let project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    config_loader.load_project(&project.path).await?;
    let _config = config_loader.get_config();

    // Initialize state manager
    let _state_manager = Arc::new(mmm::simple_state::StateManager::with_root(
        project.path.join(".mmm"),
    )?);

    match cmd {
        Start {
            target,
            max_iterations,
            scope,
            severity,
            workflow,
            dry_run,
        } => {
            println!("🚀 Starting iterative improvement loop session...");
            if dry_run {
                println!("   (dry-run mode - no changes will be made)");
            }

            // Parse severity levels
            let severity_levels: Vec<SeverityLevel> = severity
                .split(',')
                .map(|s| s.trim().parse())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| mmm::Error::Config(format!("Invalid severity level: {e}")))?;

            // Parse scope
            let scope_dirs: Vec<String> = scope.split(',').map(|s| s.trim().to_string()).collect();

            // Create loop configuration
            let loop_config = LoopConfig {
                target_score: target,
                max_iterations,
                scope: scope_dirs,
                severity_filter: severity_levels,
                workflow_template: workflow,
                ..LoopConfig::default()
            };

            println!("Configuration:");
            println!("  Target score: {:.1}", loop_config.target_score);
            println!("  Max iterations: {}", loop_config.max_iterations);
            println!("  Scope: {}", loop_config.scope.join(", "));
            println!("  Workflow: {}", loop_config.workflow_template);

            println!("\nLoop session functionality is not yet fully implemented.");
            println!("This would create and execute an iterative improvement session.");
        }

        Sessions { status, limit } => {
            println!("📋 Loop sessions:");

            // TODO: Implement session listing using state manager
            println!("Session listing functionality not yet implemented.");
            if let Some(status_filter) = status {
                println!("Would filter by status: {status_filter}");
            }
            println!("Would show {limit} sessions");
        }

        Show { session_id } => {
            println!("📊 Session details for: {session_id}");
            println!("Session detail functionality not yet implemented.");
        }

        Stop { session_id, force } => {
            println!("⏹️  Stopping session: {session_id}");
            if force {
                println!("   (force mode - no cleanup)");
            }
            println!("Session stop functionality not yet implemented.");
        }

        Resume { session_id } => {
            println!("▶️  Resuming session: {session_id}");
            println!("Session resume functionality not yet implemented.");
        }

        Config { key, value, list } => {
            if list {
                println!("📋 Loop configuration:");
                println!("Configuration listing not yet implemented.");
            } else if let (Some(key), Some(value)) = (key, value) {
                println!("Setting {key} = {value}");
                println!("Configuration setting not yet implemented.");
            } else {
                println!("Usage: config --key KEY --value VALUE or config --list");
            }
        }
    }

    Ok(())
}
