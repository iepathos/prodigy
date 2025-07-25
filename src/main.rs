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
        destination: String,
    },

    /// Archive a project
    Archive {
        /// Project name
        name: String,
    },

    /// Unarchive a project
    Unarchive {
        /// Project name
        name: String,
    },

    /// Delete a project
    Delete {
        /// Project name
        name: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Project configuration commands
    #[command(subcommand)]
    Config(ProjectConfigCommands),
}

#[derive(Subcommand)]
enum ProjectConfigCommands {
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },

    /// List all configuration values
    List,
}

#[derive(Subcommand)]
enum TemplateCommands {
    /// List available templates
    List,

    /// Create a template from existing project
    Create {
        /// Template name
        name: String,
        /// Source project
        #[arg(short, long)]
        from_project: String,
    },

    /// Install a template from URL
    Install {
        /// Template URL
        url: String,
    },

    /// Remove a template
    Remove {
        /// Template name
        name: String,
    },
}

#[derive(Subcommand)]
enum ClaudeCommands {
    /// Execute a Claude command (implement, review, debug, plan, explain)
    #[command(name = "run")]
    Run {
        /// Command to execute
        command: String,
        /// Command arguments
        args: Vec<String>,
    },

    /// List available Claude commands
    #[command(name = "commands")]
    Commands,

    /// Show token usage statistics
    #[command(name = "stats")]
    Stats,

    /// Clear response cache
    #[command(name = "clear-cache")]
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

#[derive(clap::ValueEnum, Clone)]
enum OutputFormat {
    Table,
    Json,
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

            let spec = spec_engine.get_specification(&spec_id).ok_or_else(|| {
                mmm::Error::Specification(format!("Specification '{}' not found", spec_id))
            })?;

            info!("Running specification: {} - {}", spec.id, spec.name);

            // Here you would integrate with Claude to implement the specification
            println!("Specification content:\n{}", spec.content);
        }

        Commands::Status => {
            let project = project_manager
                .current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

            let db_path = project.path.join(".mmm").join("state.db");
            let state_manager = StateManager::new(db_path, &project.name).await?;
            let state = state_manager.get_current_state().await?;

            println!("Project: {}", project.name);
            println!("Path: {}", project.path.display());
            println!("Current spec: {:?}", state.current_spec);
            println!("Completed specs: {}", state.completed_specs.len());
            println!("Failed specs: {}", state.failed_specs.len());
        }

        Commands::Multi(multi_cmd) => handle_multi_command(multi_cmd, &mut project_manager).await?,
        Commands::Claude(claude_cmd) => handle_claude_command(claude_cmd, &config_loader).await?,
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
            let project_path = path.unwrap_or_else(|| std::env::current_dir().unwrap());

            if let Some(template_name) = template {
                let template_manager = TemplateManager::new().await?;
                template_manager
                    .create_from_template(&name, &project_path, &template_name)
                    .await?;
            } else {
                project_manager.create_project(&name, &project_path).await?;
            }

