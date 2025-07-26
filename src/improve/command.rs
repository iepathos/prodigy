use clap::Args;

#[derive(Debug, Args)]
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Show detailed progress
    #[arg(long)]
    pub show_progress: bool,
}
