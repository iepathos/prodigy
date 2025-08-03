//! Unified analysis module for MMM
//!
//! Provides a single entry point for all analysis operations,
//! used by both command-line and workflow paths.

pub mod unified;

pub use unified::{
    run_analysis, AnalysisConfig, AnalysisConfigBuilder, AnalysisResults, AnalysisTiming,
    DefaultProgressReporter, Impact, ImprovementSuggestion, OutputFormat, Priority,
    ProgressReporter,
};