            info!("Created project '{}' at {:?}", name, project_path);
        }

        Init { name } => {
            let project_path = std::env::current_dir().unwrap();
            let project_name = name.unwrap_or_else(|| {
                project_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            });

            project_manager
                .create_project(&project_name, &project_path)
                .await?;
            info!(
                "Initialized project '{}' in current directory",
                project_name
            );
        }

        List { format } => {
            let projects = project_manager.list_projects();

            match format {
                OutputFormat::Table => {
                    if projects.is_empty() {
                        println!("No projects found.");
                    } else {
                        println!("Projects:");
                        for project in projects {
                            let archived = if project.archived { " [ARCHIVED]" } else { "" };
                            println!(
                                "  - {} ({}){}",
                                project.name,
                                project.path.display(),
                                archived
                            );
                        }
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
                project_manager.get_project(&name)?.clone()
            } else {
                project_manager
                    .current_project()
                    .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?
                    .clone()
            };

            let health = ProjectHealth::check(&project).await?;

            println!("Project: {}", project.name);
            println!("Path: {}", project.path.display());
            println!("Created: {}", project.created.format("%Y-%m-%d %H:%M:%S"));
            println!(
                "Last accessed: {}",
                project.last_accessed.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(template) = &project.template {
                println!("Template: {}", template);
            }
            if let Some(description) = &project.description {
                println!("Description: {}", description);
            }
            if !project.tags.is_empty() {
                println!("Tags: {}", project.tags.join(", "));
            }

            println!("\nHealth checks:");
            for check in health.checks {
                let status_str = match check.status {
                    mmm::project::HealthStatus::Passing => "✓",
                    mmm::project::HealthStatus::Warning => "⚠",
                    mmm::project::HealthStatus::Failing => "✗",
                };
                println!(
                    "  {} {}: {}",
                    status_str,
                    check.name,
                    check.message.unwrap_or_default()
                );
            }
        }

        Switch { name } => {
            project_manager.switch_project(&name).await?;
            info!("Switched to project '{}'", name);
        }

        Clone {
            source,
            destination,
        } => {
            project_manager.clone_project(&source, &destination).await?;
            info!("Cloned project '{}' to '{}'", source, destination);
        }

        Archive { name } => {
            project_manager.archive_project(&name).await?;
            info!("Archived project '{}'", name);
        }

        Unarchive { name } => {
            project_manager.unarchive_project(&name).await?;
            info!("Unarchived project '{}'", name);
        }

        Delete { name, force } => {
            if !force {
                println!(
                    "Are you sure you want to delete project '{}'? This action cannot be undone.",
                    name
                );
                println!("Type 'yes' to confirm:");

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if input.trim() != "yes" {
                    println!("Deletion cancelled.");
                    return Ok(());
                }
            }

            project_manager.delete_project(&name).await?;
            info!("Deleted project '{}'", name);
        }

        Config(config_cmd) => {
            handle_project_config_command(config_cmd, project_manager, config_loader).await?
        }
    }

    Ok(())
}

async fn handle_project_config_command(
    cmd: ProjectConfigCommands,
    project_manager: &mut ProjectManager,
    config_loader: &ConfigLoader,
) -> Result<()> {
    use ProjectConfigCommands::*;

    let _project = project_manager
        .current_project()
        .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;

    match cmd {
        Get { key } => {
            let value = config_loader.get_project_value(&key)?;
            println!("{}", value);
        }

        Set { key, value } => {
            config_loader.set_project_value(&key, &value).await?;
            info!("Set {} = {}", key, value);
        }

        List => {
            let config = config_loader.get_config();
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }
    }

    Ok(())
}

async fn handle_template_command(cmd: TemplateCommands) -> Result<()> {
    use TemplateCommands::*;

    let template_manager = TemplateManager::new().await?;

    match cmd {
        List => {
            let templates = template_manager.list_templates().await?;
            if templates.is_empty() {
                println!("No templates found.");
            } else {
                println!("Available templates:");
                for template in templates {
                    println!(
                        "  - {} (v{}): {}",
                        template.name, template.version, template.description
                    );
                }
            }
        }

        Create { name, from_project } => {
            template_manager
                .create_from_project(&name, &from_project)
                .await?;
            info!(
                "Created template '{}' from project '{}'",
                name, from_project
            );
        }

        Install { url } => {
            let name = template_manager.install_from_url(&url).await?;
            info!("Installed template '{}' from {}", name, url);
        }

        Remove { name } => {
            template_manager.remove_template(&name).await?;
            info!("Removed template '{}'", name);
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

            for project_name in project_names {
                println!("Running spec '{}' in project '{}'...", spec, project_name);
                // Implementation would integrate with spec runner
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
            // Implementation would sync configurations
        }

        Report { format, output } => {
            println!("Generating {} report...", format);
            // Implementation would generate reports

            if let Some(output_path) = output {
                println!("Report saved to: {}", output_path.display());
            }
        }

        Update { component } => {
            println!("Updating component '{}' across all projects...", component);
            // Implementation would update components
        }
    }

    Ok(())
}

async fn handle_claude_command(cmd: ClaudeCommands, config_loader: &ConfigLoader) -> Result<()> {
    use mmm::claude::{ClaudeConfig, ClaudeManager};
    use ClaudeCommands::*;

    // Load Claude configuration
    let mut claude_config = ClaudeConfig::default();

    // Get API key from environment or config
    if let Ok(api_key) = std::env::var("CLAUDE_API_KEY") {
        claude_config.api_key = api_key;
    } else if let Ok(api_key) = config_loader.get_project_value("claude.api_key") {
        claude_config.api_key = api_key;
    } else {
        return Err(mmm::Error::Config(
            "Claude API key not found. Set CLAUDE_API_KEY environment variable or use 'mmm claude config api_key <key>'".to_string()
        ));
    }

    // Initialize Claude manager
    let mut claude_manager = ClaudeManager::new(claude_config)?;

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
