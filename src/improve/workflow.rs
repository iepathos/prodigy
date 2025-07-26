use super::extractor::ExtractorEngine;
use crate::config::workflow::{Extractor, WorkflowConfig, WorkflowStep};
use anyhow::{anyhow, Context as _, Result};
use tokio::process::Command;

/// Execute a configurable workflow
pub struct WorkflowExecutor {
    config: WorkflowConfig,
    extractor_engine: ExtractorEngine,
    verbose: bool,
}

impl WorkflowExecutor {
    pub fn new(config: WorkflowConfig, verbose: bool) -> Self {
        let extractor_engine = ExtractorEngine::new(config.extractors.clone());
        Self {
            config,
            extractor_engine,
            verbose,
        }
    }

    /// Execute a single iteration of the workflow
    pub async fn execute_iteration(&mut self, iteration: u32, focus: Option<&str>) -> Result<bool> {
        if self.verbose {
            println!(
                "🔄 Workflow iteration {}/{}...",
                iteration, self.config.max_iterations
            );
        }

        // Extract initial values (e.g., from git)
        self.extractor_engine.extract_all(self.verbose).await?;

        let mut success = true;
        let mut any_changes = false;

        let steps = self.config.steps.clone();
        for (idx, step) in steps.iter().enumerate() {
            if self.verbose {
                println!(
                    "📋 Step {}/{}: {}",
                    idx + 1,
                    steps.len(),
                    step.name
                );
            }

            // Check if this is the first step and we have a focus directive
            let step_focus = if idx == 0 && iteration == 1 {
                focus
            } else {
                None
            };

            match self.execute_step(step, step_focus).await {
                Ok(step_success) => {
                    if step_success {
                        any_changes = true;
                    } else if !self.config.continue_on_error {
                        success = false;
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("❌ Step '{}' failed: {}", step.name, e);
                    if !self.config.continue_on_error {
                        return Err(e);
                    }
                    success = false;
                }
            }

            // Re-extract values after each step (in case they changed)
            self.extractor_engine.extract_all(false).await?;
        }

        Ok(success && any_changes)
    }

    /// Execute a single workflow step
    async fn execute_step(&mut self, step: &WorkflowStep, focus: Option<&str>) -> Result<bool> {
        // First check if claude command exists
        let claude_check = Command::new("which")
            .arg("claude")
            .output()
            .await
            .context("Failed to check for Claude CLI")?;

        if !claude_check.status.success() {
            return Err(anyhow!(
                "Claude CLI not found. Please install Claude CLI: https://claude.ai/cli"
            ));
        }

        // Interpolate arguments with extracted values
        let values = self.extractor_engine.get_values();
        let interpolated_args = self.config.interpolate_args(&step.args, values);

        // Build command
        let mut cmd = Command::new("claude");
        cmd.arg("--dangerously-skip-permissions")
            .arg("--print")
            .arg(format!("/{}", step.command))
            .args(&interpolated_args)
            .env("MMM_AUTOMATION", "true");

        // Add focus directive if provided (for first step of first iteration)
        if let Some(focus_directive) = focus {
            cmd.env("MMM_FOCUS", focus_directive);
        }

        // Execute command
        let output = cmd
            .output()
            .await
            .context(format!("Failed to execute step: {}", step.name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if self.verbose {
                eprintln!("Step stderr: {}", stderr);
            }
            return Err(anyhow!(
                "Step '{}' failed with exit code {}: {}",
                step.name,
                output.status.code().unwrap_or(-1),
                stderr
            ));
        }

        // Check for output extractors
        let stdout = String::from_utf8_lossy(&output.stdout);
        for (key, extractor) in &self.config.extractors {
            if let Extractor::Output { pattern } = extractor {
                if let Ok(value) = self.extract_from_output(&stdout, pattern) {
                    if !value.is_empty() {
                        self.extractor_engine.update_value(key.clone(), value);
                    }
                }
            }
        }

        if self.verbose {
            println!("✅ Step '{}' completed", step.name);
        }

        // Assume success if exit code is 0
        Ok(true)
    }

    /// Extract value from command output using regex
    fn extract_from_output(&self, output: &str, pattern: &str) -> Result<String> {
        let re = regex::Regex::new(pattern).context("Invalid regex pattern")?;

        if let Some(captures) = re.captures(output) {
            if captures.len() > 1 {
                Ok(captures
                    .get(1)
                    .map(|m| m.as_str())
                    .unwrap_or("")
                    .to_string())
            } else {
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
}
