use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct InitCommand {
    /// Force overwrite existing commands
    #[arg(short, long)]
    pub force: bool,

    /// Specific commands to install (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub commands: Option<Vec<String>>,

    /// Directory to initialize (defaults to current)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}
