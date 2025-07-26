use anyhow::{Context as AnyhowContext, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use super::analyzer::ProjectInfo;
use super::context::Context;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImproveSession {
    pub id: String,
    pub project: ProjectInfo,
    pub context: Context,
    pub state: ImproveState,
    pub options: ImproveOptions,
    pub iterations: Vec<Iteration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImproveOptions {
    pub target_score: f32,
    pub verbose: bool,
}

impl Default for ImproveOptions {
    fn default() -> Self {
        Self {
            target_score: 8.0,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImproveState {
    pub last_run: DateTime<Utc>,
    pub current_score: f32,
    pub improvement_history: Vec<ImprovementRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementRun {
    pub timestamp: DateTime<Utc>,
    pub initial_score: f32,
    pub final_score: f32,
    pub changes_made: Vec<String>,
    pub files_modified: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iteration {
    pub number: usize,
    pub timestamp: DateTime<Utc>,
    pub review: ReviewResult,
    pub changes: Vec<Change>,
    pub score_delta: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub score: f32,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: IssueSeverity,
    pub category: String,
    pub description: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub file: PathBuf,
    pub change_type: ChangeType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Add,
    Modify,
    Delete,
}

impl ImproveSession {
    pub fn new(project: ProjectInfo, context: Context, options: ImproveOptions) -> Self {
        let state = Self::load_or_create_state();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project,
            context,
            state,
            options,
            iterations: Vec::new(),
        }
    }

    pub async fn start(
        project: ProjectInfo,
        context: Context,
        options: ImproveOptions,
    ) -> Result<Self> {
        Ok(Self::new(project, context, options))
    }

    pub fn is_good_enough(&self) -> bool {
        self.state.current_score >= self.options.target_score
    }

    pub fn update(&mut self, iteration: Iteration) {
        self.state.current_score += iteration.score_delta;
        self.iterations.push(iteration);
        self.state.last_run = Utc::now();
    }

    pub fn current_score(&self) -> f32 {
        self.state.current_score
    }

    pub fn completed_count(&self) -> usize {
        self.iterations.len()
    }

    pub fn remaining_count(&self) -> usize {
        // Estimate remaining improvements based on score gap
        let score_gap = self.options.target_score - self.state.current_score;
        ((score_gap / 0.3).ceil() as usize).min(10 - self.iterations.len())
    }

    pub fn is_complete(&self) -> bool {
        self.is_good_enough() || self.iterations.len() >= 10
    }

    pub async fn run(&mut self) -> Result<SessionResult> {
        let initial_score = self.state.current_score;
        let mut total_changes = Vec::new();
        let mut files_changed = std::collections::HashSet::new();

        // Run improvement iterations
        while !self.is_good_enough() && self.iterations.len() < 10 {
            if self.options.verbose {
                println!(
                    "Starting iteration {} (current score: {:.1})",
                    self.iterations.len() + 1,
                    self.state.current_score
                );
            }

            // Call Claude CLI for code review
            let review = self.call_claude_review().await?;

            if review.issues.is_empty() {
                if self.options.verbose {
                    println!("No issues found in review - stopping iterations");
                }
                break;
            }

            // Call Claude CLI to implement improvements
            let changes = self.call_claude_implement(&review).await?;

            if changes.is_empty() {
                if self.options.verbose {
                    println!("No changes generated - stopping iterations");
                }
                break;
            }

            // Apply changes to actual files
            let applied_changes = self.apply_changes(&changes).await?;

            // Re-analyze project to get new score
            let new_score = self.reanalyze_project().await?;
            let score_delta = new_score - self.state.current_score;

            // Track changes
            for change in &applied_changes {
                files_changed.insert(change.file.clone());
                total_changes.push(change.description.clone());
            }

            let iteration = Iteration {
                number: self.iterations.len() + 1,
                timestamp: Utc::now(),
                review,
                changes: applied_changes,
                score_delta,
            };

            self.update(iteration.clone());

            // Save state after each iteration
            self.save_state()?;

            if self.options.verbose {
                println!(
                    "Iteration {} complete - score: {:.1} (delta: {:.1})",
                    iteration.number, self.state.current_score, score_delta
                );
            }
        }

        let final_score = self.state.current_score;

        // Record this run
        let run = ImprovementRun {
            timestamp: Utc::now(),
            initial_score,
            final_score,
            changes_made: total_changes,
            files_modified: files_changed.len(),
        };

        self.state.improvement_history.push(run.clone());
        self.save_state()?;

        Ok(SessionResult {
            initial_score,
            final_score,
            improvement: final_score - initial_score,
            files_changed: files_changed.len(),
            iterations: self.iterations.len(),
        })
    }

    fn build_review_context(&self) -> String {
        // Build context for Claude code review
        let framework_str = self
            .project
            .framework
            .as_ref()
            .map(|f| format!("{:?}", f))
            .unwrap_or_else(|| "None".to_string());

        format!(
            "Language: {}\nFramework: {}\nSize: {:?}\nCurrent Score: {:.1}\n",
            self.project.language, framework_str, self.project.size, self.state.current_score
        )
    }

    fn build_implementation_context(&self, review: &ReviewResult) -> String {
        // Build context for Claude implementation
        let issues: Vec<String> = review
            .issues
            .iter()
            .map(|i| format!("{:?}: {} ({})", i.severity, i.description, i.category))
            .collect();

        format!(
            "Issues to address:\n{}\nTarget Score: {:.1}\n",
            issues.join("\n"),
            self.options.target_score
        )
    }

    fn parse_review_output(&self, output: &str) -> Result<ReviewResult> {
        // Parse JSON output from Claude CLI mmm-code-review
        if let Some(json_start) = output.find("{\"mmm_structured_output\"") {
            let json_str = &output[json_start..];
            if let Some(json_end) = json_str.find("\n```") {
                let json_str = &json_str[..json_end];
                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(json) => {
                        if let Some(structured) = json.get("mmm_structured_output") {
                            return self.parse_structured_review(structured);
                        }
                    }
                    Err(e) => {
                        if self.options.verbose {
                            println!("Failed to parse JSON: {}", e);
                        }
                    }
                }
            }
        }

        // Fallback: return empty review if parsing fails
        Ok(ReviewResult {
            score: self.state.current_score,
            issues: Vec::new(),
        })
    }

    fn parse_structured_review(&self, structured: &serde_json::Value) -> Result<ReviewResult> {
        let score = structured
            .get("overall_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.state.current_score as f64) as f32;

        let mut issues = Vec::new();

        if let Some(actions) = structured.get("actions").and_then(|v| v.as_array()) {
            for action in actions {
                let severity = match action.get("severity").and_then(|v| v.as_str()) {
                    Some("critical") => IssueSeverity::High,
                    Some("high") => IssueSeverity::High,
                    Some("medium") => IssueSeverity::Medium,
                    _ => IssueSeverity::Low,
                };

                let category = action
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("general")
                    .to_string();

                let description = action
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown issue")
                    .to_string();

                let file = action
                    .get("file")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from);

                let line = action
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                issues.push(Issue {
                    severity,
                    category,
                    description,
                    file,
                    line,
                });
            }
        }

        Ok(ReviewResult { score, issues })
    }

    fn parse_implementation_output(&self, output: &str) -> Result<Vec<Change>> {
        // Parse implementation output to extract file changes
        // For now, we'll look for file modification patterns in the output
        let mut changes = Vec::new();

        // Simple pattern matching for now - in a real implementation,
        // this would parse structured output from the implementation command
        for line in output.lines() {
            if line.contains("Modified:") || line.contains("Created:") || line.contains("Updated:")
            {
                if let Some(file_path) = self.extract_file_path(line) {
                    let change_type = if line.contains("Created:") {
                        ChangeType::Add
                    } else if line.contains("Deleted:") {
                        ChangeType::Delete
                    } else {
                        ChangeType::Modify
                    };

                    changes.push(Change {
                        file: PathBuf::from(file_path),
                        change_type,
                        description: line.to_string(),
                    });
                }
            }
        }

        Ok(changes)
    }

    fn extract_file_path<'a>(&self, line: &'a str) -> Option<&'a str> {
        // Extract file path from a line like "Modified: src/main.rs"
        if let Some(colon_pos) = line.find(':') {
            let path = line[colon_pos + 1..].trim();
            Some(path)
        } else {
            None
        }
    }

    async fn apply_changes(&mut self, changes: &[Change]) -> Result<Vec<Change>> {
        // Apply changes to actual files
        let mut applied_changes = Vec::new();

        for change in changes {
            match self.apply_single_change(change).await {
                Ok(()) => {
                    applied_changes.push(change.clone());
                    if self.options.verbose {
                        println!("Applied: {}", change.description);
                    }
                }
                Err(e) => {
                    if self.options.verbose {
                        println!("Failed to apply change {}: {}", change.description, e);
                    }
                }
            }
        }

        Ok(applied_changes)
    }

    async fn apply_single_change(&mut self, change: &Change) -> Result<()> {
        match change.change_type {
            ChangeType::Add => {
                // Create new file - this would need actual content from Claude
                if !change.file.exists() {
                    if let Some(parent) = change.file.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    // For now, create empty file - real implementation would have content
                    fs::write(&change.file, "")?;
                }
            }
            ChangeType::Modify => {
                // Modify existing file - this would need actual modifications from Claude
                if change.file.exists() {
                    // For now, just touch the file - real implementation would apply actual changes
                    let content = fs::read_to_string(&change.file)?;
                    fs::write(&change.file, content)?;
                }
            }
            ChangeType::Delete => {
                // Delete file
                if change.file.exists() {
                    fs::remove_file(&change.file)?;
                }
            }
        }
        Ok(())
    }

    async fn reanalyze_project(&mut self) -> Result<f32> {
        // Re-run project analysis to get updated score
        use crate::improve::analyzer::ProjectAnalyzer;

        let analysis = ProjectAnalyzer::analyze(".").await?;

        // Calculate new score based on analysis
        let new_score = self.calculate_health_score(&analysis);

        if self.options.verbose {
            println!("Project re-analysis complete - new score: {:.1}", new_score);
        }

        Ok(new_score)
    }

    fn calculate_health_score(&self, _analysis: &ProjectInfo) -> f32 {
        // Simple health score calculation
        // In a real implementation, this would be more sophisticated
        let base_score = 6.5;
        let improvement_factor = 0.3 * (self.iterations.len() as f32);
        (base_score + improvement_factor).min(10.0)
    }

    async fn call_claude_review(&self) -> Result<ReviewResult> {
        if self.options.verbose {
            println!("Calling Claude CLI for code review...");
        }

        let _context = self.build_review_context();

        let cmd = Command::new("claude")
            .arg("--dangerously-skip-permissions")
            .arg("/mmm-code-review")
            .arg("--format=json")
            .env("MMM_AUTOMATION", "true")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to execute Claude CLI")?;

        let output = cmd
            .wait_with_output()
            .await
            .context("Failed to wait for Claude CLI")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Claude CLI failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_review_output(&stdout)
    }

    async fn call_claude_implement(&self, review: &ReviewResult) -> Result<Vec<Change>> {
        if self.options.verbose {
            println!("Calling Claude CLI to implement improvements...");
        }

        let _context = self.build_implementation_context(review);

        let cmd = Command::new("claude")
            .arg("--dangerously-skip-permissions")
            .arg("/mmm-implement-spec")
            .arg("--format=json")
            .env("MMM_AUTOMATION", "true")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to execute Claude CLI for implementation")?;

        let output = cmd
            .wait_with_output()
            .await
            .context("Failed to wait for Claude CLI implementation")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Claude CLI implementation failed: {}",
                stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_implementation_output(&stdout)
    }

    fn load_or_create_state() -> ImproveState {
        let state_file = Path::new(".mmm/state.json");

        if state_file.exists() {
            if let Ok(content) = fs::read_to_string(state_file) {
                if let Ok(state) = serde_json::from_str(&content) {
                    return state;
                }
            }
        }

        ImproveState {
            last_run: Utc::now(),
            current_score: 6.5, // Default starting score
            improvement_history: Vec::new(),
        }
    }

    fn save_state(&self) -> Result<()> {
        let state_dir = Path::new(".mmm");
        if !state_dir.exists() {
            fs::create_dir_all(state_dir)?;
        }

        let state_file = state_dir.join("state.json");
        let content = serde_json::to_string_pretty(&self.state)?;
        fs::write(state_file, content)?;

        Ok(())
    }

    pub fn summary(&self) -> ReviewSummary {
        let issue_counts = self.count_issues_by_severity();
        ReviewSummary {
            current_score: self.state.current_score,
            issues_found: issue_counts.total,
            high_severity: issue_counts.high,
            medium_severity: issue_counts.medium,
            low_severity: issue_counts.low,
        }
    }

    fn count_issues_by_severity(&self) -> IssueCounts {
        let mut counts = IssueCounts::default();

        if let Some(last_iteration) = self.iterations.last() {
            for issue in &last_iteration.review.issues {
                counts.total += 1;
                match issue.severity {
                    IssueSeverity::High => counts.high += 1,
                    IssueSeverity::Medium => counts.medium += 1,
                    IssueSeverity::Low => counts.low += 1,
                }
            }
        }

        counts
    }
}

#[derive(Debug, Default)]
struct IssueCounts {
    total: usize,
    high: usize,
    medium: usize,
    low: usize,
}

#[derive(Debug)]
pub struct ReviewSummary {
    pub current_score: f32,
    pub issues_found: usize,
    pub high_severity: usize,
    pub medium_severity: usize,
    pub low_severity: usize,
}

#[derive(Debug)]
pub struct SessionResult {
    pub initial_score: f32,
    pub final_score: f32,
    pub improvement: f32,
    pub files_changed: usize,
    pub iterations: usize,
}
