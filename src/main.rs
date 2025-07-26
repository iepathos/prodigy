use clap::{Parser, Subcommand};
use mmm::{config::ConfigLoader, spec::SpecificationEngine, Result};
use tracing::{debug, error, trace};

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
    /// Run a specification
    Run {
        /// Specification ID
        spec_id: String,
    },

    /// Claude AI integration commands
    #[command(subcommand)]
    Claude(ClaudeCommands),

    /// Improve code quality with zero configuration
    Improve(mmm::improve::command::ImproveCommand),
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

    match cli.command {
        Commands::Run { spec_id } => {
            let config = config_loader.get_config();

            let mut spec_engine =
                SpecificationEngine::new(std::env::current_dir()?.join(config.get_spec_dir()));
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

        Commands::Claude(claude_cmd) => handle_claude_command(claude_cmd, &config_loader).await?,
        Commands::Improve(improve_cmd) => mmm::improve::run(improve_cmd).await?,
    }

    Ok(())
}

async fn handle_claude_command(cmd: ClaudeCommands, _config_loader: &ConfigLoader) -> Result<()> {
    use ClaudeCommands::*;

    match cmd {
        Run { command, args } => {
            println!("Running Claude command: {command} with args: {args:?}");
            println!("Claude command execution not yet implemented");
        }
        Commands => {
            println!("Available Claude commands:");
            println!("  implement - Implement a feature");
            println!("  review - Review code");
            println!("  debug - Debug an issue");
        }
        Stats => {
            println!("Claude usage statistics:");
            println!("  Total tokens: 0");
            println!("  Commands run: 0");
        }
        ClearCache => {
            println!("Claude cache cleared");
        }
        Config { key, value } => {
            println!("Setting Claude config: {key} = {value}");
            println!("Claude configuration not yet implemented");
        }
    }

    Ok(())
}
