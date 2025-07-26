use crate::config::workflow::Extractor;
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use tokio::process::Command;

/// Extract values from various sources based on configured extractors
pub struct ExtractorEngine {
    extractors: HashMap<String, Extractor>,
    values: HashMap<String, String>,
}

impl ExtractorEngine {
    pub fn new(extractors: HashMap<String, Extractor>) -> Self {
        Self {
            extractors,
            values: HashMap::new(),
        }
    }

    /// Extract all configured values
    pub async fn extract_all(&mut self, verbose: bool) -> Result<()> {
        for (key, extractor) in &self.extractors {
            if verbose {
                println!("Extracting value for {}...", key);
            }
            let value = self.extract_value(extractor).await?;
            if !value.is_empty() {
                self.values.insert(key.clone(), value);
                if verbose {
                    println!("  {} = {}", key, self.values[key]);
                }
            }
        }
        Ok(())
    }

    /// Get extracted values
    pub fn get_values(&self) -> &HashMap<String, String> {
        &self.values
    }

    /// Extract a single value based on the extractor type
    async fn extract_value(&self, extractor: &Extractor) -> Result<String> {
        match extractor {
            Extractor::Git { pattern } => self.extract_from_git(pattern).await,
            Extractor::File { path, pattern } => self.extract_from_file(path, pattern).await,
            Extractor::Output { pattern: _ } => {
                // For output extraction, we need the previous command's output
                // This will be handled differently in the workflow execution
                Ok(String::new())
            }
        }
    }

    /// Extract value from git log
    async fn extract_from_git(&self, pattern: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=format:%s"])
            .output()
            .await
            .context("Failed to get git log")?;

        let commit_message = String::from_utf8_lossy(&output.stdout);
        self.extract_with_regex(&commit_message, pattern)
    }

    /// Extract value from file
    async fn extract_from_file(&self, path: &str, pattern: &str) -> Result<String> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context(format!("Failed to read file: {}", path))?;
        self.extract_with_regex(&content, pattern)
    }

    /// Extract value using regex pattern
    fn extract_with_regex(&self, text: &str, pattern: &str) -> Result<String> {
        let re = Regex::new(pattern).context("Invalid regex pattern")?;

        if let Some(captures) = re.captures(text) {
            // If there's a capture group, use it
            if captures.len() > 1 {
                Ok(captures
                    .get(1)
                    .map(|m| m.as_str())
                    .unwrap_or("")
                    .to_string())
            } else {
                // Otherwise, use the whole match
                Ok(captures
                    .get(0)
                    .map(|m| m.as_str())
                    .unwrap_or("")
                    .to_string())
            }
        } else {
            Ok(String::new())
        }
    }

    /// Update value (for output extraction after command execution)
    pub fn update_value(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }
}
