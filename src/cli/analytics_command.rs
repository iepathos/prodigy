//! CLI commands for Claude session analytics

use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Subcommand;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

use crate::analytics::{AnalyticsEngine, SessionReplay, SessionWatcher, TimeRange};
use crate::cook::execution::events::EventLogger;

#[derive(Debug, Subcommand)]
pub enum AnalyticsCommand {
    /// Watch Claude sessions for real-time analytics
    Watch {
        /// Path to Claude projects directory (defaults to ~/.claude/projects)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Calculate cost for a specific session
    Cost {
        /// Session ID to analyze
        session_id: String,
    },
    /// Analyze tool usage patterns
    Tools {
        /// Number of days to analyze (default: 7)
        #[arg(short, long, default_value = "7")]
        days: i64,
    },
    /// Identify performance bottlenecks
    Bottlenecks {
        /// Threshold in milliseconds for slow tools (default: 5000)
        #[arg(long, default_value = "5000")]
        threshold: u64,
    },
    /// Get cost projections
    Project {
        /// Number of days to base projection on (default: 30)
        #[arg(short, long, default_value = "30")]
        days: i64,
    },
    /// Get optimization recommendations
    Optimize,
    /// Replay a session
    Replay {
        /// Session ID to replay
        session_id: String,
        /// Starting position
        #[arg(long)]
        start: Option<usize>,
        /// Ending position
        #[arg(long)]
        end: Option<usize>,
    },
    /// Get session summary
    Summary {
        /// Session ID
        session_id: String,
    },
    /// Export session transcript
    Export {
        /// Session ID
        session_id: String,
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

pub async fn handle_analytics_command(cmd: AnalyticsCommand) -> Result<()> {
    match cmd {
        AnalyticsCommand::Watch { path } => watch_sessions(path).await,
        AnalyticsCommand::Cost { session_id } => calculate_cost(&session_id).await,
        AnalyticsCommand::Tools { days } => analyze_tools(days).await,
        AnalyticsCommand::Bottlenecks { threshold } => identify_bottlenecks(threshold).await,
        AnalyticsCommand::Project { days } => project_costs(days).await,
        AnalyticsCommand::Optimize => get_recommendations().await,
        AnalyticsCommand::Replay {
            session_id,
            start,
            end,
        } => replay_session(&session_id, start, end).await,
        AnalyticsCommand::Summary { session_id } => get_session_summary(&session_id).await,
        AnalyticsCommand::Export { session_id, output } => {
            export_transcript(&session_id, output).await
        }
    }
}

async fn watch_sessions(_path: Option<PathBuf>) -> Result<()> {
    info!("Starting Claude session watcher...");

    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    // Note: For now, we use default path. Custom paths would need SessionWatcher modification
    let watcher = SessionWatcher::new(event_logger)?;

    // Start watching in background
    let watch_handle = tokio::spawn(async move {
        if let Err(e) = watcher.watch().await {
            error!("Watch error: {}", e);
        }
    });

    info!("Session watcher running. Press Ctrl+C to stop.");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Stopping session watcher...");

    watch_handle.abort();
    Ok(())
}

async fn calculate_cost(session_id: &str) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    // Scan existing files to populate index
    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let engine = AnalyticsEngine::new(watcher.index());
    let cost = engine.calculate_session_cost(session_id).await?;

    println!("Session: {}", session_id);
    println!("Input tokens: {}", cost.input_tokens);
    println!("Output tokens: {}", cost.output_tokens);
    println!("Cache tokens: {}", cost.cache_tokens);
    println!("Estimated cost: ${:.4} USD", cost.estimated_cost_usd);

    Ok(())
}

async fn analyze_tools(days: i64) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let engine = AnalyticsEngine::new(watcher.index());
    let time_range = TimeRange {
        start: Utc::now() - Duration::days(days),
        end: Utc::now(),
    };

    let stats = engine.analyze_tool_usage(time_range).await?;

    println!("Tool Usage Analysis (last {} days):", days);
    println!(
        "{:<30} {:<10} {:<15} {:<15} {:<10}",
        "Tool", "Count", "Avg Time (ms)", "Total Time (ms)", "Success %"
    );
    println!("{}", "-".repeat(80));

    for (name, stat) in stats.stats {
        println!(
            "{:<30} {:<10} {:<15} {:<15} {:<10.1}",
            name,
            stat.total_invocations,
            stat.average_duration_ms,
            stat.total_duration_ms,
            stat.success_rate
        );
    }

    Ok(())
}

async fn identify_bottlenecks(threshold_ms: u64) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let engine = AnalyticsEngine::new(watcher.index());
    let issues = engine.identify_bottlenecks(threshold_ms).await?;

