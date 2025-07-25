use clap::{Parser, Subcommand};
use mmm::{
    config::ConfigLoader,
    project::ProjectManager,
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
    /// Initialize a new project
    Init {
        /// Project name
        name: String,
        
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    
    /// List all projects
    Projects,
    
    /// Switch to a project
    Switch {
        /// Project name
        name: String,
    },
    
    /// List specifications in current project
    Specs,
    
    /// Run a specification
    Run {
        /// Specification ID
        spec_id: String,
    },
    
    /// Show project status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    tracing_subscriber::fmt()
        .with_env_filter(if cli.verbose { "debug" } else { "info" })
        .init();
    
    let mut config_loader = ConfigLoader::new().await?;
    config_loader.load_global().await?;
    
    let mut project_manager = ProjectManager::new().await?;
    
    match cli.command {
        Commands::Init { name, path } => {
            let project_path = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            let project = project_manager.create_project(&name, &project_path).await?;
            info!("Initialized project '{}' at {:?}", name, project_path);
        }
        
        Commands::Projects => {
            let projects = project_manager.list_projects();
            if projects.is_empty() {
                println!("No projects found.");
            } else {
                println!("Projects:");
                for project in projects {
                    println!("  - {} ({})", project.name, project.path.display());
                }
            }
        }
        
        Commands::Switch { name } => {
            project_manager.switch_project(&name)?;
            info!("Switched to project '{}'", name);
        }
        
        Commands::Specs => {
            let project = project_manager.current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;
            
            config_loader.load_project(&project.path).await?;
            let config = config_loader.get_config();
            
            let mut spec_engine = SpecificationEngine::new(project.path.join(config.get_spec_dir()));
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
            let project = project_manager.current_project()
                .ok_or_else(|| mmm::Error::Project("No project selected".to_string()))?;
            
            config_loader.load_project(&project.path).await?;
            let config = config_loader.get_config();
            
            let mut spec_engine = SpecificationEngine::new(project.path.join(config.get_spec_dir()));
            spec_engine.load_specifications().await?;
            
            let spec = spec_engine.get_specification(&spec_id)
                .ok_or_else(|| mmm::Error::Specification(format!("Specification '{}' not found", spec_id)))?;
            
            info!("Running specification: {} - {}", spec.id, spec.name);
            
            // Here you would integrate with Claude to implement the specification
            println!("Specification content:\n{}", spec.content);
        }
        
        Commands::Status => {
            let project = project_manager.current_project()
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
    }
    
    Ok(())
}