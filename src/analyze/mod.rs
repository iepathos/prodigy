//! Analyze command for running project analysis and metrics

pub mod command;

use anyhow::Result;
use command::AnalyzeCommand;

/// Run the analyze command
pub async fn run(cmd: AnalyzeCommand) -> Result<()> {
    command::execute(cmd).await
}

#[cfg(test)]
mod tests;
