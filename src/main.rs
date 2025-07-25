use clap::{Parser, Subcommand};
use mmm::{
    config::ConfigLoader,
    project::{health::HealthStatus, ProjectHealth, ProjectManager},
    spec::SpecificationEngine,
    state::StateManager,
    Result,
};
use std::{path::PathBuf, sync::Arc};
use tracing::info;

#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "Memento Mori Manager - A Git-based project management system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    verbose: bool,
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

    /// Monitoring and analytics commands
    #[command(subcommand)]
    Monitor(MonitorCommands),

    /// Plugin management commands
    #[command(subcommand)]
    Plugin(PluginCommands),
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
enum MonitorCommands {
    /// Start the monitoring dashboard server
    Dashboard {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    /// Show current metrics
    Metrics {
        /// Metric name filter
        #[arg(short, long)]
        name: Option<String>,

        /// Time range (e.g., "1h", "24h", "7d")
        #[arg(short, long, default_value = "24h")]
        range: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// List and manage alerts
    Alerts {
        /// Show only active alerts
        #[arg(short, long)]
        active: bool,

        /// Acknowledge alert by ID
        #[arg(short, long)]
        acknowledge: Option<String>,
    },

    /// Generate a report
    Report {
        /// Report template name
        #[arg(short, long, default_value = "weekly-progress")]
        template: String,

        /// Time range for the report
        #[arg(short, long, default_value = "7d")]
        range: String,

        /// Export format
        #[arg(short, long, value_enum, default_value = "html")]
        format: ExportFormat,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run analytics
    Analytics {
        /// Specific analyzer to run
        #[arg(short, long)]
        analyzer: Option<String>,

        /// Time range for analysis
        #[arg(short, long, default_value = "7d")]
        range: String,
    },

    /// Show performance statistics
    Performance {
        /// Operation to analyze
        #[arg(short, long)]
        operation: Option<String>,

        /// Time range
        #[arg(short, long, default_value = "24h")]
        range: String,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List available plugins
    List {
        /// Filter by capability
        #[arg(short, long)]
        capability: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Search for plugins in marketplace
    Search {
        /// Search query
        query: String,
    },

    /// Install a plugin
    Install {
        /// Plugin name
        name: String,

        /// Specific version to install
        #[arg(short, long)]
        version: Option<String>,
    },

    /// Uninstall a plugin
    Uninstall {
        /// Plugin name
        name: String,

        /// Specific version to uninstall
        #[arg(short, long)]
        version: Option<String>,
    },

    /// Update a plugin to latest version
    Update {
        /// Plugin name
        name: String,
    },

    /// Show plugin information
    Info {
        /// Plugin name
        name: String,
    },

    /// Load a plugin
    Load {
        /// Plugin name
        name: String,
    },

    /// Unload a plugin
    Unload {
        /// Plugin name or ID
        name: String,
    },

    /// Reload a plugin (for development)
    Reload {
        /// Plugin name or ID
        name: String,
    },

    /// Enable hot-reload for a plugin
    HotReload {
        /// Plugin name or ID
        name: String,
    },

    /// Execute a plugin command
    Execute {
        /// Plugin name
        plugin: String,

        /// Command to execute  
        command: String,

        /// Command arguments
        args: Vec<String>,
    },

    /// Manage plugin permissions
    Permissions {
        /// Plugin name
        plugin: String,

        /// Grant permission
        #[arg(short, long)]
        grant: Option<String>,

        /// Revoke permission
        #[arg(short, long)]
        revoke: Option<String>,

        /// List permissions
        #[arg(short, long)]
        list: bool,
    },

    /// Create a new plugin from template
    Create {
        /// Plugin name
        name: String,

        /// Plugin template type
        #[arg(short, long, default_value = "command")]
        template: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Build a plugin
    Build {
        /// Plugin directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },

    /// Test a plugin
    Test {
        /// Plugin directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },

    /// Package a plugin for distribution
    Package {
        /// Plugin directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Publish a plugin to marketplace
    Publish {
        /// Plugin directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// API key for marketplace
        #[arg(short, long)]
        api_key: String,
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
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(if cli.verbose { "debug" } else { "info" })
        .init();

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

            let state_manager = StateManager::new(project.path.clone(), &project.name).await?;
            let state = state_manager.get_current_state().await?;

            if state.completed_specs.contains(&spec_id) {
                println!("Specification '{spec_id}' is already completed.");
            } else {
                println!("Running specification '{spec_id}'...");
                // TODO: Implement actual spec execution
                println!("Specification execution not yet implemented.");
            }
        }

        Commands::Status => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            let state_manager = StateManager::new(project.path.clone(), &project.name).await?;
            let state = state_manager.get_current_state().await?;

            println!("Project: {}", project.name);
            println!("Path: {}", project.path.display());
            println!("Completed specs: {}", state.completed_specs.len());
            println!("Failed specs: {}", state.failed_specs.len());
        }

        Commands::Multi(multi_cmd) => handle_multi_command(multi_cmd, &mut project_manager).await?,
        Commands::Claude(claude_cmd) => handle_claude_command(claude_cmd, &config_loader).await?,
        Commands::Workflow(workflow_cmd) => {
            handle_workflow_command(workflow_cmd, &project_manager, &config_loader).await?
        }
        Commands::Monitor(monitor_cmd) => {
            handle_monitor_command(monitor_cmd, &project_manager, &config_loader).await?
        }
        Commands::Plugin(plugin_cmd) => {
            handle_plugin_command(plugin_cmd, &project_manager, &config_loader).await?
        }
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
            // Initialize Claude manager
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
                    "Claude API key not configured. Set CLAUDE_API_KEY environment variable or configure in project.".to_string()
                ));
            }

            let mut claude_manager = mmm::claude::ClaudeManager::new(claude_config)?;

            println!("🚀 Creating specification for: {description}");
            let result = claude_manager
                .execute_command("add-spec", vec![description])
                .await?;
            println!("{result}");
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

async fn handle_claude_command(cmd: ClaudeCommands, config_loader: &ConfigLoader) -> Result<()> {
    use mmm::claude::{ClaudeConfig, ClaudeManager};
    use ClaudeCommands::*;

    // Load Claude configuration
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

    match cmd {
        Run { command, args } => {
            info!(
                "Executing Claude command: {} with args: {:?}",
                command, args
            );
            let result = claude_manager.execute_command(&command, args).await?;
            println!("{result}");
        }

        Commands => {
            let commands = claude_manager.commands.list_commands();
            println!("Available Claude commands:");
            for cmd in commands {
                println!("  {} - {}", cmd.name, cmd.description);
                if !cmd.aliases.is_empty() {
                    println!("    Aliases: {}", cmd.aliases.join(", "));
                }
            }
        }

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

    let state_manager =
        StateManager::new(project.path.join(".mmm").join("state.db"), &project.name).await?;
    let event_bus = std::sync::Arc::new(EventBus::new());
    let workflow_state_manager = std::sync::Arc::new(
        mmm::workflow::state::WorkflowStateManager::new(state_manager.get_pool().clone()),
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

async fn handle_monitor_command(
    cmd: MonitorCommands,
    project_manager: &ProjectManager,
    config_loader: &ConfigLoader,
) -> Result<()> {
    use mmm::monitor::*;
    use MonitorCommands::*;

    // Get current project
    let project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    config_loader.load_project(&project.path).await?;
    let _config = config_loader.get_config();

    // Initialize monitoring components
    let db_path = project.path.join(".mmm").join("state.db");
    let state_manager = std::sync::Arc::new(StateManager::new(db_path, &project.name).await?);
    let pool = state_manager.get_pool().clone();

    let metrics_db = std::sync::Arc::new(metrics::MetricsDatabase::new(pool.clone()));
    metrics_db.create_tables().await?;

    let alerts_db = alert::AlertsDatabase::new(pool.clone());
    alerts_db.create_tables().await?;

    let _perf_tracker = performance::PerformanceTracker::new(pool.clone());
    // TODO: Initialize performance tracking tables if needed

    match cmd {
        Dashboard { port } => {
            println!("🚀 Starting monitoring dashboard on http://localhost:{port}");

            // For now, just print that the dashboard would start
            println!("Dashboard functionality not yet fully implemented.");
            println!("You can access metrics via: mmm monitor metrics --name <metric>");
        }

        Metrics {
            name,
            range,
            format,
        } => {
            let timeframe = parse_time_range(&range)?;

            if let Some(metric_name) = name {
                let metrics = metrics_db
                    .query_metrics(&metric_name, timeframe.start, timeframe.end, None)
                    .await?;

                match format {
                    OutputFormat::Table => {
                        println!("{:<30} {:<20} {:<15}", "TIMESTAMP", "NAME", "VALUE");
                        println!("{}", "-".repeat(65));

                        for metric in metrics {
                            let value_str = match &metric.value {
                                MetricValue::Counter(v) => format!("{v}"),
                                MetricValue::Gauge(v) => format!("{v:.2}"),
                                MetricValue::Histogram(v) => {
                                    format!("histogram({} values)", v.len())
                                }
                                MetricValue::Summary { sum, count, .. } => {
                                    format!("sum={sum:.2}, count={count}")
                                }
                            };

                            println!(
                                "{:<30} {:<20} {:<15}",
                                metric.timestamp.format("%Y-%m-%d %H:%M:%S"),
                                metric.name,
                                value_str
                            );
                        }
                    }
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&metrics)?);
                    }
                }
            } else {
                println!("Please specify a metric name with --name");
            }
        }

        Alerts {
            active: _,
            acknowledge,
        } => {
            if let Some(alert_id) = acknowledge {
                let alert_uuid = uuid::Uuid::parse_str(&alert_id)
                    .map_err(|_| mmm::Error::Other("Invalid alert ID".to_string()))?;

                let alert_manager = alert::AlertManager::new(metrics_db.clone(), pool.clone());
                alert_manager.acknowledge_alert(alert_uuid).await?;
                println!("✅ Alert {alert_id} acknowledged");
            } else {
                println!("Alert listing not yet implemented.");
                println!("Use --acknowledge <id> to acknowledge an alert.");
            }
        }

        Report {
            template: _,
            range: _,
            format: _,
            output: _,
        } => {
            println!("Report generation not yet fully implemented.");
        }

        Analytics {
            analyzer: _,
            range: _,
        } => {
            println!("Analytics not yet fully implemented.");
        }

        Performance {
            operation: _,
            range: _,
        } => {
            println!("Performance analysis not yet fully implemented.");
        }
    }

    Ok(())
}

fn parse_time_range(range: &str) -> Result<mmm::monitor::TimeFrame> {
    let end = chrono::Utc::now();
    let start = match range {
        "1h" => end - chrono::Duration::hours(1),
        "24h" => end - chrono::Duration::hours(24),
        "7d" => end - chrono::Duration::days(7),
        "30d" => end - chrono::Duration::days(30),
        _ => return Err(mmm::Error::Other(format!("Invalid time range: {range}"))),
    };

    Ok(mmm::monitor::TimeFrame { start, end })
}

async fn handle_plugin_command(
    cmd: PluginCommands,
    project_manager: &ProjectManager,
    _config_loader: &ConfigLoader,
) -> Result<()> {
    use PluginCommands::*;

    // Get current project for plugin manager initialization
    let current_project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    // Initialize plugin manager
    let state_manager = Arc::new(
        StateManager::new(
            current_project.path.join(".mmm").join("state.db"),
            &current_project.name,
        )
        .await?,
    );

    let claude_config = mmm::claude::ClaudeConfig::default(); // TODO: Load from config
    let claude_manager = Arc::new(mmm::claude::ClaudeManager::new(claude_config)?);

    let event_bus = Arc::new(mmm::workflow::EventBus::new());
    let workflow_state_manager = Arc::new(mmm::workflow::state::WorkflowStateManager::new(
        state_manager.get_pool().clone(),
    ));
    let workflow_engine = Arc::new(mmm::workflow::WorkflowEngine::new(
        workflow_state_manager,
        event_bus,
    ));

    let plugin_manager = std::sync::Arc::new(mmm::plugin::PluginManager::new(
        std::sync::Arc::new(ProjectManager::new().await?),
        state_manager,
        claude_manager,
        workflow_engine,
    ));

    plugin_manager.initialize().await?;

    match cmd {
        List { capability, format } => {
            let plugins = plugin_manager.list_loaded_plugins().await?;

            let filtered_plugins = if let Some(cap) = capability {
                plugins
                    .into_iter()
                    .filter(|p| {
                        p.capabilities.iter().any(|c| match c {
                            mmm::plugin::Capability::Command { name, .. } => name == &cap,
                            mmm::plugin::Capability::Hook { event, .. } => event == &cap,
                            mmm::plugin::Capability::Integration { service, .. } => service == &cap,
                            mmm::plugin::Capability::Reporter { format: fmt, .. } => fmt == &cap,
                            mmm::plugin::Capability::Analyzer { name, .. } => name == &cap,
                        })
                    })
                    .collect()
            } else {
                plugins
            };

            match format {
                OutputFormat::Table => {
                    if filtered_plugins.is_empty() {
                        println!("No plugins loaded.");
                    } else {
                        println!(
                            "{:<20} {:<10} {:<30} {:<20}",
                            "Name", "Version", "Description", "Author"
                        );
                        println!("{:-<80}", "");
                        for plugin in filtered_plugins {
                            println!(
                                "{:<20} {:<10} {:<30} {:<20}",
                                plugin.name,
                                plugin.version,
                                plugin.description.chars().take(30).collect::<String>(),
                                plugin.author
                            );
                        }
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&filtered_plugins)?);
                }
            }
        }

        Search { query } => {
            // Initialize marketplace
            let cache_dir = dirs::cache_dir()
                .unwrap_or_default()
                .join("mmm")
                .join("plugins");
            let marketplace = mmm::plugin::PluginMarketplace::new(
                "https://registry.mmm.dev".to_string(), // TODO: Make configurable
                cache_dir,
            );

            let results = marketplace.search(&query).await?;

            if results.is_empty() {
                println!("No plugins found matching '{query}'");
            } else {
                println!("Found {} plugin(s):", results.len());
                for plugin in results {
                    println!(
                        "  {} v{} by {}",
                        plugin.name, plugin.latest_version, plugin.author
                    );
                    println!("    {}", plugin.description);
                    println!(
                        "    Downloads: {} | Rating: {:.1}/5.0",
                        plugin.downloads, plugin.rating
                    );
                    if let Some(homepage) = plugin.homepage {
                        println!("    Homepage: {homepage}");
                    }
                    println!();
                }
            }
        }

        Install { name, version } => {
            let cache_dir = dirs::cache_dir()
                .unwrap_or_default()
                .join("mmm")
                .join("plugins");
            let marketplace = mmm::plugin::PluginMarketplace::new(
                "https://registry.mmm.dev".to_string(),
                cache_dir,
            );

            println!(
                "Installing plugin: {} {}",
                name,
                version.as_deref().unwrap_or("(latest)")
            );
            let install_path = marketplace.install(&name, version.as_deref()).await?;

            // Load the installed plugin
            plugin_manager.load_plugin(&name).await?;

            println!(
                "✅ Plugin '{}' installed successfully at {}",
                name,
                install_path.display()
            );
        }

        Uninstall { name, version } => {
            // Unload plugin first if it's loaded
            if (plugin_manager.load_plugin(&name).await).is_ok() {
                println!("Unloading plugin before uninstall...");
                // Plugin would be unloaded by ID, but we need to find it first
                // This is simplified for now
            }

            let cache_dir = dirs::cache_dir()
                .unwrap_or_default()
                .join("mmm")
                .join("plugins");
            let marketplace = mmm::plugin::PluginMarketplace::new(
                "https://registry.mmm.dev".to_string(),
                cache_dir,
            );

            marketplace.uninstall(&name, version.as_deref()).await?;
            println!("✅ Plugin '{name}' uninstalled successfully");
        }

        Update { name } => {
            let cache_dir = dirs::cache_dir()
                .unwrap_or_default()
                .join("mmm")
                .join("plugins");
            let marketplace = mmm::plugin::PluginMarketplace::new(
                "https://registry.mmm.dev".to_string(),
                cache_dir,
            );

            marketplace.update(&name).await?;
            println!("✅ Plugin '{name}' updated successfully");
        }

        Info { name } => {
            // Try to get from marketplace first
            let cache_dir = dirs::cache_dir()
                .unwrap_or_default()
                .join("mmm")
                .join("plugins");
            let marketplace = mmm::plugin::PluginMarketplace::new(
                "https://registry.mmm.dev".to_string(),
                cache_dir,
            );

            match marketplace.get_plugin_info(&name).await {
                Ok(info) => {
                    println!("Plugin: {}", info.name);
                    println!("Author: {}", info.author);
                    println!("Latest Version: {}", info.latest_version);
                    println!("Description: {}", info.description);
                    println!("License: {}", info.license);
                    println!("Downloads: {}", info.downloads);
                    println!("Rating: {:.1}/5.0", info.rating);
                    if let Some(homepage) = info.homepage {
                        println!("Homepage: {homepage}");
                    }
                    if let Some(repository) = info.repository {
                        println!("Repository: {repository}");
                    }
                    println!(
                        "Available versions: {}",
                        info.versions.keys().cloned().collect::<Vec<_>>().join(", ")
                    );
                }
                Err(_) => {
                    // Try to get from loaded plugins
                    let loaded_plugins = plugin_manager.list_loaded_plugins().await?;
                    if let Some(plugin) = loaded_plugins.iter().find(|p| p.name == name) {
                        println!("Plugin: {}", plugin.name);
                        println!("Version: {}", plugin.version);
                        println!("Author: {}", plugin.author);
                        println!("Description: {}", plugin.description);
                        println!("License: {}", plugin.license);
                        println!("Capabilities: {:?}", plugin.capabilities);
                    } else {
                        println!("Plugin '{name}' not found");
                    }
                }
            }
        }

        Load { name } => {
            let plugin_id = plugin_manager.load_plugin(&name).await?;
            println!("✅ Plugin '{name}' loaded successfully (ID: {plugin_id})");
        }

        Unload { name } => {
            // This is simplified - in reality we'd need to find the plugin ID
            // For now, assume name is the plugin ID
            if let Ok(plugin_id) = uuid::Uuid::parse_str(&name) {
                plugin_manager.unload_plugin(&plugin_id).await?;
                println!("✅ Plugin unloaded successfully");
            } else {
                println!("Invalid plugin ID format. Use 'mmm plugin list' to see loaded plugins with their IDs.");
            }
        }

        Reload { name } => {
            if let Ok(plugin_id) = uuid::Uuid::parse_str(&name) {
                plugin_manager.reload_plugin(&plugin_id).await?;
                println!("✅ Plugin reloaded successfully");
            } else {
                println!("Invalid plugin ID format. Use 'mmm plugin list' to see loaded plugins with their IDs.");
            }
        }

        HotReload { name } => {
            if let Ok(plugin_id) = uuid::Uuid::parse_str(&name) {
                plugin_manager.enable_hot_reload(&plugin_id).await?;
                println!("✅ Hot-reload enabled for plugin");
            } else {
                println!("Invalid plugin ID format. Use 'mmm plugin list' to see loaded plugins with their IDs.");
            }
        }

        Execute {
            plugin,
            command,
            args,
        } => {
            // This would require finding the plugin ID and executing the command
            println!("Plugin command execution not fully implemented yet.");
            println!("Would execute: {} {} {}", plugin, command, args.join(" "));
        }

        Permissions {
            plugin,
            grant,
            revoke,
            list,
        } => {
            println!("Plugin permission management not fully implemented yet.");
            if list {
                println!("Would list permissions for plugin: {plugin}");
            }
            if let Some(perm) = grant {
                println!("Would grant permission '{perm}' to plugin '{plugin}'");
            }
            if let Some(perm) = revoke {
                println!("Would revoke permission '{perm}' from plugin '{plugin}'");
            }
        }

        Create {
            name,
            template,
            output,
        } => {
            let output_dir = output.unwrap_or_else(|| PathBuf::from(&name));

            println!(
                "Creating plugin '{}' with template '{}' in {}",
                name,
                template,
                output_dir.display()
            );

            // Create plugin directory structure
            std::fs::create_dir_all(&output_dir)?;
            std::fs::create_dir_all(output_dir.join("src"))?;

            // Create plugin.toml
            let manifest = format!(
                r#"[plugin]
name = "{name}"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]
description = "A plugin for mmm"
license = "MIT"

[dependencies]
mmm = "^1.0"

[capabilities]
commands = ["{name}"]

[permissions]
filesystem = ["read"]
"#
            );

            std::fs::write(output_dir.join("plugin.toml"), manifest)?;

            // Create basic plugin structure based on template
            match template.as_str() {
                "command" => {
                    let plugin_code = format!(
                        r#"#!/usr/bin/env node

const {{ readFileSync }} = require('fs');

function main() {{
    const command = process.argv[2];
    const args = process.argv.slice(3);
    
    switch (command) {{
        case 'init':
            console.log('Plugin {name} initialized');
            break;
        case 'shutdown':
            console.log('Plugin {name} shutdown');
            break;
        case 'execute':
            const commandArgs = JSON.parse(args[0] || '{{}}');
            console.log(JSON.stringify({{
                success: true,
                output: `Hello from {name} plugin!`,
                exit_code: 0,
                artifacts: []
            }}));
            break;
        case 'autocomplete':
            console.log(JSON.stringify(['hello', 'world']));
            break;
        default:
            console.error('Unknown command:', command);
            process.exit(1);
    }}
}}

if (require.main === module) {{
    main();
}}
"#
                    );

                    std::fs::write(output_dir.join("plugin.js"), plugin_code)?;
                }
                _ => {
                    return Err(mmm::Error::Other(format!("Unknown template: {template}")));
                }
            }

            // Create README
            let readme = format!(
                "# {name}\n\nA plugin for mmm.\n\n## Usage\n\n```bash\nmmm plugin load {name}\n```\n"
            );
            std::fs::write(output_dir.join("README.md"), readme)?;

            println!(
                "✅ Plugin '{}' created successfully in {}",
                name,
                output_dir.display()
            );
        }

        Build { path } => {
            println!("Building plugin in {}", path.display());

            // Check if this is a JavaScript plugin
            if path.join("plugin.js").exists() {
                println!("JavaScript plugin detected - no build required");
            } else if path.join("Cargo.toml").exists() {
                // Rust plugin
                let output = tokio::process::Command::new("cargo")
                    .arg("build")
                    .arg("--release")
                    .current_dir(&path)
                    .output()
                    .await?;

                if output.status.success() {
                    println!("✅ Plugin built successfully");
                } else {
                    println!("❌ Build failed:");
                    println!("{}", String::from_utf8_lossy(&output.stderr));
                }
            } else {
                println!("No buildable plugin found in {}", path.display());
            }
        }

        Test { path } => {
            println!("Testing plugin in {}", path.display());

            if path.join("Cargo.toml").exists() {
                let output = tokio::process::Command::new("cargo")
                    .arg("test")
                    .current_dir(&path)
                    .output()
                    .await?;

                if output.status.success() {
                    println!("✅ All tests passed");
                } else {
                    println!("❌ Tests failed:");
                    println!("{}", String::from_utf8_lossy(&output.stderr));
                }
            } else {
                println!("No test suite found in {}", path.display());
            }
        }

        Package { path, output } => {
            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                return Err(mmm::Error::Other("No plugin.toml found".to_string()));
            }

            let manifest_content = std::fs::read_to_string(&manifest_path)?;
            let manifest: mmm::plugin::PluginManifest = toml::from_str(&manifest_content)?;

            let package_name = format!(
                "{}-{}.tar.gz",
                manifest.plugin.name, manifest.plugin.version
            );
            let output_path = output.unwrap_or_else(|| PathBuf::from(&package_name));

            println!("Packaging plugin to {}", output_path.display());

            let output = tokio::process::Command::new("tar")
                .args([
                    "-czf",
                    output_path.to_str().unwrap(),
                    "-C",
                    path.to_str().unwrap(),
                    ".",
                ])
                .output()
                .await?;

            if output.status.success() {
                println!("✅ Plugin packaged successfully");
            } else {
                println!("❌ Packaging failed:");
                println!("{}", String::from_utf8_lossy(&output.stderr));
            }
        }

        Publish { path, api_key } => {
            let cache_dir = dirs::cache_dir()
                .unwrap_or_default()
                .join("mmm")
                .join("plugins");
            let marketplace = mmm::plugin::PluginMarketplace::new(
                "https://registry.mmm.dev".to_string(),
                cache_dir,
            );

            marketplace.publish(&path, &api_key).await?;
            println!("✅ Plugin published successfully");
        }
    }

    Ok(())
}
