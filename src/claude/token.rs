//! Token tracking and optimization

use crate::error::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Token usage record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub timestamp: DateTime<Utc>,
    pub project: String,
    pub command: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub model: String,
}

/// Token tracker for usage limits and optimization
pub struct TokenTracker {
    daily_limit: Option<usize>,
    project_limits: HashMap<String, usize>,
    usage_file: PathBuf,
    current_usage: Vec<TokenUsage>,
}

impl TokenTracker {
    /// Create a new token tracker
    pub fn new(daily_limit: Option<usize>) -> Result<Self> {
        let usage_file = PathBuf::from(".mmm/token_usage.json");
        let current_usage = Self::load_usage(&usage_file)?;

        Ok(Self {
            daily_limit,
            project_limits: HashMap::new(),
            usage_file,
            current_usage,
        })
    }

    /// Set project-specific token limit
    pub fn set_project_limit(&mut self, project: String, limit: usize) {
        self.project_limits.insert(project, limit);
    }

    /// Check if we can proceed with estimated tokens
    pub fn can_proceed(&self, estimated_tokens: usize) -> Result<bool> {
        // Check daily limit
        if let Some(limit) = self.daily_limit {
            let today_usage = self.get_today_usage();
            if today_usage + estimated_tokens > limit {
                return Err(Error::Validation(format!(
                    "Daily token limit exceeded. Used: {today_usage}, Limit: {limit}, Requested: {estimated_tokens}"
                )));
            }

            // Warn at 80%
            if today_usage + estimated_tokens > limit * 80 / 100 {
                eprintln!(
                    "Warning: Approaching daily token limit ({}% used)",
                    (today_usage + estimated_tokens) * 100 / limit
                );
            }
        }

        Ok(true)
    }

    /// Record token usage
    pub fn record_usage(&mut self, tokens_used: usize) -> Result<()> {
        let usage = TokenUsage {
            timestamp: Utc::now(),
            project: "current".to_string(), // TODO: Get from context
            command: "unknown".to_string(), // TODO: Get from context
            input_tokens: tokens_used / 2,  // Rough estimate
            output_tokens: tokens_used / 2,
            total_tokens: tokens_used,
            model: "claude-3-sonnet".to_string(), // TODO: Get actual model
        };

        self.current_usage.push(usage);
        self.save_usage()?;

        Ok(())
    }

    /// Get today's total usage
    fn get_today_usage(&self) -> usize {
        let today = Utc::now().date_naive();
        self.current_usage
            .iter()
            .filter(|u| u.timestamp.date_naive() == today)
            .map(|u| u.total_tokens)
            .sum()
    }

    /// Get usage statistics
    pub fn get_stats(&self) -> TokenStats {
        let _today = Utc::now().date_naive();
        let week_ago = Utc::now() - Duration::days(7);

        let today_usage = self.get_today_usage();
        let week_usage: usize = self
            .current_usage
            .iter()
            .filter(|u| u.timestamp > week_ago)
            .map(|u| u.total_tokens)
            .sum();

        let by_project: HashMap<String, usize> = self
            .current_usage
            .iter()
            .filter(|u| u.timestamp > week_ago)
            .fold(HashMap::new(), |mut acc, u| {
                *acc.entry(u.project.clone()).or_insert(0) += u.total_tokens;
                acc
            });

        TokenStats {
            today_usage,
            week_usage,
            by_project,
            daily_limit: self.daily_limit,
        }
    }

    /// Optimize a prompt to reduce tokens
    pub fn optimize_prompt(&self, prompt: &str) -> String {
        let mut optimized = prompt.to_string();

        // Remove excessive whitespace
        optimized = optimized.split_whitespace().collect::<Vec<_>>().join(" ");

        // Remove redundant newlines
        while optimized.contains("\n\n\n") {
            optimized = optimized.replace("\n\n\n", "\n\n");
        }

        // Compress repetitive patterns
        // TODO: More sophisticated compression

        optimized
    }

    /// Get current token usage
    pub async fn get_usage(&self) -> Result<crate::claude::TokenUsage> {
        let today = Utc::now().date_naive();
        let today_usage = self
            .current_usage
            .iter()
            .filter(|u| u.timestamp.date_naive() == today)
            .fold((0u64, 0u64, 0u64), |acc, u| {
                (
                    acc.0 + u.input_tokens as u64,
                    acc.1 + u.output_tokens as u64,
                    acc.2 + u.total_tokens as u64,
                )
            });

        Ok(crate::claude::TokenUsage {
            input_tokens: today_usage.0,
            output_tokens: today_usage.1,
            total_tokens: today_usage.2,
        })
    }

    /// Load usage from file
    fn load_usage(path: &PathBuf) -> Result<Vec<TokenUsage>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path).map_err(Error::Io)?;

        serde_json::from_str(&content).map_err(|e| Error::Parse(format!("Invalid usage JSON: {e}")))
    }

    /// Save usage to file
    fn save_usage(&self) -> Result<()> {
        // Create directory if needed
        if let Some(parent) = self.usage_file.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        let json = serde_json::to_string_pretty(&self.current_usage)
            .map_err(|e| Error::Parse(format!("Failed to serialize usage: {e}")))?;

        fs::write(&self.usage_file, json).map_err(Error::Io)?;

        Ok(())
    }

    /// Clean up old usage records
    pub fn cleanup_old_records(&mut self, days: i64) -> Result<()> {
        let cutoff = Utc::now() - Duration::days(days);
        self.current_usage.retain(|u| u.timestamp > cutoff);
        self.save_usage()?;
        Ok(())
    }
}

/// Token usage statistics
#[derive(Debug, Serialize)]
pub struct TokenStats {
    pub today_usage: usize,
    pub week_usage: usize,
    pub by_project: HashMap<String, usize>,
    pub daily_limit: Option<usize>,
}
