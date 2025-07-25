use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info, warn};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "mmm")]
#[command(about = "Memento Mori - A self-sufficient loop implementation for Claude CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, value_name = "DIR", default_value = "specs")]
    specs_dir: PathBuf,

    #[arg(short, long, default_value = "3")]
    max_iterations: u32,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(short, long)]
        spec: Option<String>,
        #[arg(long)]
        command: Option<String>,
    },
    List,
    Init {
        #[arg(short, long, value_name = "NAME")]
        name: String,
    },
    Add {
        #[arg(short, long, value_name = "NAME")]
        name: String,
        #[arg(short, long, help = "Use Claude to generate the spec")]
        generate: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Spec {
    name: String,
    path: PathBuf,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoopState {
    iteration: u32,
    specs: Vec<Spec>,
    completed: Vec<String>,
    in_progress: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    claude: ClaudeConfig,
    commands: CommandsConfig,
    templates: TemplatesConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeConfig {
    default_args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandsConfig {
    implement: String,
    lint: String,
    test: String,
    review: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TemplatesConfig {
    implement: String,
    lint: String,
    test: String,
    review: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            claude: ClaudeConfig {
                default_args: vec!["--no-preamble".to_string()],
            },
            commands: CommandsConfig {
                implement: "/implement-spec".to_string(),
                lint: "/lint".to_string(),
                test: "/test".to_string(),
                review: "/review".to_string(),
            },
            templates: TemplatesConfig {
                implement: String::new(),
                lint: String::new(),
                test: String::new(),
                review: String::new(),
            },
        }
    }
}

fn load_config() -> Result<Config> {
    let config_path = PathBuf::from("mmm.toml");
    if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;
        toml::from_str(&content).context("Failed to parse config file")
    } else {
        Ok(Config::default())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let _config = load_config()?;

    tracing_subscriber::fmt()
        .with_env_filter(if cli.verbose { "debug" } else { "info" })
        .init();

    match &cli.command {
        Some(Commands::Run { spec, command }) => {
            run_loop(&cli, spec.as_deref(), command.as_deref())?
        }
        Some(Commands::List) => list_specs(&cli.specs_dir)?,
        Some(Commands::Init { name }) => init_spec(&cli.specs_dir, name)?,
        Some(Commands::Add { name, generate }) => add_spec(&cli.specs_dir, name, *generate)?,
        None => run_loop(&cli, None, None)?,
    }

    Ok(())
}

fn run_loop(cli: &Cli, spec_filter: Option<&str>, custom_command: Option<&str>) -> Result<()> {
    info!("Starting Memento Mori self-sufficient loop");

    let specs = load_specs(&cli.specs_dir, spec_filter)?;
    if specs.is_empty() {
        warn!("No specifications found in {:?}", cli.specs_dir);
        return Ok(());
    }

    info!("Found {} specifications", specs.len());

    let mut state = LoopState {
        iteration: 0,
        specs,
        completed: Vec::new(),
        in_progress: None,
    };

    while state.iteration < cli.max_iterations && state.completed.len() < state.specs.len() {
        state.iteration += 1;
        info!(
            "Starting iteration {}/{}",
            state.iteration, cli.max_iterations
        );

        if let Some(spec_idx) = get_next_spec_index(&state) {
            let spec = &state.specs[spec_idx];
            let spec_name = spec.name.clone();
            state.in_progress = Some(spec_name.clone());
            info!("Processing spec: {}", spec_name);

            match process_spec(spec, &state, custom_command) {
                Ok(true) => {
                    info!("Spec {} completed successfully", spec.name);
                    state.completed.push(spec.name.clone());
                    state.in_progress = None;
                }
                Ok(false) => {
                    info!("Spec {} needs more iterations", spec.name);
                }
                Err(e) => {
                    error!("Error processing spec {}: {}", spec.name, e);
                    state.in_progress = None;
                }
            }
        } else {
            info!("All specs completed!");
            break;
        }

        save_state(&state)?;
    }

    print_summary(&state);
    Ok(())
}

fn load_specs(specs_dir: &Path, filter: Option<&str>) -> Result<Vec<Spec>> {
    let mut specs = Vec::new();

    for entry in WalkDir::new(specs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("md")
            && !path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("README")
        {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            if let Some(filter) = filter {
                if !name.contains(filter) {
                    continue;
                }
            }

            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read spec file: {path:?}"))?;

            specs.push(Spec {
                name,
                path: path.to_path_buf(),
                content,
            });
        }
    }

    specs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(specs)
}

fn get_next_spec_index(state: &LoopState) -> Option<usize> {
    state
        .specs
        .iter()
        .position(|spec| !state.completed.contains(&spec.name))
}

fn process_spec(spec: &Spec, state: &LoopState, custom_command: Option<&str>) -> Result<bool> {
    let (command, prompt) = if let Some(cmd) = custom_command {
        // Use custom command (e.g., /implement-spec, /lint)
        (
            cmd.to_string(),
            format!("Spec: {}\n\nContent:\n{}", spec.name, spec.content),
        )
    } else {
        // Default behavior
        (
            String::new(),
            format!(
                "I'm implementing a self-sufficient loop. Here's the current state:\n\
                - Iteration: {}/{}\n\
                - Completed specs: {:?}\n\
                - Current spec: {}\n\n\
                Specification content:\n{}\n\n\
                Please analyze this specification and implement or improve the solution. \
                If the specification is fully implemented, respond with 'COMPLETED: <summary>'. \
                Otherwise, make progress on the implementation and explain what still needs to be done.",
                state.iteration,
                state.specs.len(),
                state.completed,
                spec.name,
                spec.content
            )
        )
    };

    let mut cmd = Command::new("claude");
    cmd.arg("--no-preamble");

    if !command.is_empty() {
        cmd.arg(command);
    }

    cmd.arg(prompt);

    let output = cmd.output().context("Failed to execute claude command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Claude command failed: {}", stderr));
    }

    let response = String::from_utf8_lossy(&output.stdout);
    info!(
        "Claude response preview: {}",
        response.lines().next().unwrap_or("")
    );

    Ok(response.contains("COMPLETED:"))
}

fn save_state(state: &LoopState) -> Result<()> {
    let state_file = PathBuf::from(".mmm-state.json");
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(&state_file, json)
        .with_context(|| format!("Failed to save state to {state_file:?}"))?;
    Ok(())
}

fn list_specs(specs_dir: &Path) -> Result<()> {
    let specs = load_specs(specs_dir, None)?;

    if specs.is_empty() {
        println!("No specifications found in {specs_dir:?}");
    } else {
        println!("Available specifications:");
        for spec in specs {
            println!("  - {}", spec.name);
        }
    }

    Ok(())
}

fn init_spec(specs_dir: &Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(specs_dir)?;

    let spec_path = specs_dir.join(format!("{name}.md"));
    if spec_path.exists() {
        return Err(anyhow::anyhow!("Spec '{}' already exists", name));
    }

    let template = format!(
        "# Feature: {name}\n\n\
        ## Objective\n\
        Brief description of what needs to be implemented.\n\n\
        ## Acceptance Criteria\n\
        - [ ] Criterion 1\n\
        - [ ] Criterion 2\n\
        - [ ] Criterion 3\n\n\
        ## Technical Details\n\
        Any specific technical requirements or constraints.\n"
    );

    std::fs::write(&spec_path, template)
        .with_context(|| format!("Failed to create spec file: {spec_path:?}"))?;

    println!("Created new specification: {spec_path:?}");
    Ok(())
}

fn add_spec(specs_dir: &Path, name: &str, generate: bool) -> Result<()> {
    std::fs::create_dir_all(specs_dir)?;

    let spec_path = specs_dir.join(format!("{name}.md"));
    if spec_path.exists() {
        return Err(anyhow::anyhow!("Spec '{}' already exists", name));
    }

    let content = if generate {
        // Use Claude to generate the spec
        info!("Generating spec with Claude...");

        let prompt = format!(
            "Create a detailed specification for a feature called '{name}'. \
            Include:\n\
            1. A clear objective\n\
            2. Specific acceptance criteria (as checkboxes)\n\
            3. Technical implementation details\n\
            4. Any constraints or considerations\n\n\
            Format it as a markdown specification document."
        );

        let output = Command::new("claude")
            .arg("--no-preamble")
            .arg(prompt)
            .output()
            .context("Failed to execute claude command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Claude command failed: {}", stderr));
        }

        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        // Prompt user for spec content
        println!("Enter the specification content (press Ctrl+D when done):");
        let mut content = String::new();
        std::io::stdin()
            .read_to_string(&mut content)
            .context("Failed to read from stdin")?;
        content
    };

    std::fs::write(&spec_path, content)
        .with_context(|| format!("Failed to create spec file: {spec_path:?}"))?;

    println!("Created new specification: {spec_path:?}");
    Ok(())
}

fn print_summary(state: &LoopState) {
    println!("\n=== Loop Summary ===");
    println!("Total iterations: {}", state.iteration);
    println!(
        "Completed specs: {}/{}",
        state.completed.len(),
        state.specs.len()
    );

    if !state.completed.is_empty() {
        println!("\nCompleted:");
        for spec in &state.completed {
            println!("  ✓ {spec}");
        }
    }

    let remaining: Vec<&String> = state
        .specs
        .iter()
        .map(|s| &s.name)
        .filter(|name| !state.completed.contains(name))
        .collect();

    if !remaining.is_empty() {
        println!("\nRemaining:");
        for spec in remaining {
            println!("  ○ {spec}");
        }
    }
}