    if issues.is_empty() {
        println!("No performance bottlenecks detected.");
    } else {
        println!("Performance Bottlenecks:");
        println!();

        for issue in issues {
            println!("Tool: {}", issue.tool_name);
            println!("Issue: {:?}", issue.issue_type);
            println!("Average Duration: {}ms", issue.average_duration_ms);
            println!("Occurrences: {}", issue.occurrence_count);
            println!("Recommendation: {}", issue.recommendation);
            println!();
        }
    }

    Ok(())
}

async fn project_costs(days: i64) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let engine = AnalyticsEngine::new(watcher.index());
    let projection = engine.project_costs(days).await?;

    println!("Cost Projection (based on last {} days):", days);
    println!();
    println!("Daily average: ${:.2}", projection.daily_average);
    println!("Weekly projection: ${:.2}", projection.weekly_projection);
    println!("Monthly projection: ${:.2}", projection.monthly_projection);
    println!("Annual projection: ${:.2}", projection.annual_projection);
    println!();
    println!("Average Daily Token Usage:");
    println!("  Input: {} tokens", projection.average_daily_tokens.input);
    println!(
        "  Output: {} tokens",
        projection.average_daily_tokens.output
    );
    println!("  Cache: {} tokens", projection.average_daily_tokens.cache);

    Ok(())
}

async fn get_recommendations() -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let engine = AnalyticsEngine::new(watcher.index());
    let recommendations = engine.get_optimization_recommendations().await?;

    if recommendations.is_empty() {
        println!("No optimization recommendations at this time.");
    } else {
        println!("Optimization Recommendations:");
        println!();

        for rec in recommendations {
            let priority_str = match rec.priority {
                crate::analytics::engine::Priority::High => "HIGH",
                crate::analytics::engine::Priority::Medium => "MEDIUM",
                crate::analytics::engine::Priority::Low => "LOW",
            };

            println!("[{}] {}", priority_str, rec.title);
            println!("Category: {:?}", rec.category);
            println!("{}", rec.description);
            if let Some(savings) = rec.estimated_savings {
                println!("Estimated savings: ${:.2}/month", savings);
            }
            println!();
        }
    }

    Ok(())
}

async fn replay_session(session_id: &str, start: Option<usize>, end: Option<usize>) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let index = watcher.index();
    let index_guard = index.read().await;
    let session = index_guard.get_session(session_id).await?;

    let mut replay = SessionReplay::new(session.clone());

    if let Some(start_pos) = start {
        replay.jump_to_position(start_pos);
    }

    let events = if let Some(end_pos) = end {
        replay.play_range(start.unwrap_or(0), end_pos).await?
    } else {
        replay.play().await?
    };

    for event in events {
        println!(
            "[{}] {}: {}",
            event.timestamp.format("%H:%M:%S"),
            event.event_type,
            serde_json::to_string(&event.content)?
        );
    }

    Ok(())
}

async fn get_session_summary(session_id: &str) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let index = watcher.index();
    let index_guard = index.read().await;
    let session = index_guard.get_session(session_id).await?;

    let replay = SessionReplay::new(session.clone());
    let summary = replay.get_summary();

    println!("Session Summary");
    println!("ID: {}", summary.session_id);
    println!("Total Events: {}", summary.total_events);
    println!("Tool Invocations: {}", summary.tool_invocations);
    println!("Errors: {}", summary.errors);
    println!("Duration: {} seconds", summary.duration_seconds);
    println!("Tokens:");
    println!("  Input: {}", summary.input_tokens);
    println!("  Output: {}", summary.output_tokens);
    println!("  Cache: {}", summary.cache_tokens);

    Ok(())
}

