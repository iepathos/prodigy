use clap::{Parser, Subcommand};
use mmm::{
    config::ConfigLoader,
    project::{ProjectHealth, ProjectManager, TemplateManager},
    spec::SpecificationEngine,
    state::StateManager,
    Result,
};
use std::path::PathBuf;
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

    /// Template management commands
    #[command(subcommand)]
    Template(TemplateCommands),

    /// List specifications in current project
    Specs,

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
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Create a new project
    New {
        /// Project name
        name: String,

        /// Use a template
        #[arg(short, long)]
        template: Option<String>,

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
enum TemplateCommands {
    /// List available templates
    List,

    /// Show template details
    Show {
        /// Template name
        name: String,
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
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
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
        Commands::Template(template_cmd) => handle_template_command(template_cmd).await?,

        Commands::Specs => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            config_loader.load_project(&project.path).await?;
            let config = config_loader.get_config();

            let mut spec_engine =
                SpecificationEngine::new(project.path.join(config.get_spec_dir()));
            spec_engine.load_specifications().await?;

            let specs = spec_engine.topological_sort()?;
            if specs.is_empty() {
                println!("No specifications found.");
            } else {
                println!("Specifications:");
                for spec_id in specs {
                    if let Some(spec) = spec_engine.get_specification(&spec_id) {
                        println!("  - {} ({}): {:?}", spec.id, spec.name, spec.status);
                    }
                }
            }
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
                    "Specification '{}' not found",
                    spec_id
                )));
            }

            let state_manager = StateManager::new(&project.path).await?;
            let state = state_manager.get_state().await?;

            if state.completed_specs.contains(&spec_id) {
                println!("Specification '{}' is already completed.", spec_id);
            } else {
                println!("Running specification '{}'...", spec_id);
                // TODO: Implement actual spec execution
                println!("Specification execution not yet implemented.");
            }
        }

        Commands::Status => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            let state_manager = StateManager::new(&project.path).await?;
            let state = state_manager.get_state().await?;

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
        New {
            name,
            template,
            path,
        } => {
            let project_path = path.unwrap_or_else(|| PathBuf::from(&name));
            let created = project_manager
                .create_project(&name, &project_path, template.as_deref())
                .await?;
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
                .init_project(&project_name, &current_dir)
                .await?;
            info!(
                "Initialized project '{}' in {}",
                initialized.name,
                initialized.path.display()
            );
        }

        List { format } => {
            let projects = project_manager.list_projects()?;
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
                            if project.active { "active" } else { "inactive" },
                            project.path.display()
                        );
                    }
                }
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&projects)?;
                    println!("{}", json);
                }
            }
        }

        Info { name } => {
            let project = if let Some(name) = name {
                project_manager
                    .get_project(&name)?
                    .ok_or_else(|| mmm::Error::Project(format!("Project '{}' not found", name)))?
            } else {
                project_manager
                    .current_project()
                    .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?
                    .clone()
            };

            println!("Project: {}", project.name);
            println!("Path: {}", project.path.display());
            println!(
                "Status: {}",
                if project.active { "active" } else { "inactive" }
            );
            println!("Created: {}", project.created_at);
            if let Some(template) = &project.template {
                println!("Template: {}", template);
            }
            if let Some(desc) = &project.description {
                println!("Description: {}", desc);
            }

            // Show health status
            let health = ProjectHealth::check(&project.path).await?;
            println!("\nHealth Status:");
            for issue in &health.issues {
                println!("  ⚠️  {}", issue);
            }
            if health.issues.is_empty() {
                println!("  ✅ All checks passed");
            }
        }

        Switch { name } => {
            project_manager.switch_project(&name)?;
            info!("Switched to project '{}'", name);
        }

        Clone { source, dest } => {
            let cloned = project_manager.clone_project(&source, &dest).await?;
            info!(
                "Cloned project '{}' to '{}' at {}",
                source,
                dest,
                cloned.path.display()
            );
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

            project_manager.archive_project(&project_name)?;
            info!("Archived project '{}'", project_name);
        }

        Delete { name, force } => {
            if !force {
                print!("Are you sure you want to delete project '{}'? [y/N] ", name);
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
                project_manager
                    .get_project(&name)?
                    .ok_or_else(|| mmm::Error::Project(format!("Project '{}' not found", name)))?
            } else {
                project_manager
                    .current_project()
                    .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?
                    .clone()
            };

            let mut health = ProjectHealth::check(&project.path).await?;

            println!("Health check for project '{}':", project.name);
            if health.issues.is_empty() {
                println!("✅ All checks passed!");
            } else {
                println!("Found {} issue(s):", health.issues.len());
                for issue in &health.issues {
                    println!("  ⚠️  {}", issue);
                }

                if fix {
                    println!("\nAttempting to fix issues...");
                    health.fix_issues(&project.path).await?;
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
                println!("  spec_dir: {}", config.get_spec_dir());
                println!("  state_dir: {}", config.get_state_dir());
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

async fn handle_template_command(cmd: TemplateCommands) -> Result<()> {
    use TemplateCommands::*;

    let template_manager = TemplateManager::new();

    match cmd {
        List => {
            let templates = template_manager.list_templates();
            println!("Available templates:");
            for (name, template) in templates {
                println!("  {} - {}", name, template.description);
            }
        }

        Show { name } => {
            if let Some(template) = template_manager.get_template(&name) {
                println!("Template: {}", name);
                println!("Description: {}", template.description);
                println!("\nConfiguration:");
                println!("{}", serde_yaml::to_string(&template.config)?);
                println!("\nFiles:");
                for (path, _) in &template.files {
                    println!("  {}", path);
                }
            } else {
                return Err(mmm::Error::Project(format!(
                    "Template '{}' not found",
                    name
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
                if let Some(project) = project_manager.get_project(project_name)? {
                    println!("\n📁 Project: {}", project.name);
                    // TODO: Actually run the spec
                    println!("  Would run spec '{}' here", spec);
                } else {
                    println!("\n⚠️  Project '{}' not found", project_name);
                }
            }
        }

        SyncConfig { source, targets } => {
            let target_projects = if targets == "all" {
                project_manager
                    .list_projects()?
                    .into_iter()
                    .map(|p| p.name)
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
                println!("  Would sync to project: {}", target);
            }
        }

        Report { format, output } => {
            let projects = project_manager.list_projects()?;

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
                println!("{}", report_content);
            }
        }

        Update { component } => {
            let projects = project_manager.list_projects()?;

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
        claude_config.api_key = Some(api_key);
    }

    // Get config from project if available
    let config = config_loader.get_config();
    if let Some(project_claude_config) = config.get_claude_config() {
        claude_config = project_claude_config.clone();
    }

    if claude_config.api_key.is_none() {
        return Err(mmm::Error::Config(
            "Claude API key not configured. Set CLAUDE_API_KEY environment variable or configure in project.".to_string()
        ));
    }

    let claude_manager = ClaudeManager::new(claude_config)?;

    match cmd {
        Run { command, args } => {
            info!(
                "Executing Claude command: {} with args: {:?}",
                command, args
            );
            let result = claude_manager.execute_command(&command, args).await?;
            println!("{}", result);
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
                println!("  Daily limit: {} tokens", limit);
                let percentage = (stats.today_usage as f64 / limit as f64 * 100.0) as u32;
                println!("  Usage: {}%", percentage);
            }
            if !stats.by_project.is_empty() {
                println!("\nBy project (this week):");
                for (project, tokens) in stats.by_project {
                    println!("  {}: {} tokens", project, tokens);
                }
            }
        }

        ClearCache => {
            claude_manager.cache.clear()?;
            info!("Claude response cache cleared");
        }

        Config { key, value } => {
            config_loader
                .set_project_value(&format!("claude.{}", key), &value)
                .await?;
            info!("Set claude.{} = {}", key, value);
        }
    }

    Ok(())
}

async fn handle_workflow_command(
    cmd: WorkflowCommands,
    project_manager: &ProjectManager,
    config_loader: &ConfigLoader,
) -> Result<()> {
    use mmm::workflow::{
        checkpoint::{CheckpointOption, CheckpointResponse},
        EventBus, WorkflowEngine,
    };
    use WorkflowCommands::*;

    let project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    let state_manager = StateManager::new(&project.path).await?;
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
                    .join(format!("{}.yaml", workflow))
            };

            if !workflow_path.exists() {
                return Err(mmm::Error::Workflow(format!(
                    "Workflow file not found: {:?}",
                    workflow_path
                )));
            }

            println!("🚀 Running workflow '{}'...", workflow);
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
                        println!("   Error: {}", error);
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
                .join(format!("{}.yaml", workflow));
            if !workflow_path.exists() {
                return Err(mmm::Error::Workflow(format!(
                    "Workflow '{}' not found",
                    workflow
                )));
            }

            let content = std::fs::read_to_string(&workflow_path)?;
            println!("Workflow: {}", workflow);
            println!("{}", content);
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
            println!("🐛 Debug mode for workflow '{}'", workflow);
            if let Some(bp) = breakpoint {
                println!("   Breakpoint set at: {}", bp);
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
                            changes: data.unwrap_or_else(|| String::new()),
                        },
                        _ => {
                            return Err(mmm::Error::Workflow(format!("Invalid option: {}", option)))
                        }
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
                    println!(
                        "✅ Created trigger for workflow '{}' on event '{}'",
                        workflow, event
                    );
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
    let config = config_loader.get_config();

    // Initialize monitoring components
    let db_path = project.path.join(".mmm").join("state.db");
    let state_manager = std::sync::Arc::new(StateManager::new(db_path, &project.name).await?);
    let pool = state_manager.get_pool().clone();
    
    let metrics_db = std::sync::Arc::new(metrics::MetricsDatabase::new(pool.clone()));
    metrics_db.create_tables().await?;

    let alerts_db = alert::AlertsDatabase::new(pool.clone());
    alerts_db.create_tables().await?;

    let perf_storage = performance::TraceStorage::new(pool.clone());
    perf_storage.create_tables().await?;

    match cmd {
        Dashboard { port } => {
            println!("🚀 Starting monitoring dashboard on http://localhost:{}", port);
            
            // For now, just print that the dashboard would start
            println!("Dashboard functionality not yet fully implemented.");
            println!("You can access metrics via: mmm monitor metrics --name <metric>");
        }

        Metrics { name, range, format } => {
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
                                MetricValue::Counter(v) => format!("{}", v),
                                MetricValue::Gauge(v) => format!("{:.2}", v),
                                MetricValue::Histogram(v) => format!("histogram({} values)", v.len()),
                                MetricValue::Summary { sum, count, .. } => {
                                    format!("sum={:.2}, count={}", sum, count)
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

        Alerts { active: _, acknowledge } => {
            if let Some(alert_id) = acknowledge {
                let alert_uuid = uuid::Uuid::parse_str(&alert_id)
                    .map_err(|_| mmm::Error::Other("Invalid alert ID".to_string()))?;
                
                let alert_manager = alert::AlertManager::new(metrics_db.clone(), pool.clone());
                alert_manager.acknowledge_alert(alert_uuid).await?;
                println!("✅ Alert {} acknowledged", alert_id);
            } else {
                println!("Alert listing not yet implemented.");
                println!("Use --acknowledge <id> to acknowledge an alert.");
            }
        }

        Report { template: _, range: _, format: _, output: _ } => {
            println!("Report generation not yet fully implemented.");
        }

        Analytics { analyzer: _, range: _ } => {
            println!("Analytics not yet fully implemented.");
        }

        Performance { operation: _, range: _ } => {
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
        _ => return Err(mmm::Error::Other(format!("Invalid time range: {}", range))),
    };
    
    Ok(mmm::monitor::TimeFrame { start, end })
}
