//! Learning manager for tracking improvement patterns

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use super::types::{Improvement, Learning, PatternInfo};

/// Manages learning data about successful improvements
pub struct LearningManager {
    learning: Learning,
    path: PathBuf,
}

impl LearningManager {
    /// Load learning data from disk
    pub fn load() -> Result<Self> {
        let path = PathBuf::from(".mmm/learning.json");
        let learning = if path.exists() {
            let contents = fs::read_to_string(&path).context("Failed to read learning file")?;
            serde_json::from_str(&contents).context("Failed to deserialize learning data")?
        } else {
            Learning::default()
        };

        Ok(Self { learning, path })
    }

    /// Load from custom path
    pub fn load_from(path: PathBuf) -> Result<Self> {
        let learning = if path.exists() {
            let contents = fs::read_to_string(&path).context("Failed to read learning file")?;
            serde_json::from_str(&contents).context("Failed to deserialize learning data")?
        } else {
            Learning::default()
        };

        Ok(Self { learning, path })
    }

    /// Save learning data to disk
    pub fn save(&self) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).context("Failed to create directory for learning file")?;
        }

        // Write atomically
        let temp_path = self.path.with_extension("tmp");
        let json = serde_json::to_string_pretty(&self.learning)
            .context("Failed to serialize learning data")?;
        fs::write(&temp_path, json).context("Failed to write learning file")?;
        fs::rename(temp_path, &self.path).context("Failed to rename learning file")?;

        Ok(())
    }

    /// Record a successful improvement
    pub fn record_improvement(&mut self, improvement: &Improvement) -> Result<()> {
        // Update pattern statistics
        let pattern = self
            .learning
            .patterns
            .successful_improvements
            .entry(improvement.improvement_type.clone())
            .or_insert_with(PatternInfo::default);

        pattern.total_attempts += 1;
        pattern.successful += 1;
        pattern.success_rate = pattern.successful as f32 / pattern.total_attempts as f32;
        pattern.impacts.push(improvement.impact);

        // Update average impact
        let sum: f32 = pattern.impacts.iter().sum();
        pattern.average_impact = sum / pattern.impacts.len() as f32;

        // Add example if not already present
        if !pattern.examples.contains(&improvement.description) && pattern.examples.len() < 10 {
            pattern.examples.push(improvement.description.clone());
        }

        self.save()?;
        Ok(())
    }

    /// Record a failed improvement attempt
    pub fn record_failure(&mut self, improvement_type: &str) -> Result<()> {
        let pattern = self
            .learning
            .patterns
            .successful_improvements
            .entry(improvement_type.to_string())
            .or_insert_with(PatternInfo::default);

        pattern.total_attempts += 1;
        pattern.success_rate = pattern.successful as f32 / pattern.total_attempts as f32;

        // If success rate drops too low, mark as failed pattern
        if pattern.success_rate < 0.3 && pattern.total_attempts >= 5 {
            self.learning.patterns.failed_patterns.insert(
                improvement_type.to_string(),
                super::types::FailureInfo {
                    failure_rate: 1.0 - pattern.success_rate,
                    avoid: true,
                },
            );
        }

        self.save()?;
        Ok(())
    }

    /// Get suggested improvements based on past success
    pub fn suggest_improvements(&self, limit: usize) -> Vec<(String, f32)> {
        let mut suggestions: Vec<_> = self
            .learning
            .patterns
            .successful_improvements
            .iter()
            .filter(|(name, _)| {
                // Filter out patterns marked as failed
                !self
                    .learning
                    .patterns
                    .failed_patterns
                    .get(*name)
                    .map(|f| f.avoid)
                    .unwrap_or(false)
            })
            .map(|(name, stats)| {
                // Score based on success rate * average impact
                let score = stats.success_rate * stats.average_impact;
                (name.clone(), score)
            })
            .collect();

        // Sort by score descending
        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top suggestions
        suggestions.truncate(limit);
        suggestions
    }

    /// Get patterns to avoid
    pub fn patterns_to_avoid(&self) -> Vec<String> {
        self.learning
            .patterns
            .failed_patterns
            .iter()
            .filter(|(_, info)| info.avoid)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Add a focus area preference
    pub fn add_focus_area(&mut self, area: &str) -> Result<()> {
        if !self
            .learning
            .preferences
            .focus_areas
            .contains(&area.to_string())
        {
            self.learning.preferences.focus_areas.push(area.to_string());
            self.save()?;
        }
        Ok(())
    }

    /// Remove a focus area preference
    pub fn remove_focus_area(&mut self, area: &str) -> Result<()> {
        self.learning.preferences.focus_areas.retain(|a| a != area);
        self.save()?;
        Ok(())
    }

    /// Add a skip pattern
    pub fn add_skip_pattern(&mut self, pattern: &str) -> Result<()> {
        if !self
            .learning
            .preferences
            .skip_patterns
            .contains(&pattern.to_string())
        {
            self.learning
                .preferences
                .skip_patterns
                .push(pattern.to_string());
            self.save()?;
        }
        Ok(())
    }

    /// Get current preferences
    pub fn preferences(&self) -> &super::types::Preferences {
        &self.learning.preferences
    }

    /// Get pattern statistics
    pub fn get_pattern_stats(&self, pattern: &str) -> Option<&PatternInfo> {
        self.learning.patterns.successful_improvements.get(pattern)
    }

    /// Reset learning data
    pub fn reset(&mut self) -> Result<()> {
        self.learning = Learning::default();
        self.save()?;
        Ok(())
    }

    /// Get summary statistics
    pub fn summary(&self) -> LearningSummary {
        let total_patterns = self.learning.patterns.successful_improvements.len();
        let successful_patterns = self
            .learning
            .patterns
            .successful_improvements
            .values()
            .filter(|p| p.success_rate > 0.7)
            .count();
        let total_attempts: u32 = self
            .learning
            .patterns
            .successful_improvements
            .values()
            .map(|p| p.total_attempts)
            .sum();
        let total_successes: u32 = self
            .learning
            .patterns
            .successful_improvements
            .values()
            .map(|p| p.successful)
            .sum();

        LearningSummary {
            total_patterns,
            successful_patterns,
            failed_patterns: self.learning.patterns.failed_patterns.len(),
            total_attempts,
            total_successes,
            overall_success_rate: if total_attempts > 0 {
                total_successes as f32 / total_attempts as f32
            } else {
                0.0
            },
        }
    }
}

/// Summary of learning statistics
#[derive(Debug, Clone)]
pub struct LearningSummary {
    pub total_patterns: usize,
    pub successful_patterns: usize,
    pub failed_patterns: usize,
    pub total_attempts: u32,
    pub total_successes: u32,
    pub overall_success_rate: f32,
}
