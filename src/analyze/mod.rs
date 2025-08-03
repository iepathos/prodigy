//! Project analysis and metrics collection module
//!
//! This module provides comprehensive project analysis capabilities including
//! code quality metrics, test coverage analysis, dependency mapping, and
//! technical debt assessment. It serves as the foundation for MMM's
//! context-aware improvement recommendations.
//!
//! # Key Features
//!
//! - **Multi-Modal Analysis**: Supports context, metrics, and combined analysis modes
//! - **Incremental Analysis**: Only re-analyzes changed files for performance
//! - **Coverage Integration**: Integrates with cargo-tarpaulin for accurate test coverage
//! - **Persistent Results**: Saves analysis results to `.mmm/context/` for reuse
//! - **Health Scoring**: Provides unified project health scores (0-100)
//!
//! # Analysis Types
//!
//! ## Context Analysis
//!
//! Analyzes project structure, dependencies, conventions, and technical debt:
//! - Module dependency graphs and circular dependency detection
//! - Architectural patterns and violations
//! - Code conventions and naming patterns  
//! - Technical debt items with impact scoring
//!
//! ## Metrics Analysis
//!
//! Collects quantitative metrics about code quality:
//! - Test coverage percentages
//! - Lint warning counts
//! - Code complexity metrics
//! - Performance benchmarks
//!
//! ## Combined Analysis
//!
//! Performs both context and metrics analysis for complete project assessment.
//!
//! # Examples
//!
//! ## Basic Analysis
//!
//! ```rust
//! use mmm::analyze::command::AnalyzeCommand;
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let cmd = AnalyzeCommand {
//!     analysis_type: "all".to_string(),
//!     output: "summary".to_string(),
//!     save: true,
//!     verbose: false,
//!     path: Some(PathBuf::from("/path/to/project")),
//!     run_coverage: false,
//!     no_commit: false,
//! };
//!
//! mmm::analyze::run(cmd).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Coverage-Enhanced Analysis
//!
//! ```rust
//! # use mmm::analyze::command::AnalyzeCommand;
//! # use std::path::PathBuf;
//! # async fn example() -> anyhow::Result<()> {
//! let cmd = AnalyzeCommand {
//!     analysis_type: "context".to_string(),
//!     output: "json".to_string(),
//!     save: true,
//!     verbose: true,
//!     path: None, // Use current directory
//!     run_coverage: true, // Run cargo-tarpaulin first
//!     no_commit: false,
//! };
//!
//! mmm::analyze::run(cmd).await?;
//! # Ok(())
//! # }
//! ```

pub mod command;

use anyhow::Result;
use command::AnalyzeCommand;

/// Run the analyze command
pub async fn run(cmd: AnalyzeCommand) -> Result<()> {
    command::execute(cmd).await
}

#[cfg(test)]
mod tests;