async fn export_transcript(session_id: &str, output: Option<PathBuf>) -> Result<()> {
    let event_logger = Arc::new(EventLogger::new(Vec::new()));
    let watcher = SessionWatcher::new(event_logger)?;

    info!("Scanning existing sessions...");
    watcher.scan_existing_files().await?;

    let index = watcher.index();
    let index_guard = index.read().await;
    let session = index_guard.get_session(session_id).await?;

    let replay = SessionReplay::new(session.clone());
    let transcript = replay.export_transcript();

    if let Some(output_path) = output {
        tokio::fs::write(&output_path, &transcript).await?;
        println!("Transcript exported to: {}", output_path.display());
    } else {
        println!("{}", transcript);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_command_parsing() {
        // Test Watch command
        let watch_cmd = AnalyticsCommand::Watch {
            path: Some(PathBuf::from("/test/path")),
        };
        match watch_cmd {
            AnalyticsCommand::Watch { path } => {
                assert_eq!(path, Some(PathBuf::from("/test/path")));
            }
            _ => panic!("Expected Watch command"),
        }

        // Test Cost command
        let cost_cmd = AnalyticsCommand::Cost {
            session_id: "session-123".to_string(),
        };
        match cost_cmd {
            AnalyticsCommand::Cost { session_id } => {
                assert_eq!(session_id, "session-123");
            }
            _ => panic!("Expected Cost command"),
        }

        // Test Tools command
        let tools_cmd = AnalyticsCommand::Tools { days: 14 };
        match tools_cmd {
            AnalyticsCommand::Tools { days } => {
                assert_eq!(days, 14);
            }
            _ => panic!("Expected Tools command"),
        }

        // Test Bottlenecks command
        let bottlenecks_cmd = AnalyticsCommand::Bottlenecks { threshold: 3000 };
        match bottlenecks_cmd {
            AnalyticsCommand::Bottlenecks { threshold } => {
                assert_eq!(threshold, 3000);
            }
            _ => panic!("Expected Bottlenecks command"),
        }

        // Test Optimize command
        let optimize_cmd = AnalyticsCommand::Optimize;
        match optimize_cmd {
            AnalyticsCommand::Optimize => {}
            _ => panic!("Expected Optimize command"),
        }
    }

    #[test]
    fn test_replay_command_optional_fields() {
        let replay_cmd = AnalyticsCommand::Replay {
            session_id: "test-session".to_string(),
            start: None,
            end: None,
        };
        match replay_cmd {
            AnalyticsCommand::Replay {
                session_id,
                start,
                end,
            } => {
                assert_eq!(session_id, "test-session");
                assert_eq!(start, None);
                assert_eq!(end, None);
            }
            _ => panic!("Expected Replay command"),
        }
    }

    #[test]
    fn test_export_command_formatting() {
        let export_cmd = AnalyticsCommand::Export {
            session_id: "export-test".to_string(),
            output: None,
        };
        match export_cmd {
            AnalyticsCommand::Export { session_id, output } => {
                assert_eq!(session_id, "export-test");
                assert_eq!(output, None);
            }
            _ => panic!("Expected Export command"),
        }
    }

    #[tokio::test]
    async fn test_handle_analytics_invalid_session() {
        // Test with non-existent session for Cost command
        let cmd = AnalyticsCommand::Cost {
            session_id: "non-existent-session".to_string(),
        };

        let result = handle_analytics_command(cmd).await;
        // Should fail gracefully
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_analytics_summary_invalid() {
        // Test with non-existent session for Summary command
        let cmd = AnalyticsCommand::Summary {
            session_id: "invalid-session".to_string(),
        };

        let result = handle_analytics_command(cmd).await;
        // Should fail gracefully
        assert!(result.is_err());
    }
}
